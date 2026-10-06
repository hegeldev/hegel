"""Functions usable inside a test body: ``assume``, ``note``, ``event``,
``target`` and ``reject``, plus the build context they act through."""

from __future__ import annotations

import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from typing import TYPE_CHECKING, Any, NoReturn

from hegel.hypothesis.errors import InvalidArgument, UnsatisfiedAssumption

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "BuildContext",
    "assume",
    "cleanup",
    "current_build_context",
    "currently_in_test_context",
    "event",
    "note",
    "reject",
    "target",
]


class BuildContext:
    """The state of the test case currently being executed."""

    def __init__(self, tc: TestCase, *, is_final: bool = False, runner: Any = None) -> None:
        self.tc = tc
        self.is_final = is_final
        self.runner = runner
        self.tasks: list[Callable[[], Any]] = []
        self.shared: dict[Any, Any] = {}
        self.random_seeds: list[int] = []
        self.known_object_printers: dict[int, str] = {}

    def __enter__(self) -> BuildContext:
        _stack().append(self)
        return self

    def __exit__(self, *exc: object) -> None:
        stack = _stack()
        assert stack and stack[-1] is self
        stack.pop()
        self.close()

    def close(self) -> None:
        tasks, self.tasks = self.tasks, []
        errors: list[BaseException] = []
        for task in reversed(tasks):
            try:
                task()
            except BaseException as error:  # noqa: BLE001
                errors.append(error)
        if errors:
            if len(errors) == 1:
                raise errors[0]
            raise BaseExceptionGroup("Cleanup failed", errors)


_local = threading.local()


def _stack() -> list[BuildContext]:
    stack = getattr(_local, "stack", None)
    if stack is None:
        stack = _local.stack = []
    return stack


def currently_in_test_context() -> bool:
    """Whether a Hypothesis-style test body is running on this thread."""
    return bool(_stack())


def current_build_context() -> BuildContext:
    stack = _stack()
    if not stack:
        raise InvalidArgument("No build context registered")
    return stack[-1]


def _current_tc(what: str) -> TestCase:
    stack = _stack()
    if not stack:
        raise InvalidArgument(f"Cannot {what} outside of a test")
    return stack[-1].tc


def reject() -> NoReturn:
    """Reject the current test case as invalid."""
    raise UnsatisfiedAssumption()


def assume(condition: object) -> bool:
    """Reject the current test case unless ``condition`` holds."""
    if not condition:
        raise UnsatisfiedAssumption()
    return True


def cleanup(teardown: Callable[[], Any]) -> None:
    """Register a function to run at the end of the current test case."""
    context = _stack()
    if not context:
        raise InvalidArgument("Cannot register cleanup outside of a build context")
    context[-1].tasks.append(teardown)


def note(value: object) -> None:
    """Report ``value`` as part of the failing test case's output."""
    tc = _current_tc("make notes")
    tc.note(value if isinstance(value, str) else repr(value))


def event(value: str, payload: Any = "") -> None:
    """Record an event for the run's statistics."""
    if not isinstance(value, str):
        raise InvalidArgument(f"Expected str but got {value!r} (type={type(value).__name__})")
    tc = _current_tc("make record an event")
    if payload == "":
        tc.event(value)
    elif isinstance(payload, (int, float)) and not isinstance(payload, bool):
        tc.event_value(value, float(payload))
    else:
        tc.event(f"{value}: {payload}")


def target(observation: int | float, *, label: str = "") -> int | float:
    """Guide generation toward maximising ``observation``."""
    if isinstance(observation, bool) or not isinstance(observation, (int, float)):
        raise InvalidArgument(
            f"Expected an int or float but got observation={observation!r} "
            f"(type={type(observation).__name__})"
        )
    if observation != observation or observation in (float("inf"), float("-inf")):
        raise InvalidArgument(f"observation={observation!r} must be a finite float.")
    if not isinstance(label, str):
        raise InvalidArgument(f"label={label!r} must be a string")
    tc = _current_tc("target")
    tc.target(float(observation), label)
    return observation


@contextmanager
def _context_for(tc: TestCase, **kwargs: Any) -> Iterator[BuildContext]:
    with BuildContext(tc, **kwargs) as context:
        yield context
