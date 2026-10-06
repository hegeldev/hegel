"""Hypothesis's exception hierarchy, mapped onto Hegel's where the two
describe the same event."""

from __future__ import annotations

from datetime import timedelta

from hegel.errors import (
    AssumeFailed,
    DidNotReproduce,
    FailedHealthCheck,
    Flaky,
    HegelError,
    InvalidArgument,
    StopTest,
)

__all__ = [
    "BackendCannotProceed",
    "ChoiceTooLarge",
    "DeadlineExceeded",
    "DidNotReproduce",
    "FailedHealthCheck",
    "Flaky",
    "FlakyFailure",
    "FlakyReplay",
    "FlakyStrategyDefinition",
    "Found",
    "Frozen",
    "HypothesisDeprecationWarning",
    "HypothesisException",
    "HypothesisSideeffectWarning",
    "HypothesisWarning",
    "InvalidArgument",
    "InvalidDefinition",
    "InvalidState",
    "MultipleFailures",
    "NoSuchExample",
    "NonInteractiveExampleWarning",
    "NonRoundTrippableCharactersWarning",
    "ResolutionFailed",
    "SmallSearchSpaceWarning",
    "StopTest",
    "Unsatisfiable",
    "UnsatisfiedAssumption",
]

#: The base of Hypothesis's exceptions is Hegel's.
HypothesisException = HegelError

#: ``assume(False)`` raises Hegel's assumption signal.
UnsatisfiedAssumption = AssumeFailed


class NoSuchExample(HypothesisException):
    """``find`` found no example satisfying the condition."""

    def __init__(self, condition: object, extra: str = "") -> None:
        if isinstance(condition, str):
            desc = condition
        else:
            desc = getattr(condition, "__name__", repr(condition))
        super().__init__(f"No examples found of condition {desc}{extra}")


class Unsatisfiable(HypothesisException):
    """Too few valid examples could be generated."""


class ChoiceTooLarge(HypothesisException):
    pass


class FlakyReplay(Flaky):
    pass


class FlakyStrategyDefinition(Flaky):
    pass


class FlakyFailure(ExceptionGroup, Flaky):  # type: ignore[type-arg]
    pass


class ResolutionFailed(InvalidArgument):
    """``from_type`` could not resolve a type to a strategy."""


class InvalidState(HypothesisException):
    pass


class InvalidDefinition(HypothesisException, TypeError):
    """A state machine or strategy was defined incorrectly."""


class HypothesisWarning(HypothesisException, Warning):
    pass


class NonInteractiveExampleWarning(HypothesisWarning):
    pass


class HypothesisDeprecationWarning(HypothesisWarning, FutureWarning):
    pass


class HypothesisSideeffectWarning(HypothesisWarning):
    pass


class SmallSearchSpaceWarning(HypothesisWarning):
    pass


class NonRoundTrippableCharactersWarning(HypothesisWarning):
    pass


class Frozen(HypothesisException):
    pass


class DeadlineExceeded(HypothesisException):
    """A test case took longer than the configured deadline."""

    def __init__(self, runtime: float | timedelta, deadline: float | timedelta) -> None:
        if isinstance(runtime, timedelta):
            runtime = runtime.total_seconds()
        if isinstance(deadline, timedelta):
            deadline = deadline.total_seconds()
        super().__init__(
            f"Test took {runtime * 1000:.2f}ms, which exceeds the deadline of "
            f"{deadline * 1000:.2f}ms"
        )
        self.runtime = runtime
        self.deadline = deadline

    def __reduce__(self) -> tuple[object, ...]:
        return (type(self), (self.runtime, self.deadline))


class Found(HypothesisException):
    pass


class BackendCannotProceed(HypothesisException):
    pass


# Hypothesis reports several distinct failures as an exception group.
MultipleFailures = BaseExceptionGroup
