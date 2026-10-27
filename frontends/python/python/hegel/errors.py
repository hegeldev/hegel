"""Exceptions raised by Hegel."""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any

from hegel._native import (
    AssumeFailed,
    BackendError,
    HegelError,
    InternalError,
    InvalidArgument,
    StopTest,
)

if TYPE_CHECKING:
    from hegel.settings import HealthCheck

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
    "NotPrintable",
    "ResolutionFailed",
    "RunError",
    "StopTest",
    "Unsatisfiable",
]


class ResolutionFailed(InvalidArgument):
    """:func:`~hegel.generators.from_type` could not find a generator for a
    type. Raised when the generator is built; a subclass of
    :class:`InvalidArgument`."""


class NotPrintable(InvalidArgument):
    """:meth:`TestCase.draw <hegel.TestCase.draw>` was given a generator
    that cannot print the values it draws: a
    :class:`~hegel.generators.Generator` subclass that implements only
    ``do_draw``, or a structure containing one. Raised from the draw, on
    the first test case that reaches it; a subclass of
    :class:`InvalidArgument`, so it ends the run as a usage error. Implement
    ``do_draw_and_print`` on the generator, make it printable with
    :meth:`~hegel.generators.Generator.print_with` or
    :meth:`~hegel.generators.Generator.print_as_repr`, or draw it with
    :meth:`TestCase.draw_silent <hegel.TestCase.draw_silent>`."""


class LoopDone(BaseException):
    """Control flow: a ``tc.repeat`` loop has run all of its iterations."""


class RunError(HegelError):
    """The engine aborted the run rather than reporting a pass or a test
    failure: a failed health check (:class:`FailedHealthCheck`),
    nondeterminism under the ``error`` strictness (:class:`Flaky`), too few
    valid test cases (:class:`Unsatisfiable`), or another engine error.
    The message explains which."""


class FailedHealthCheck(RunError):
    """A health check failed: the run was likely to test poorly or slowly
    (too many rejected test cases, say). ``health_check`` is the
    :class:`~hegel.HealthCheck` that failed, and the message names it and
    says how to suppress it with ``suppress_health_check`` if the
    behaviour is intentional."""

    def __init__(self, message: str, health_check: HealthCheck | None = None) -> None:
        super().__init__(message)
        self.health_check = health_check


class Flaky(RunError):
    """The engine aborted the run because the test behaved
    nondeterministically, under ``NondeterminismStrictness.error``. The
    default strictness instead confirms such failures by repeated replay
    and reports them with a caveat."""


class Unsatisfiable(RunError):
    """The engine could not find enough inputs satisfying the test's
    assumptions and filters. With the ``filter_too_much`` health check
    enabled, a run that rejects nearly everything fails that check
    (:class:`FailedHealthCheck`) first."""


class DidNotReproduce(HegelError):
    """The blob given to :func:`~hegel.reproduce_failure` no longer makes
    the test fail: the bug is fixed (remove the decorator) or the test has
    changed so the blob means something else."""


class MultipleFailures(BaseExceptionGroup):  # type: ignore[type-arg]
    """A run found several distinct failures, with
    ``report_multiple_failures=True``. Each failure is in ``exceptions``,
    with its own falsifying example attached as notes.

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
