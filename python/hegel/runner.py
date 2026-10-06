"""The run lifecycle: driving the engine, replaying failures, reporting."""

from __future__ import annotations

import contextlib
import functools
import inspect
import os
import sys
import threading
import traceback
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from typing import Any, TypeVar

from hegel import _native
from hegel.errors import (
    FLAKY_DIAGNOSTIC,
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
)
from hegel.explicit import NO_HANDLE, ExplicitCase, ExplicitTestCase
from hegel.settings import Phase, Settings, TestLocation, Verbosity
from hegel.test_case import OutputSink, TestCase

__all__ = [
    "RunOutput",
    "explicit_test_case",
    "find_example",
    "reproduce_failure",
    "run",
    "test",
    "with_output_override",
]

T = TypeVar("T")

_HEGEL_DIR = os.path.dirname(os.path.abspath(__file__))
_output_override = threading.local()


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
        for line in text.rstrip("\n").split("\n"):
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
    is_final: bool,
    verbosity: Verbosity,
    output: RunOutput,
    case_sink: OutputSink | None,
) -> _CaseOutcome:
    verbose = verbosity in (Verbosity.verbose, Verbosity.debug)
    quiet = verbosity is Verbosity.quiet
    capture_at_discovery = not is_final and handle.is_nondeterministic()
    should_emit = ((is_final or capture_at_discovery) and not quiet) or verbose
    tc = TestCase(handle, should_emit, case_sink or output.resolved_sink())
    outcome = _CaseOutcome(_native.STATUS_VALID, None, None, None)
    try:
        test_fn(tc)
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
    except BaseException as exception:
        outcome.status = _native.STATUS_INTERESTING
        outcome.origin = f"{type(exception).__qualname__} at {_user_location(exception)}"
        outcome.exception = exception
        if (is_final or capture_at_discovery) and not quiet:
            outcome.diagnostic = _format_exception(exception)
            if verbose and capture_at_discovery:
                output.block(outcome.diagnostic)
        elif verbose:
            output.block(_format_exception(exception))
    tc._emit_rendered_output()
    if verbose:
        output.line(_stop_reason(outcome))
    handle.mark_complete(outcome.status, outcome.origin)
    return outcome


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


def _annotate(exception: BaseException, lines: list[str], reproducer: str | None) -> None:
    notes = []
    if lines:
        notes.append("Falsifying example:\n" + "\n".join(lines))
    if reproducer is not None:
        notes.append(reproducer.strip("\n"))
    for note in notes:
        exception.add_note(note)


def _drive(
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
) -> None:
    verbosity = settings.verbosity
    quiet = verbosity is Verbosity.quiet
    verbose = verbosity in (Verbosity.verbose, Verbosity.debug)
    output = RunOutput.resolve()
    native_settings = settings._build(database_key, test_location)
    run = _native.Run.start(native_settings)
    stash: tuple[list[str], str | None, BaseException] | None = None

    def flush() -> None:
        for line in run.drain_output():
            output.line(line)

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

        outcome = _run_test_case(handle, test_fn, False, verbosity, output, case_sink)
        flush()
        if outcome.status == _native.STATUS_INTERESTING and outcome.exception is not None:
            stash = (buffer, outcome.diagnostic, outcome.exception)

    result = run.result()
    flush()
    status = result.status()
    if status == _native.RUN_STATUS_PASSED:
        return
    if status == _native.RUN_STATUS_ERROR:
        message = result.error() or "the run failed with an unknown error"
        if message.startswith("FailedHealthCheck:"):
            raise FailedHealthCheck(message)
        if message.startswith("Flaky test detected"):
            raise Flaky(message)
        raise RunError(message)
    if status == _native.RUN_STATUS_FAILED_NONDETERMINISTIC:
        if stash is None:
            raise InternalError("a failed nondeterministic run has no stashed failure")
        stashed_lines, diagnostic, exception = stash
        for line in stashed_lines:
            output.line(line)
        if diagnostic is not None:
            output.block(diagnostic)
        _annotate(exception, stashed_lines, None)
        raise exception
    count = result.failure_count()
    multiple = count > 1
    if multiple and not quiet:
        output.line(f"Property-based test failed with {count} distinct failures.")
    replay_settings = settings._build(database_key, None)
    exceptions: list[BaseException] = []
    for index in range(count):
        if multiple and not quiet:
            output.line("")
        origin, blob = result.failure(index)
        if blob is None:
            raise InternalError(f"failure {index} has no reproduction blob")
        lines: list[str] = []
        case_output = RunOutput(output.sink)
        collecting = _Collecting(output, lines)
        outcome = _replay_blob(replay_settings, blob, test_fn, verbosity, case_output, collecting)
        if outcome.status != _native.STATUS_INTERESTING or outcome.exception is None:
            raise Flaky(f"{FLAKY_DIAGNOSTIC}\nThe failure that did not reproduce was: {origin}")
        if outcome.diagnostic is not None:
            output.block(outcome.diagnostic)
        reproducer = _reproducer_line(settings, blob)
        if reproducer is not None:
            output.block(reproducer + "\n")
        _annotate(outcome.exception, lines, reproducer)
        exceptions.append(outcome.exception)
    if multiple:
        raise MultipleFailures(
            f"Property-based test failed with {count} distinct failures.",
            exceptions,  # type: ignore[arg-type]
        )
    raise exceptions[0]


class _Collecting:
    """An output sink that both forwards lines and keeps them."""

    __slots__ = ("lines", "output")

    def __init__(self, output: RunOutput, lines: list[str]) -> None:
        self.output = output
        self.lines = lines

    def __call__(self, line: str) -> None:
        self.lines.append(line)
        self.output.line(line)


def _replay_blob(
    native_settings: _native.Settings,
    blob: str,
    test_fn: Callable[[TestCase], Any],
    verbosity: Verbosity,
    output: RunOutput,
    case_sink: OutputSink | None,
) -> _CaseOutcome:
    handle = _native.TestCase.from_blob(native_settings, blob)
    for line in handle.drain_output():
        output.line(line)
    outcome = _run_test_case(handle, test_fn, True, verbosity, output, case_sink)
    for line in handle.drain_output():
        output.line(line)
    return outcome


def _drive_blob_replay(
    test_fn: Callable[[TestCase], Any],
    settings: Settings,
    database_key: str | None,
    test_location: TestLocation | None,
    blob: str,
) -> None:
    output = RunOutput.resolve()
    native_settings = settings._build(database_key, test_location)
    lines: list[str] = []
    outcome = _replay_blob(
        native_settings, blob, test_fn, settings.verbosity, output, _Collecting(output, lines)
    )
    if outcome.diagnostic is not None:
        output.block(outcome.diagnostic)
    if outcome.status == _native.STATUS_INTERESTING and outcome.exception is not None:
        _annotate(outcome.exception, lines, None)
        raise outcome.exception
    raise DidNotReproduce(
        "reproduce_failure: the supplied failure blob no longer reproduces a failure. "
        "The failure may have been fixed, or the blob is stale."
    )


def _run_explicit(
    test_fn: Callable[[TestCase], Any], case: ExplicitCase, output: RunOutput
) -> None:
    explicit = ExplicitTestCase(case)
    tc = TestCase(NO_HANDLE, True, output.resolved_sink(), explicit)
    try:
        test_fn(tc)
    except LoopDone:
        pass
    except (KeyboardInterrupt, SystemExit, InvalidArgument, InternalError):
        raise
    except BaseException as exception:
        output.line(f"Explicit test case {case!r} failed:")
        for line in explicit.lines:
            output.line(line)
        output.block(_format_exception(exception))
        _annotate(exception, explicit.lines, None)
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
    if not callable(test_fn):
        raise InvalidArgument("run() takes a test function")
    resolved = (
        Settings(settings, **overrides) if (settings is not None or overrides) else Settings()
    )
    if reproduce_blob is not None:
        _drive_blob_replay(test_fn, resolved, database_key, test_location, reproduce_blob)
        return
    if Phase.explicit in resolved.phases:
        output = RunOutput.resolve()
        for case in explicit_cases:
            _run_explicit(test_fn, case, output)
    _drive(test_fn, resolved, database_key, test_location)


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
        signature = inspect.signature(inner)
        parameters = list(signature.parameters.values())
        if not parameters:
            raise InvalidArgument(f"{inner.__name__} must take the test case as its first argument")
        location = _location_of(inner)
        database_key = _database_key_of(inner)
        base_settings = settings

        @functools.wraps(inner)
        def wrapper(*args: Any, **kwargs: Any) -> None:
            run(
                lambda tc: inner(tc, *args, **kwargs),
                base_settings,
                database_key=database_key,
                test_location=location,
                explicit_cases=list(wrapper.hegel_explicit_cases),  # type: ignore[attr-defined]
                reproduce_blob=wrapper.hegel_reproduce_blob,  # type: ignore[attr-defined]
                **overrides,
            )

        wrapper.__signature__ = signature.replace(parameters=parameters[1:])  # type: ignore[attr-defined]
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


def find_example(generator: Any, condition: Callable[[Any], bool] | None = None) -> Any:
    """Find a minimal value from ``generator`` satisfying ``condition`` (any
    value when no condition is given), outside of any test. The search
    runs like a test whose failure is finding the value, so the result is
    shrunk."""
    from hegel.generators.combinators import as_generator

    generator = as_generator(generator)
    found: list[Any] = []

    def body(tc: TestCase) -> None:
        value = tc.draw_silent(generator)
        if condition is not None and not condition(value):
            tc.reject()
        found.append(value)
        raise _Found()

    try:
        with with_output_override(lambda line: None):
            run(
                body,
                Settings(
                    database="",
                    verbosity=Verbosity.quiet,
                    phases=[Phase.generate, Phase.shrink],
                    test_cases=1000,
                    print_blob=False,
                ),
            )
    except _Found:
        return found[-1]
    raise InvalidArgument("Could not find an example satisfying the condition")


class _Found(Exception):
    pass
