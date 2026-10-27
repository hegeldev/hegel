"""The run lifecycle: driving the engine, replaying failures, reporting."""

from __future__ import annotations

import contextlib
import functools
import inspect
import os
import sys
import threading
import traceback
from collections.abc import Callable, Iterable, Iterator
from dataclasses import dataclass
from typing import Any, NoReturn, TypeVar

from hegel import _native
from hegel.errors import (
    AssumeFailed,
    DidNotReproduce,
    FailedHealthCheck,
    Flaky,
    InternalError,
    InvalidArgument,
    LoopDone,
    MultipleFailures,
    RunError,
    StopTest,
    Unsatisfiable,
)
from hegel.explicit import NO_HANDLE, ExplicitCase, ExplicitTestCase
from hegel.settings import HealthCheck, Phase, Settings, TestLocation, Verbosity
from hegel.test_case import OutputSink, TestCase

__all__ = [
    "default_reproducer",
    "explicit_test_case",
    "reproduce_failure",
    "run",
    "test",
    "with_output_override",
]

T = TypeVar("T")

_HEGEL_DIR = os.path.dirname(os.path.abspath(__file__))
_output_override = threading.local()
_running = threading.local()


def _stderr_line(line: str) -> None:
    sys.stderr.write(line + "\n")


class RunOutput:
    """Where a run's output goes: the installed override, or stderr."""

    __slots__ = ("sink",)

    def __init__(self, sink: OutputSink | None) -> None:
        self.sink = sink

    @classmethod
    def resolve(cls) -> RunOutput:
        return cls(getattr(_output_override, "sink", None))

    def line(self, message: str) -> None:
        (self.sink or _stderr_line)(message)

    def block(self, text: str) -> None:
        self.lines(text.rstrip("\n").split("\n"))

    def lines(self, lines: Iterable[str]) -> None:
        for line in lines:
            self.line(line)

    def resolved_sink(self) -> OutputSink:
        return self.sink or _stderr_line


@contextlib.contextmanager
def with_output_override(sink: OutputSink) -> Iterator[None]:
    """Route all of Hegel's output on this thread through ``sink`` (called
    once per line) instead of stderr, for the duration of the block."""
    previous = getattr(_output_override, "sink", None)
    _output_override.sink = sink
    try:
        yield
    finally:
        _output_override.sink = previous


@dataclass
class _CaseOutcome:
    status: int
    origin: str | None
    exception: BaseException | None
    stamped: bool


def _user_location(exception: BaseException) -> str:
    """``file:line`` of the innermost traceback frame outside Hegel."""
    location = None
    for frame in traceback.extract_tb(exception.__traceback__):
        if location is None or not _inside_hegel(frame.filename):
            location = f"{frame.filename}:{frame.lineno}"
    return location or "<unknown>"


def _origin(exception: BaseException, seen: tuple[BaseException, ...] = ()) -> str:
    """The origin the engine groups failures by: the exception's type and
    user location, followed by the exception it was raised from (its cause,
    or the one it was handling) and, for an exception group, its members,
    so that one ``raise`` reached by different routes counts as different
    bugs. One line, as it is also shown in messages."""
    seen = (*seen, exception)
    origin = f"{type(exception).__qualname__} at {_user_location(exception)}"
    chained = exception.__cause__
    if chained is None:
        chained = exception.__context__
    if chained is not None and not _is_in(chained, seen):
        origin += f" (from {_origin(chained, seen)})"
    if isinstance(exception, BaseExceptionGroup):
        members = (_origin(e, seen) for e in exception.exceptions if not _is_in(e, seen))
        origin += f" [{', '.join(members)}]"
    return origin


def _is_in(exception: BaseException, seen: tuple[BaseException, ...]) -> bool:
    return any(exception is s for s in seen)


def _inside_hegel(filename: str) -> bool:
    return os.path.abspath(filename).startswith(_HEGEL_DIR + os.sep)


def _format_exception(exception: BaseException) -> str:
    return "".join(traceback.format_exception(exception)).rstrip("\n")


def _run_test_case(
    handle: _native.TestCase,
    test_fn: Callable[[TestCase], Any],
    verbosity: Verbosity,
    output: RunOutput,
    case_sink: OutputSink | None,
) -> _CaseOutcome:
    """Run one engine-scheduled test case to completion.

    The engine stamps the cases a failure report can be built from (the
    final replay of each failure it is about to report, confirmation
    replays, and generation cases under nondeterministic handling); a
    stamped case records its draws and notes for the report. Other cases
    run silently unless the run is verbose.
    """
    verbose = verbosity in (Verbosity.verbose, Verbosity.debug)
    quiet = verbosity is Verbosity.quiet
    stamped = handle.should_capture()
    should_emit = (stamped and not quiet) or verbose
    tc = TestCase(handle, should_emit, case_sink or output.resolved_sink())
    outcome = _CaseOutcome(_native.STATUS_VALID, None, None, stamped)
    try:
        _call_test(test_fn, tc)
    except (AssumeFailed, LoopDone) as signal:
        if isinstance(signal, AssumeFailed):
            outcome.status = _native.STATUS_INVALID
    except StopTest:
        outcome.status = _native.STATUS_OVERRUN
    except (InvalidArgument, InternalError):
        tc._emit_rendered_output()
        raise
    except (KeyboardInterrupt, SystemExit):
        raise
    except _skip_exceptions():
        raise
    except BaseException as exception:
        outcome.status = _native.STATUS_INTERESTING
        outcome.origin = _origin(exception)
        outcome.exception = exception
        if verbose:
            output.block(_format_exception(exception))
    tc._emit_rendered_output()
    if verbose:
        reason = _stop_reason(outcome.status)
        if reason is not None:
            output.line(reason)
    handle.mark_complete(outcome.status, outcome.origin)
    return outcome


# Exceptions that steer a test case rather than fail it: the ones
# ``_run_test_case`` handles by kind.
_CONTROL_EXCEPTIONS = (AssumeFailed, LoopDone, StopTest, InvalidArgument, InternalError)


def _call_test(test_fn: Callable[[TestCase], Any], tc: TestCase) -> None:
    """Call the test, unwrapping an exception group of nothing but control
    exceptions (from a ``TaskGroup`` whose tasks drew or rejected, say) to
    the one to handle: the first member that is not a ``StopTest``, else the
    first ``StopTest``. A group with any other exception in it is a failure
    like any other."""
    try:
        _check_not_awaitable(test_fn(tc))
    except BaseExceptionGroup as group:
        signals, others = group.split(_CONTROL_EXCEPTIONS)
        if others is not None or signals is None:
            raise
        leaves = _leaves(signals)
        signal = next((e for e in leaves if not isinstance(e, StopTest)), leaves[0])
        raise signal from signal.__cause__


def _leaves(group: BaseExceptionGroup[Any]) -> list[BaseException]:
    leaves: list[BaseException] = []
    for member in group.exceptions:
        if isinstance(member, BaseExceptionGroup):
            leaves.extend(_leaves(member))
        else:
            leaves.append(member)
    return leaves


def _check_not_awaitable(result: object) -> None:
    """Fail a test function that returned a coroutine (an ``async def``
    test, say): its body never ran, so it would otherwise pass."""
    if inspect.isawaitable(result):
        close = getattr(result, "close", None)
        if close is not None:
            close()
        raise InvalidArgument(
            "The test function returned an awaitable, so its body never ran: Hegel does "
            "not run async tests. Make the test synchronous, running the async code "
            "under test with asyncio.run()."
        )


def _skip_exceptions() -> tuple[type[BaseException], ...]:
    """Exceptions that skip a test outright (a test runner's "skip" signal)
    end the run instead of counting as a failure to shrink."""
    import unittest

    found: list[type[BaseException]] = [unittest.SkipTest]
    outcomes = sys.modules.get("_pytest.outcomes")
    if outcomes is not None:
        found.extend(
            exc
            for exc in (getattr(outcomes, "Skipped", None), getattr(outcomes, "XFailed", None))
            if exc is not None
        )
    return tuple(found)


def _stop_reason(status: int) -> str | None:
    """The verbose line saying why a test case stopped short of its end,
    for one that did."""
    if status == _native.STATUS_INVALID:
        return "Test case stopped: failed assumption"
    if status == _native.STATUS_OVERRUN:
        return "Test case stopped: out of data"
    return None


Reproducer = Callable[[str], str]


def default_reproducer(blob: str) -> str:
    """How a failure report says to replay a failure: with the
    :func:`reproduce_failure` decorator."""
    return (
        "To reproduce this failure, add this decorator below @hegel.test:\n"
        f'    @hegel.reproduce_failure("{blob}")'
    )


def _annotate(
    exception: BaseException, lines: list[str], caveat: str | None, reproducer: str | None
) -> None:
    notes = []
    if lines:
        notes.append("\n".join(lines))
    if caveat is not None:
        notes.append(f"note: {caveat}")
    if reproducer is not None:
        notes.append(reproducer)
    for note in notes:
        exception.add_note(note)


def _trim_traceback(exception: BaseException) -> None:
    """Drop the leading frames of Hegel's own (the runner calling the test)
    from ``exception``'s traceback, so it starts in the user's code. A
    traceback with no frame outside Hegel (an error in Hegel itself) is
    left whole."""
    tb = exception.__traceback__
    while tb is not None and _inside_hegel(tb.tb_frame.f_code.co_filename):
        tb = tb.tb_next
    if tb is not None:
        exception.__traceback__ = tb


@dataclass
class _Captured:
    """One origin's failure report material, captured as the case ran:
    its buffered draw and note lines (empty under quiet verbosity), whether
    the engine stamped the case, and the exception to re-raise. Captures
    are ranked so that an unstamped shrink probe's bare capture cannot
    replace a stamped one, while at equal rank the newest wins: final
    replay over confirmation over discovery."""

    lines: list[str]
    stamped: bool
    exception: BaseException

    @property
    def rank(self) -> int:
        if self.stamped:
            return 2
        return 1 if self.lines else 0


def _drive(
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
    reproducer: Reproducer,
) -> None:
    __tracebackhide__ = True
    output = RunOutput.resolve()
    native_settings = settings._build(database_key, test_location)
    run = _native.Run.start(native_settings)
    failures = _drive_run(run, test_fn, native_settings, output, reproducer)
    if failures:
        _raise_failures(failures, None)


def _drive_blob_replay(
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
    reproducer: Reproducer,
    blobs: list[str],
) -> NoReturn:
    """Replay every reproduce blob, each in a run of its own: the engine
    replays a blob until a replay fails, under a bounded budget. Every
    failure found is raised, together; when none of the blobs reproduced,
    that is a :class:`~hegel.DidNotReproduce`."""
    __tracebackhide__ = True
    output = RunOutput.resolve()
    failures: list[BaseException] = []
    for blob in blobs:
        native_settings = settings._build(database_key, test_location)
        run = _native.Run.start_blob(native_settings, blob)
        failures += _drive_run(run, test_fn, native_settings, output, reproducer)
    if not failures:
        raise DidNotReproduce(
            "reproduce_failure: the supplied failure blob did not reproduce a failure. "
            "The failure may have been fixed, or, for a nondeterministic blob, it may not "
            "have recurred within the replay budget (a bug failing 10% of the time escapes "
            "it about 5% of the time). Re-run to retry, or remove the decorator once the "
            "failure is fixed."
            if len(blobs) == 1
            else f"reproduce_failure: none of the {len(blobs)} supplied failure blobs "
            "reproduced a failure. The failures may have been fixed, or, for "
            "nondeterministic blobs, they may not have recurred within the replay budget. "
            "Re-run to retry, or remove the decorators once the failures are fixed."
        )
    _raise_failures(failures, None)


# The exception to raise for a run that ended in error, by the kind the
# engine reports for it. Any other kind (a usage error, an internal engine
# error) is a plain RunError carrying the engine's message.
_RUN_ERROR_KINDS: dict[int, type[RunError]] = {
    _native.RUN_ERROR_FLAKY: Flaky,
    _native.RUN_ERROR_NONDETERMINISTIC: Flaky,
    _native.RUN_ERROR_UNSATISFIABLE: Unsatisfiable,
}


def _run_error(result: _native.RunResult) -> RunError:
    kind, check = result.error_kind()
    message = result.error() or "the run failed with an unknown error"
    if kind == _native.RUN_ERROR_HEALTH_CHECK:
        health_check = HealthCheck(check)
        return FailedHealthCheck(
            f"{message}\nThe failed health check is HealthCheck.{health_check.name}; "
            f"suppress it with suppress_health_check=[HealthCheck.{health_check.name}].",
            health_check,
        )
    return _RUN_ERROR_KINDS.get(kind, RunError)(message)


def _drive_run(
    run: _native.Run,
    test_fn: Callable[[TestCase], Any],
    settings: _native.Settings,
    output: RunOutput,
    reproducer: Reproducer,
) -> list[BaseException]:
    """Pull every test case off ``run``, capturing report material per
    failure origin, then act on the verdict. The engine owns the whole
    exploration, the final replay of each reported failure included, so
    the report is assembled from the freshest stamped capture of each
    origin, and attached to the exception as notes. Returns the failures
    to raise, none for a passing run; a run ending in error raises."""
    __tracebackhide__ = True
    verbosity = Verbosity(settings.get_verbosity())
    print_blob = settings.get_print_blob()
    quiet = verbosity is Verbosity.quiet
    verbose = verbosity in (Verbosity.verbose, Verbosity.debug)
    captured: dict[str, _Captured] = {}

    def flush() -> None:
        output.lines(run.drain_output())

    while True:
        handle = run.next_test_case()
        flush()
        if handle is None:
            break
        buffer: list[str] = []
        case_sink: OutputSink | None = None
        if not quiet:

            def case_sink(line: str, buffer: list[str] = buffer) -> None:
                if verbose:
                    output.line(line)
                buffer.append(line)

        outcome = _run_test_case(handle, test_fn, verbosity, output, case_sink)
        flush()
        if outcome.status == _native.STATUS_INTERESTING and outcome.exception is not None:
            report = _Captured(buffer, outcome.stamped, outcome.exception)
            origin = outcome.origin or ""
            stored = captured.get(origin)
            if stored is None or report.rank >= stored.rank:
                captured[origin] = report

    result = run.result()
    flush()
    status = result.status()
    if status == _native.RUN_STATUS_PASSED:
        return []
    if status == _native.RUN_STATUS_ERROR:
        raise _run_error(result)
    count = result.failure_count()
    exceptions: list[BaseException] = []
    for index in range(count):
        origin, blob, caveat = result.failure(index)
        if origin not in captured:
            raise InternalError(f"failure {index} ({origin}) was never seen by the test")
        report = captured.pop(origin)
        note = reproducer(blob) if print_blob and blob is not None else None
        _annotate(report.exception, report.lines, None if quiet else caveat, note)
        _trim_traceback(report.exception)
        exceptions.append(report.exception)
    return exceptions


def _run_explicit(
    test_fn: Callable[[TestCase], Any],
    case: ExplicitCase,
    verbosity: Verbosity,
    output: RunOutput,
) -> _CaseOutcome:
    """Run one explicit test case. A failure is returned in the outcome
    rather than raised, annotated with the case's drawn values and notes
    (none under quiet verbosity), so the caller can raise it or collect it
    with further failures. A verbose run reports the case as it does a
    generated one: the drawn values, then the exception when it failed."""
    verbose = verbosity in (Verbosity.verbose, Verbosity.debug)
    explicit = ExplicitTestCase(case)
    tc = TestCase(NO_HANDLE, True, output.resolved_sink(), explicit)
    outcome = _CaseOutcome(_native.STATUS_VALID, None, None, True)
    try:
        _check_not_awaitable(test_fn(tc))
    except LoopDone:
        pass
    except (InvalidArgument, InternalError):
        if verbose:
            output.lines(explicit.lines)
        raise
    except (KeyboardInterrupt, SystemExit):
        raise
    except _skip_exceptions():
        raise
    except BaseException as exception:
        outcome.status = _native.STATUS_INTERESTING
        outcome.origin = _origin(exception)
        outcome.exception = exception
    if verbose:
        output.lines(explicit.lines)
        if outcome.exception is not None:
            output.block(_format_exception(outcome.exception))
    if outcome.exception is None:
        explicit.check_consumed()
        return outcome
    lines = [] if verbosity is Verbosity.quiet else explicit.lines
    _annotate(outcome.exception, lines, None, None)
    _trim_traceback(outcome.exception)
    return outcome


def _drive_after_explicit_failures(
    failures: dict[str, BaseException],
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
    reproducer: Reproducer,
) -> NoReturn:
    """Under ``report_multiple_failures``, explicit cases failing (keyed by
    origin in ``failures``) do not end the run: the engine's phases still
    look for other bugs, and every distinct failure is then reported
    together, the explicit ones first. An error ending the engine's run
    is chained onto the failures rather than hiding them."""
    __tracebackhide__ = True
    cause: BaseException | None = None
    generated: list[BaseException] = []
    try:
        _drive(test_fn, settings, database_key, test_location, reproducer)
    except (InvalidArgument, InternalError, KeyboardInterrupt, SystemExit):
        raise
    except _skip_exceptions():
        raise
    except RunError as error:
        cause = error
    except MultipleFailures as group:
        generated = list(group.exceptions)
    except BaseException as exception:
        generated = [exception]
    for found in generated:
        failures.setdefault(_origin(found), found)
    _raise_failures(list(failures.values()), cause)


def _raise_failures(exceptions: list[BaseException], cause: BaseException | None) -> NoReturn:
    __tracebackhide__ = True
    failure = exceptions[0]
    if len(exceptions) > 1:
        failure = MultipleFailures(
            f"Property-based test failed with {len(exceptions)} distinct failures.",
            exceptions,  # type: ignore[arg-type]
        )
    if cause is not None:
        failure.__cause__ = cause
    raise failure


def run(
    test_fn: Callable[[TestCase], Any],
    settings: Settings | None = None,
    *,
    database_key: str | None = None,
    test_location: TestLocation | None = None,
    explicit_cases: Iterator[ExplicitCase] | list[ExplicitCase] | tuple[ExplicitCase, ...] = (),
    reproduce_blob: str | Iterable[str] | None = None,
    reproducer: Reproducer = default_reproducer,
    **overrides: Any,
) -> None:
    """Run ``test_fn`` as a property test, without a decorator: it is
    called once per test case with a :class:`TestCase`, and the first
    exception it raises (other than a rejected assumption) is a failure,
    reported after shrinking: the drawn values, any caveat, and how to
    reproduce it are added to the exception as notes, and the exception is
    re-raised. A passing run returns ``None``.

    ``settings`` (with ``overrides`` applied, as for :class:`Settings`)
    configures the run; by default it is ``Settings()``.
    ``database_key`` identifies the test in the example database (without
    one, failures are not saved or replayed) and ``test_location`` in the
    engine's reports. ``explicit_cases`` run first, with their given
    values. ``reproduce_blob`` (one blob, or a collection of them) makes the
    run consist only of replaying previously reported failures, each blob
    in turn: every failure they reproduce is raised, together, and when
    none reproduces the run raises :class:`~hegel.DidNotReproduce`.
    ``reproducer`` words the note saying how to replay a failure, given its
    blob.

    ::

        def check(tc):
            xs = tc.draw(gs.lists(gs.integers()))
            assert sorted(sorted(xs)) == sorted(xs)

        hegel.run(check, test_cases=1000)

    :func:`test` is this, run when the decorated function is called.
    """
    __tracebackhide__ = True
    if not callable(test_fn):
        raise InvalidArgument("run() takes a test function")
    _check_settings(settings)
    resolved = (
        Settings(settings, **overrides) if (settings is not None or overrides) else Settings()
    )
    blobs = _blobs(reproduce_blob)
    _running.depth = getattr(_running, "depth", 0) + 1
    try:
        if blobs:
            _drive_blob_replay(test_fn, resolved, database_key, test_location, reproducer, blobs)
        # Explicit failures, by origin: the first explicit case failing ends
        # the run, unless every distinct failure is to be reported.
        failures: dict[str, BaseException] = {}
        values = resolved._values
        if Phase.explicit in values["phases"]:
            output = RunOutput.resolve()
            for case in explicit_cases:
                outcome = _run_explicit(test_fn, case, values["verbosity"], output)
                if outcome.exception is None:
                    continue
                if not values["report_multiple_failures"]:
                    raise outcome.exception
                failures.setdefault(outcome.origin or "", outcome.exception)
        if failures:
            _drive_after_explicit_failures(
                failures, test_fn, resolved, database_key, test_location, reproducer
            )
        _drive(test_fn, resolved, database_key, test_location, reproducer)
    finally:
        _running.depth -= 1


def _blobs(reproduce_blob: str | Iterable[str] | None) -> list[str]:
    if reproduce_blob is None:
        return []
    blobs = [reproduce_blob] if isinstance(reproduce_blob, str) else list(reproduce_blob)
    for blob in blobs:
        _check_blob(blob)
    return blobs


def _check_blob(blob: Any) -> None:
    if not isinstance(blob, str) or not blob:
        raise InvalidArgument("reproduce_failure takes the blob string printed with a failure")


def _inside_a_run() -> bool:
    return getattr(_running, "depth", 0) > 0


def _check_settings(settings: Any) -> None:
    if settings is not None and not isinstance(settings, Settings):
        raise InvalidArgument(f"settings={settings!r} must be a Settings object or None")


def _location_of(fn: Callable[..., Any]) -> TestLocation | None:
    try:
        file = inspect.getsourcefile(fn) or inspect.getfile(fn)
        _, line = inspect.getsourcelines(fn)
    except (OSError, TypeError):
        return None
    qualname = getattr(fn, "__qualname__", fn.__name__)
    class_name = (
        qualname.rsplit(".", 1)[0] if "." in qualname and "<locals>" not in qualname else ""
    )
    return TestLocation(file, line, class_name, fn.__name__)


def _database_key_of(fn: Callable[..., Any]) -> str:
    return f"{getattr(fn, '__module__', '')}.{getattr(fn, '__qualname__', fn.__name__)}"


def test(
    fn: Callable[..., Any] | None = None,
    /,
    *,
    settings: Settings | None = None,
    **overrides: Any,
) -> Any:
    """Decorate a test function taking a :class:`TestCase` as its first
    argument (after ``self``, for a method) so that calling it, as pytest
    does, runs the property test.

    ::

        @hegel.test
        def test_reversing_twice(tc):
            xs = tc.draw(gs.lists(gs.integers()))
            assert list(reversed(list(reversed(xs)))) == xs

        @hegel.test(test_cases=500, derandomize=True)
        def test_with_settings(tc): ...

    The decorated function takes the remaining parameters, which are
    passed through, so pytest fixtures and ``parametrize`` work. Settings
    may be given as ``settings=`` (a :class:`Settings`), as individual
    fields, or both, the fields overriding; without ``settings=`` they
    start from the default profile, resolved each time the test runs.
    Failing examples are saved in the example database under the test's
    module and qualified name, and replayed first on the next run.

    The test runs synchronously: an ``async def`` test, a class, or a
    function with no positional parameter for the test case is an
    :class:`~hegel.InvalidArgument`, as is calling a Hegel test from inside
    another. The wrapper has ``is_hegel_test = True`` and the original
    function as ``hegel_inner_test``.
    """
    _check_settings(settings)

    def decorate(inner: Callable[..., Any]) -> Callable[..., Any]:
        if not callable(inner):
            raise InvalidArgument("@hegel.test must decorate a function")
        if inspect.isclass(inner):
            raise InvalidArgument(
                f"@hegel.test decorates a test function, not the class {inner.__name__}. "
                "Decorate its test methods instead."
            )
        if inspect.iscoroutinefunction(inner) or inspect.isasyncgenfunction(inner):
            raise InvalidArgument(
                f"{inner.__name__} is async, and Hegel does not run async tests. Make it "
                "synchronous, running the async code under test with asyncio.run()."
            )
        signature = inspect.signature(inner)
        parameters = list(signature.parameters.values())
        # A method takes the test case after ``self``.
        tc_index = 1 if parameters and parameters[0].name in ("self", "cls") else 0
        if tc_index >= len(parameters):
            after = f" after {parameters[0].name}" if parameters else ""
            raise InvalidArgument(
                f"{inner.__name__} must take the test case as its first argument{after}"
            )
        if "POSITIONAL" not in parameters[tc_index].kind.name:
            raise InvalidArgument(
                f"{inner.__name__} must take the test case as a positional parameter, but "
                f"{parameters[tc_index].name!r} cannot be passed positionally"
            )
        location = _location_of(inner)
        database_key = _database_key_of(inner)
        reproducer = getattr(inner, "hegel_reproducer", default_reproducer)
        base_settings = settings

        remaining = parameters[:tc_index] + parameters[tc_index + 1 :]
        outer_signature = signature.replace(parameters=remaining)

        @functools.wraps(inner)
        def wrapper(*args: Any, **kwargs: Any) -> None:
            __tracebackhide__ = True
            if _inside_a_run():
                raise InvalidArgument(
                    f"{inner.__name__} is a Hegel test, called from inside another Hegel "
                    "test. Draw what it needs in the outer test instead, or move its body "
                    "into a helper function taking the test case."
                )
            # Arguments that don't fit are the caller's mistake, not a failure
            # for the engine to shrink.
            outer_signature.bind(*args, **kwargs)
            leading, rest = args[:tc_index], args[tc_index:]
            run(
                lambda tc: inner(*leading, tc, *rest, **kwargs),
                base_settings,
                database_key=database_key,
                test_location=location,
                explicit_cases=list(wrapper.hegel_explicit_cases),  # type: ignore[attr-defined]
                reproduce_blob=wrapper.hegel_reproduce_blobs,  # type: ignore[attr-defined]
                reproducer=reproducer,
                **overrides,
            )

        wrapper.__signature__ = outer_signature  # type: ignore[attr-defined]
        wrapper.hegel_inner_test = inner  # type: ignore[attr-defined]
        wrapper.hegel_explicit_cases = list(getattr(inner, "hegel_explicit_cases", []))  # type: ignore[attr-defined]
        wrapper.hegel_reproduce_blobs = list(getattr(inner, "hegel_reproduce_blobs", []))  # type: ignore[attr-defined]
        wrapper.is_hegel_test = True  # type: ignore[attr-defined]
        return wrapper

    if fn is not None:
        if settings is not None or overrides:
            raise InvalidArgument("@hegel.test takes either a function or keyword settings")
        return decorate(fn)
    return decorate


def explicit_test_case(*args: Any, **kwargs: Any) -> Callable[[T], T]:
    """Add an explicit test case: the test runs with these values before
    any are generated, in the ``explicit`` phase. Positional values are
    consumed by draws in order; keyword values by draw name (``x=1`` for
    ``x = tc.draw(...)``).

    ::

        @hegel.explicit_test_case(xs=[])
        @hegel.explicit_test_case(xs=[1, 1])
        @hegel.test
        def test_sort_keeps_length(tc):
            xs = tc.draw(gs.lists(gs.integers()))
            assert len(sorted(xs)) == len(xs)

    Stacked decorators run their cases top to bottom. Explicit cases are
    not shrunk, and their values are not checked against the generators:
    each draw simply returns the given value. Every value must be drawn,
    and every draw needs a value; a mismatch, a rejected assumption, or
    :meth:`~TestCase.draw_silent` in an explicit case is an
    :class:`~hegel.InvalidArgument`. The decorator may go above or below
    :func:`test`.
    """
    case = ExplicitCase(args, kwargs)

    def decorate(fn: T) -> T:
        cases = getattr(fn, "hegel_explicit_cases", None)
        if cases is None:
            cases = []
            fn.hegel_explicit_cases = cases  # type: ignore[attr-defined]
        cases.insert(0, case)
        return fn

    return decorate


def reproduce_failure(blob: str) -> Callable[[T], T]:
    """Replay the failure that ``blob`` encodes instead of generating test
    cases.

    A failure report ends with a line such as
    ``@hegel.reproduce_failure("AXicY2Vg...")`` when ``print_blob`` is on;
    adding that decorator to a :func:`test` makes the next run consist
    only of replaying exactly that failing test case, which is useful when
    the failure is hard to find again. The decorators of several failures
    (from a report of :class:`~hegel.MultipleFailures`, say) may be
    stacked: each blob is replayed, top to bottom, and every failure they
    reproduce is raised, together. If none of them fails any more, the run
    raises :class:`~hegel.DidNotReproduce`. Remove the decorators once the
    bugs are fixed. An empty or non-string blob is an
    :class:`~hegel.InvalidArgument`.
    """
    _check_blob(blob)

    def decorate(fn: T) -> T:
        blobs = getattr(fn, "hegel_reproduce_blobs", None)
        if blobs is None:
            blobs = []
            fn.hegel_reproduce_blobs = blobs  # type: ignore[attr-defined]
        blobs.insert(0, blob)
        return fn

    return decorate
