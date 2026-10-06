"""Exceptions raised by Hegel."""

from __future__ import annotations

from hegel._native import (
    AssumeFailed,
    BackendError,
    HegelError,
    InternalError,
    InvalidArgument,
    RetrySignal,
    StopTest,
)

__all__ = [
    "AssumeFailed",
    "BackendError",
    "DidNotReproduce",
    "FailedHealthCheck",
    "Flaky",
    "HegelError",
    "InternalError",
    "InvalidArgument",
    "LoopDone",
    "MultipleFailures",
    "RetrySignal",
    "RunError",
    "StopTest",
]


class LoopDone(BaseException):
    """Control flow: a ``tc.repeat`` loop has run all of its iterations."""


class RunError(HegelError):
    """The engine aborted the run: a failed health check, nondeterminism, or
    an internal engine error."""


class FailedHealthCheck(RunError):
    """A health check failed. The message explains which one, and how to
    suppress it if the behaviour is intentional."""


class Flaky(RunError):
    """A failure found during generation did not reproduce when replayed."""


class DidNotReproduce(HegelError):
    """A ``reproduce_failure`` blob no longer reproduces a failure."""


class MultipleFailures(ExceptionGroup):  # type: ignore[type-arg]
    """A run found several distinct failures (``report_multiple_failures``)."""


FLAKY_DIAGNOSTIC = (
    "Flaky test detected: Your test produced different outcomes when run with "
    "the same generated data - it failed when it previously succeeded, or "
    "succeeded when it previously failed. This usually means your test depends "
    "on external state such as global variables, system time, or external "
    "random number generators."
)
