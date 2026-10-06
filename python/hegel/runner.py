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
from typing import Any, TypeVar

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
from hegel.settings import Phase, Settings, TestLocation, Verbosity
from hegel.test_case import OutputSink, TestCase

__all__ = [
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
    diagnostic: str | None


def _user_location(exception: BaseException) -> str:
    """``file:line`` of the innermost traceback frame outside Hegel."""
    location = None
    for frame in traceback.extract_tb(exception.__traceback__):
        if location is None or not _inside_hegel(frame.filename):
            location = f"{frame.filename}:{frame.lineno}"
    return location or "<unknown>"


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
    stamped case emits its draws and notes and, if it fails, carries its
    rendered traceback for the deferred report. Other cases run silently
    unless the run is verbose.
    """
    verbose = verbosity in (Verbosity.verbose, Verbosity.debug)
    quiet = verbosity is Verbosity.quiet
    stamped = handle.should_capture()
    should_emit = (stamped and not quiet) or verbose
    tc = TestCase(handle, should_emit, case_sink or output.resolved_sink(), is_final=stamped)
    outcome = _CaseOutcome(_native.STATUS_VALID, None, None, None)
    try:
        _check_not_awaitable(test_fn(tc))
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
        outcome.origin = f"{type(exception).__qualname__} at {_user_location(exception)}"
        outcome.exception = exception
        if stamped and not quiet:
            outcome.diagnostic = _format_exception(exception)
            if verbose:
                output.block(outcome.diagnostic)
        elif verbose:
            output.block(_format_exception(exception))
    tc._emit_rendered_output()
    if verbose:
        output.line(_stop_reason(outcome))
    handle.mark_complete(outcome.status, outcome.origin)
    return outcome


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


def _stop_reason(outcome: _CaseOutcome) -> str:
    if outcome.status == _native.STATUS_VALID:
        return "Test case passed"
    if outcome.status == _native.STATUS_INVALID:
        return "Test case rejected: assumption failed"
    if outcome.status == _native.STATUS_OVERRUN:
        return "Test case stopped: the engine abandoned it"
    return f"Test case failed: {outcome.origin}"


def _reproducer_line(settings: Settings, blob: str | None) -> str | None:
    if not settings.print_blob or blob is None:
        return None
    return (
        "\nTo reproduce this failure, decorate the test (above @hegel.test) with:\n"
        f'    @hegel.reproduce_failure("{blob}")'
    )


def _annotate(
    exception: BaseException, lines: list[str], caveat: str | None, reproducer: str | None
) -> None:
    notes = []
    if lines:
        notes.append("Falsifying example:\n" + "\n".join(lines))
    if caveat is not None:
        notes.append(caveat)
    if reproducer is not None:
        notes.append(reproducer.strip("\n"))
    for note in notes:
        exception.add_note(note)


@dataclass
class _Captured:
    """One origin's failure report material, captured as the case ran:
    its buffered draw and note lines (empty under quiet verbosity), its
    rendered traceback (only for a stamped case), and the exception to
    re-raise. Captures are ranked so that an unstamped shrink probe's bare
    capture cannot replace a stamped one, while at equal rank the newest
    wins: final replay over confirmation over discovery."""

    lines: list[str]
    diagnostic: str | None
    exception: BaseException

    @property
    def rank(self) -> int:
        if self.diagnostic is not None:
            return 2
        return 1 if self.lines else 0


def _drive(
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
) -> None:
    output = RunOutput.resolve()
    native_settings = settings._build(database_key, test_location)
    run = _native.Run.start(native_settings)
    _drive_run(run, test_fn, settings, output, None)


def _drive_blob_replay(
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
    blob: str,
) -> None:
    """Replay a reproduce blob: the engine replays it until a replay fails,
    under a bounded budget, and a run with no failures means the blob did
    not reproduce."""
    output = RunOutput.resolve()
    native_settings = settings._build(database_key, test_location)
    run = _native.Run.start_blob(native_settings, blob)
    _drive_run(
        run,
        test_fn,
        settings,
        output,
        "reproduce_failure: the supplied failure blob did not reproduce a failure. "
        "The failure may have been fixed, or, for a nondeterministic blob, it may not "
        "have recurred within the replay budget (a bug failing 10% of the time escapes "
        "it about 5% of the time). Re-run to retry, or remove the decorator once the "
        "failure is fixed.",
    )


def _drive_run(
    run: _native.Run,
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    output: RunOutput,
    stale_message: str | None,
) -> None:
    """Pull every test case off ``run``, capturing report material per
    failure origin, then act on the verdict. The engine owns the whole
    exploration, the final replay of each reported failure included, so
    the report is assembled from the freshest stamped capture of each
    origin. A passing run returns, unless ``stale_message`` is set (a blob
    replay), where a pass means the blob did not reproduce."""
    verbosity = settings.verbosity
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
            report = _Captured(buffer, outcome.diagnostic, outcome.exception)
            origin = outcome.origin or ""
            stored = captured.get(origin)
            if stored is None or report.rank >= stored.rank:
                captured[origin] = report

    result = run.result()
    flush()
    status = result.status()
    if status == _native.RUN_STATUS_PASSED:
        if stale_message is not None:
            raise DidNotReproduce(stale_message)
        return
    if status == _native.RUN_STATUS_ERROR:
        message = result.error() or "the run failed with an unknown error"
        if message.startswith("FailedHealthCheck:"):
            raise FailedHealthCheck(message)
        if message.startswith(("Flaky test detected", "Your data generation is non-deterministic")):
            raise Flaky(message)
        if message.startswith("Unsatisfiable"):
            raise Unsatisfiable(message)
        raise RunError(message)
    count = result.failure_count()
    multiple = count > 1
    if multiple and not quiet:
        output.line(f"Property-based test failed with {count} distinct failures.")
    exceptions: list[BaseException] = []
    for index in range(count):
        if multiple and not quiet:
            output.line("")
        origin, blob, caveat = result.failure(index)
        if origin not in captured:
            raise InternalError(f"failure {index} ({origin}) was never seen by the test")
        report = captured.pop(origin)
        output.lines(report.lines)
        if report.diagnostic is not None:
            output.block(report.diagnostic)
        if quiet:
            caveat = None
        elif caveat is not None:
            output.line(f"note: {caveat}")
        reproducer = _reproducer_line(settings, blob)
        if reproducer is not None:
            output.block(reproducer + "\n")
        _annotate(report.exception, report.lines, caveat, reproducer)
        exceptions.append(report.exception)
    if multiple:
        raise MultipleFailures(
            f"Property-based test failed with {count} distinct failures.",
            exceptions,  # type: ignore[arg-type]
        )
    raise exceptions[0]


def _run_explicit(
    test_fn: Callable[[TestCase], Any], case: ExplicitCase, output: RunOutput
) -> None:
    explicit = ExplicitTestCase(case)
    tc = TestCase(NO_HANDLE, True, output.resolved_sink(), explicit, is_final=True)
    try:
        _check_not_awaitable(test_fn(tc))
    except LoopDone:
        pass
    except (KeyboardInterrupt, SystemExit, InvalidArgument, InternalError):
        raise
    except BaseException as exception:
        output.line(f"Explicit test case {case!r} failed:")
        output.lines(explicit.lines)
        output.block(_format_exception(exception))
        _annotate(exception, explicit.lines, None, None)
        raise
    explicit.check_consumed()


def run(
    test_fn: Callable[[TestCase], Any],
    settings: Settings | None = None,
    *,
    database_key: str | None = None,
    test_location: TestLocation | None = None,
    explicit_cases: Iterator[ExplicitCase] | list[ExplicitCase] | tuple[ExplicitCase, ...] = (),
    reproduce_blob: str | None = None,
    **overrides: Any,
) -> None:
    """Run ``test_fn`` as a property test: it is called once per test
    case with a :class:`TestCase`, and the first exception it raises (other
    than a rejected assumption) is a failure, reported after shrinking.

    ``settings`` (with ``overrides`` applied) configures the run.
    ``explicit_cases`` run first, with their given values; ``reproduce_blob``
    replays one previously reported failure instead of generating.
    """
    __tracebackhide__ = True
    if not callable(test_fn):
        raise InvalidArgument("run() takes a test function")
    resolved = (
        Settings(settings, **overrides) if (settings is not None or overrides) else Settings()
    )
    _running.depth = getattr(_running, "depth", 0) + 1
    try:
        if reproduce_blob is not None:
            _drive_blob_replay(test_fn, resolved, database_key, test_location, reproduce_blob)
        if Phase.explicit in resolved.phases:
            output = RunOutput.resolve()
            for case in explicit_cases:
                _run_explicit(test_fn, case, output)
        _drive(test_fn, resolved, database_key, test_location)
    finally:
        _running.depth -= 1


def _inside_a_run() -> bool:
    return getattr(_running, "depth", 0) > 0


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
    argument so that calling it (as pytest does) runs the property test.

    Any further parameters are passed through, so pytest fixtures work.
    Settings may be given as ``settings=`` or as individual fields, e.g.
    ``@hegel.test(test_cases=500)``.
    """

    def decorate(inner: Callable[..., Any]) -> Callable[..., Any]:
        if not callable(inner):
            raise InvalidArgument("@hegel.test must decorate a function")
        if inspect.iscoroutinefunction(inner) or inspect.isasyncgenfunction(inner):
            raise InvalidArgument(
                f"{inner.__name__} is async, and Hegel does not run async tests. Make it "
                "synchronous, running the async code under test with asyncio.run()."
            )
        signature = inspect.signature(inner)
        parameters = list(signature.parameters.values())
        if not parameters:
            raise InvalidArgument(f"{inner.__name__} must take the test case as its first argument")
        location = _location_of(inner)
        database_key = _database_key_of(inner)
        base_settings = settings
        # A method takes the test case after ``self``.
        tc_index = 1 if parameters[0].name in ("self", "cls") and len(parameters) > 1 else 0

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
                reproduce_blob=wrapper.hegel_reproduce_blob,  # type: ignore[attr-defined]
                **overrides,
            )

        wrapper.__signature__ = outer_signature  # type: ignore[attr-defined]
        wrapper.hegel_inner_test = inner  # type: ignore[attr-defined]
        wrapper.hegel_explicit_cases = list(getattr(inner, "hegel_explicit_cases", []))  # type: ignore[attr-defined]
        wrapper.hegel_reproduce_blob = getattr(inner, "hegel_reproduce_blob", None)  # type: ignore[attr-defined]
        wrapper.is_hegel_test = True  # type: ignore[attr-defined]
        return wrapper

    if fn is not None:
        if settings is not None or overrides:
            raise InvalidArgument("@hegel.test takes either a function or keyword settings")
        return decorate(fn)
    return decorate


def explicit_test_case(*args: Any, **kwargs: Any) -> Callable[[T], T]:
    """Add an explicit test case: the test runs with these values before
    any are generated. Positional values are consumed by draws in order;
    keyword values by draw name (``x=1`` for ``x = tc.draw(...)``)."""
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
    """Replay the reproduction blob printed for a failure instead of
    generating test cases. Remove the decorator once the bug is fixed."""
    if not isinstance(blob, str) or not blob:
        raise InvalidArgument("reproduce_failure takes the blob string printed with a failure")

    def decorate(fn: T) -> T:
        fn.hegel_reproduce_blob = blob  # type: ignore[attr-defined]
        return fn

    return decorate
