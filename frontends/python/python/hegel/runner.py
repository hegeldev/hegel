"""The run lifecycle: driving the engine, replaying failures, reporting."""

from __future__ import annotations

import argparse
import contextlib
import functools
import inspect
import os
import sys
import threading
import traceback
from collections.abc import Callable, Iterable, Iterator, Sequence
from dataclasses import dataclass
from types import TracebackType
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
from hegel.settings import (
    _FIELDS,
    HealthCheck,
    Phase,
    Settings,
    TestLocation,
    Verbosity,
    _without_function_name,
)
from hegel.test_case import OutputSink, TestCase

__all__ = [
    "default_reproducer",
    "explicit_test_case",
    "main",
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
    """Where a run's output goes: the installed override, or stderr. A
    ``None`` sink is nowhere: nothing is printed."""

    __slots__ = ("sink",)

    def __init__(self, sink: OutputSink | None) -> None:
        self.sink = sink

    @classmethod
    def resolve(cls) -> RunOutput:
        return cls(getattr(_output_override, "sink", _stderr_line))

    def line(self, message: str) -> None:
        if self.sink is not None:
            self.sink(message)

    def block(self, text: str) -> None:
        self.lines(text.rstrip("\n").split("\n"))

    def lines(self, lines: Iterable[str]) -> None:
        for line in lines:
            self.line(line)


@contextlib.contextmanager
def with_output_override(sink: OutputSink | None) -> Iterator[None]:
    """Route all of Hegel's output on this thread through ``sink`` (called
    once per line) instead of stderr, for the duration of the block, or
    nowhere when ``sink`` is ``None``. A run started in the block calls
    ``sink`` as it prints, which may be from the test's own threads; an
    exception ``sink`` raises goes to :func:`sys.unraisablehook`. With no
    destination, the engine prints nothing and does no work for verbose
    output."""
    previous = getattr(_output_override, "sink", _stderr_line)
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
    wants_output: bool = False


def _user_frame(exception: BaseException) -> traceback.FrameSummary | None:
    """The innermost traceback frame outside Hegel, or the innermost frame
    when every frame is Hegel's."""
    found = None
    for frame in traceback.extract_tb(exception.__traceback__):
        if found is None or not _inside_hegel(frame.filename):
            found = frame
    return found


def _user_location(exception: BaseException) -> str:
    """``file:line`` of :func:`_user_frame`."""
    frame = _user_frame(exception)
    return f"{frame.filename}:{frame.lineno}" if frame else "<unknown>"


def _summary(exception: BaseException) -> str:
    """The one-line form of a failure the engine prints after a failing
    test case in a verbose run: the traceback's line for the frame it was
    raised in, as Python prints it, then its type and the first line of its
    message, `` …`` marking a message cut short."""
    kind = type(exception)
    name = kind.__qualname__
    if kind.__module__ not in ("builtins", "__main__"):
        name = f"{kind.__module__}.{name}"
    try:
        message = str(exception)
    except Exception:
        message = "<exception str() failed>"
    first, *rest = message.strip().splitlines() or [""]
    frame = _user_frame(exception)
    summary = f'File "{frame.filename}", line {frame.lineno}, in {frame.name}: ' if frame else ""
    summary += name
    if first:
        summary += f": {first}"
    if rest:
        summary += " …"
    return summary


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
    """The full traceback of ``exception``, as Python prints it, starting
    in the user's code (see :func:`_trim_traceback`)."""
    formatted = traceback.format_exception(type(exception), exception, _user_traceback(exception))
    return "".join(formatted).rstrip("\n")


def _run_test_case(handle: _native.TestCase, test_fn: Callable[[TestCase], Any]) -> _CaseOutcome:
    """Run one engine-scheduled test case to completion.

    The engine decides whether what the case prints is used (streamed by a
    verbose run, or kept for the failure report of a case it stamped for
    capture) and prints it itself; the case prints its draws and notes, and
    the one-line summary of its failure, only when it is, and formats the
    failure's traceback only when the engine wants that too.
    """
    wants_output = handle.wants_output()
    wants_detail = handle.wants_detail()
    tc = TestCase(handle, wants_output)
    outcome = _CaseOutcome(_native.STATUS_VALID, None, None, wants_output)
    try:
        _call_test(test_fn, tc)
    except (AssumeFailed, LoopDone) as signal:
        if isinstance(signal, AssumeFailed):
            outcome.status = _native.STATUS_INVALID
    except StopTest:
        outcome.status = _native.STATUS_OVERRUN
    except (InvalidArgument, InternalError, KeyboardInterrupt, SystemExit):
        raise
    except _skip_exceptions():
        raise
    except BaseException as exception:
        outcome.status = _native.STATUS_INTERESTING
        outcome.origin = _origin(exception)
        outcome.exception = exception
    summary = detail = None
    if outcome.exception is not None and wants_output:
        summary = _summary(outcome.exception)
        if wants_detail:
            detail = _format_exception(outcome.exception)
    handle.mark_complete(outcome.status, outcome.origin, summary, detail)
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


Reproducer = Callable[[str], str]


def default_reproducer(blob: str) -> str:
    """How a failure report says to replay a failure: with the
    :func:`reproduce_failure` decorator."""
    return (
        "To reproduce this failure, add this decorator below @hegel.test:\n"
        f'    @hegel.reproduce_failure("{blob}")'
    )


def _annotate(
    exception: BaseException, lines: str, caveat: str | None, reproducer: str | None
) -> None:
    notes = []
    if lines:
        notes.append(lines)
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
    exception.__traceback__ = _user_traceback(exception)


def _user_traceback(exception: BaseException) -> TracebackType | None:
    """``exception``'s traceback from its first frame outside Hegel, or
    whole when it has none."""
    tb = exception.__traceback__
    while tb is not None and _inside_hegel(tb.tb_frame.f_code.co_filename):
        tb = tb.tb_next
    return tb or exception.__traceback__


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
    run = _native.Run.start(native_settings, output.sink)
    failures = _drive_run(run, test_fn, native_settings, reproducer)
    if failures:
        _raise_failures(failures, None)


def _drive_blob_replay(
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
    reproducer: Reproducer,
    blobs: list[str],
    suggestion: str | None = None,
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
        run = _native.Run.start_blob(native_settings, blob, output.sink)
        failures += _drive_run(run, test_fn, native_settings, reproducer)
    if not failures:
        prefix = "reproduce_failure: " if suggestion is None else ""
        if suggestion is None:
            suggestion = "remove the decorator" if len(blobs) == 1 else "remove the decorators"
        raise DidNotReproduce(
            f"{prefix}the supplied failure blob did not reproduce a failure. "
            "The failure may have been fixed, or, for a nondeterministic blob, it may not "
            "have recurred within the replay budget (a bug failing 10% of the time escapes "
            f"it about 5% of the time). Re-run to retry, or {suggestion} once the failure is "
            "fixed."
            if len(blobs) == 1
            else f"{prefix}none of the {len(blobs)} supplied failure blobs "
            "reproduced a failure. The failures may have been fixed, or, for "
            "nondeterministic blobs, they may not have recurred within the replay budget. "
            f"Re-run to retry, or {suggestion} once the failures are fixed."
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
    reproducer: Reproducer,
) -> list[BaseException]:
    """Pull every test case off ``run``, keeping the exception to raise for
    each failure origin, then act on the verdict. The engine owns the whole
    exploration, the final replay of each reported failure included, and
    prints each test case's output itself: it keeps the lines of the latest
    failing execution it stamped for capture as the origin's report, which
    is attached to the exception as a note (the exception shows its own
    traceback). Returns the
    failures to raise, none for a passing run; a run ending in error
    raises."""
    __tracebackhide__ = True
    print_blob = settings.get_print_blob()
    quiet = Verbosity(settings.get_verbosity()) is Verbosity.quiet
    failing: dict[str, BaseException] = {}

    while (handle := run.next_test_case()) is not None:
        outcome = _run_test_case(handle, test_fn)
        origin = outcome.origin or ""
        if outcome.exception is not None and (outcome.wants_output or origin not in failing):
            failing[origin] = outcome.exception

    result = run.result()
    status = result.status()
    if status == _native.RUN_STATUS_PASSED:
        return []
    if status == _native.RUN_STATUS_ERROR:
        raise _run_error(result)
    count = result.failure_count()
    exceptions: list[BaseException] = []
    for index in range(count):
        origin, blob, caveat, report = result.failure(index)
        if origin not in failing:
            raise InternalError(f"failure {index} ({origin}) was never seen by the test")
        exception = failing.pop(origin)
        note = reproducer(blob) if print_blob and blob is not None else None
        _annotate(exception, report or "", None if quiet else caveat, note)
        _trim_traceback(exception)
        exceptions.append(exception)
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
    generated one: the drawn values, then the failure's summary when it
    failed, or its traceback at debug verbosity."""
    verbose = verbosity in (Verbosity.verbose, Verbosity.debug)
    explicit = ExplicitTestCase(case)
    tc = TestCase(NO_HANDLE, True, explicit)
    outcome = _CaseOutcome(_native.STATUS_VALID, None, None)
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
            debug = verbosity is Verbosity.debug
            output.block((_format_exception if debug else _summary)(outcome.exception))
    if outcome.exception is None:
        explicit.check_consumed()
        return outcome
    lines = "" if verbosity is Verbosity.quiet else "\n".join(explicit.lines)
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
    _run(
        test_fn,
        resolved,
        database_key=database_key,
        test_location=test_location,
        explicit_cases=explicit_cases,
        blobs=_blobs(reproduce_blob),
        reproducer=reproducer,
    )


def _run(
    test_fn: Callable[[TestCase], Any],
    resolved: Settings,
    *,
    database_key: str | None,
    test_location: TestLocation | None,
    explicit_cases: Iterable[ExplicitCase],
    blobs: list[str],
    reproducer: Reproducer,
    suggestion: str | None = None,
) -> None:
    """The body of :func:`run`, with its settings resolved and its blobs
    checked. ``suggestion`` is as for :func:`_drive_blob_replay`."""
    __tracebackhide__ = True
    _running.depth = getattr(_running, "depth", 0) + 1
    try:
        if blobs:
            _drive_blob_replay(
                test_fn, resolved, database_key, test_location, reproducer, blobs, suggestion
            )
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


def main(
    fn: Callable[[TestCase], Any] | None = None,
    /,
    *,
    settings: Settings | None = None,
    **overrides: Any,
) -> Any:
    """Make a function that takes a :class:`TestCase` the entry point of a
    program, such as an Antithesis workload. Calling the decorated function
    reads the command line and runs one test case.

    ::

        @hegel.main
        def workload(tc):
            n = tc.draw(gs.integers())
            assert n + 0 == n

        if __name__ == "__main__":
            workload()

    A failure is raised as from :func:`test`, and its report ends with a
    ``rerun with: --failure-blob BLOB`` line. The ``too_slow`` and
    ``test_cases_too_large`` health checks are always suppressed.

    Each setting except ``test_cases`` is a command-line option named after
    its ``hegel.toml`` key, with ``-`` in place of ``_``. Values use the same
    format as in ``hegel.toml``, and options override the decorator's settings.
    ``test_cases`` cannot be set.

    ``--profile NAME`` uses the named settings profile in place of the
    default one, and the decorator's keyword settings apply on top of it.
    It does not change a ``settings=`` object. ``--failure-blob BLOB``
    replays a failure. ``--help`` lists the options.

    An invalid command line argument prints a usage message and exits with
    status 2.
    """
    _check_settings(settings)
    if "test_cases" in overrides:
        raise InvalidArgument("@hegel.main only runs one test case, so test_cases cannot be set")

    def decorate(inner: Callable[[TestCase], Any]) -> Callable[..., None]:
        if not callable(inner) or inspect.isclass(inner):
            raise InvalidArgument("@hegel.main must decorate a function")
        if inspect.iscoroutinefunction(inner) or inspect.isasyncgenfunction(inner):
            raise InvalidArgument(
                f"{inner.__name__} is async, and Hegel does not run async tests. Make it "
                "synchronous, running the async code under test with asyncio.run()."
            )
        parameters = list(inspect.signature(inner).parameters.values())
        if not parameters:
            raise InvalidArgument(f"{inner.__name__} must take the test case as its argument")
        if "POSITIONAL" not in parameters[0].kind.name:
            raise InvalidArgument(
                f"{inner.__name__} must take the test case as a positional parameter, but "
                f"{parameters[0].name!r} cannot be passed positionally"
            )
        unsupplied = [
            parameter.name
            for parameter in parameters[1:]
            if parameter.default is parameter.empty
            and parameter.kind not in (parameter.VAR_POSITIONAL, parameter.VAR_KEYWORD)
        ]
        if unsupplied:
            raise InvalidArgument(
                f"{inner.__name__} must take only the test case, but it also takes "
                f"{unsupplied[0]!r}"
            )
        location = _location_of(inner)
        database_key = _database_key_of(inner)
        if location is not None and database_key.startswith("__main__."):
            database_key = (
                f"{os.path.abspath(location.file)}:{database_key.removeprefix('__main__.')}"
            )

        @functools.wraps(inner)
        def wrapper(argv: Sequence[str] | None = None) -> None:
            __tracebackhide__ = True
            if _inside_a_run():
                raise InvalidArgument(
                    f"{inner.__name__} cannot be called from inside a Hegel test. Move its "
                    "body into a function that takes the test case, and call that instead."
                )
            parser = _main_parser()
            args = parser.parse_args(argv)
            base = settings
            if args.profile is not None:
                try:
                    profile = Settings.from_profile(args.profile)
                except InvalidArgument as e:
                    parser.error(f"--profile: {_without_function_name(e)}")
                if base is None:
                    base = profile
            resolved = Settings(base, **overrides)
            for key in _MAIN_OPTIONS:
                value = getattr(args, key)
                if value is None:
                    continue
                try:
                    resolved = resolved._with_string(key, value)
                except InvalidArgument as e:
                    parser.error(f"--{key.replace('_', '-')}: {e}")
            decorator_blobs: list[str] = wrapper.hegel_reproduce_blobs  # type: ignore[attr-defined]
            suggestions = []
            if decorator_blobs:
                suggestions.append(
                    "remove the decorator" if len(decorator_blobs) == 1 else "remove the decorators"
                )
            if args.failure_blob:
                suggestions.append("stop passing --failure-blob")
            _run(
                inner,
                resolved.replace(
                    test_cases=1,
                    suppress_health_check=resolved.suppress_health_check
                    | {HealthCheck.too_slow, HealthCheck.test_cases_too_large},
                ),
                database_key=database_key,
                test_location=location,
                explicit_cases=list(wrapper.hegel_explicit_cases),  # type: ignore[attr-defined]
                blobs=[*decorator_blobs, *([args.failure_blob] if args.failure_blob else [])],
                reproducer=_rerun_with,
                suggestion=" and ".join(suggestions),
            )

        wrapper.__signature__ = inspect.signature(wrapper, follow_wrapped=False)  # type: ignore[attr-defined]
        wrapper.hegel_explicit_cases = list(getattr(inner, "hegel_explicit_cases", []))  # type: ignore[attr-defined]
        wrapper.hegel_reproduce_blobs = list(getattr(inner, "hegel_reproduce_blobs", []))  # type: ignore[attr-defined]
        return wrapper

    if fn is not None:
        if settings is not None or overrides:
            raise InvalidArgument("@hegel.main takes either a function or keyword settings")
        return decorate(fn)
    return decorate


_MAIN_OPTIONS = [key for key in _FIELDS if key != "test_cases"]


def _main_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run one Hegel test case.", allow_abbrev=False)
    parser.add_argument("--profile", metavar="NAME", help="the settings profile to use")
    for key in _MAIN_OPTIONS:
        parser.add_argument(
            f"--{key.replace('_', '-')}",
            dest=key,
            metavar="VALUE",
            help=f"the {key} setting",
        )
    parser.add_argument(
        "--failure-blob",
        metavar="BLOB",
        help="replay the failure that BLOB encodes",
    )
    return parser


def _rerun_with(blob: str) -> str:
    return f"rerun with: --failure-blob {blob}"


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
