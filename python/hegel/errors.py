"""Exceptions raised by Hegel."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any

from hegel._native import (
    AssumeFailed,
    BackendError,
    HegelError,
    InternalError,
    InvalidArgument,
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
    "MultipleFailures",
    "ResolutionFailed",
    "RunError",
    "StopTest",
    "Unsatisfiable",
]


class ResolutionFailed(InvalidArgument):
    """``from_type`` could not find a generator for a type."""


class LoopDone(BaseException):
    """Control flow: a ``tc.repeat`` loop has run all of its iterations."""


class RunError(HegelError):
    """The engine aborted the run: a failed health check, nondeterminism, or
    an internal engine error."""


class FailedHealthCheck(RunError):
    """A health check failed. The message explains which one, and how to
    suppress it if the behaviour is intentional."""


class Flaky(RunError):
    """The engine aborted the run because the test behaved
    nondeterministically, under ``NondeterminismStrictness.error``. The
    default strictness instead confirms such failures by repeated replay
    and reports them with a caveat."""


class Unsatisfiable(RunError):
    """The engine could not find enough inputs satisfying the test's
    assumptions."""


class DidNotReproduce(HegelError):
    """A ``reproduce_failure`` blob no longer reproduces a failure."""


class MultipleFailures(BaseExceptionGroup):  # type: ignore[type-arg]
    """A run found several distinct failures (``report_multiple_failures``).

    Like the built-in exception groups, an instance whose failures are all
    :class:`Exception` s is itself an :class:`ExceptionGroup`, so
    ``except Exception`` catches it; one with a :class:`BaseException`
    among them is only a :class:`BaseExceptionGroup`."""

    def __new__(cls, message: str, exceptions: Any) -> MultipleFailures:
        if cls is MultipleFailures and all(isinstance(e, Exception) for e in exceptions):
            cls = _MultipleExceptions
        return super().__new__(cls, message, exceptions)  # type: ignore[no-any-return]

    def derive(self, excs: Sequence[BaseException], /) -> MultipleFailures:  # type: ignore[override]
        """The group ``split`` and ``subgroup`` (and so ``except*`` and
        ``contextlib.suppress``) build from a subset of the failures: another
        ``MultipleFailures``, where the default would be a plain group."""
        return MultipleFailures(self.message, excs)


class _MultipleExceptions(MultipleFailures, ExceptionGroup):  # type: ignore[misc]
    pass
