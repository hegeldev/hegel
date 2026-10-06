"""Where Hypothesis-style output goes: ``report`` prints a line, and
``with_reporter`` redirects every line (Hegel's run output included)."""

from __future__ import annotations

import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from typing import Any

from hegel.runner import with_output_override

__all__ = [
    "base_report",
    "current_reporter",
    "debug_report",
    "default",
    "report",
    "verbose_report",
    "with_reporter",
]

_local = threading.local()


def default(value: object) -> None:
    try:
        print(value)  # noqa: T201
    except UnicodeEncodeError:
        print(str(value).encode("utf-8"))  # noqa: T201


def current_reporter() -> Callable[[object], Any]:
    return getattr(_local, "reporter", default)


@contextmanager
def with_reporter(new_reporter: Callable[[object], Any]) -> Iterator[None]:
    previous = current_reporter()
    _local.reporter = new_reporter
    try:
        with with_output_override(new_reporter):
            yield
    finally:
        _local.reporter = previous


def report(text: str) -> None:
    current_reporter()(text)


base_report = report


def verbose_report(text: str) -> None:
    """Report ``text`` when the current settings are verbose or debug."""
    from hegel.hypothesis._settings import Verbosity, settings

    if settings.default.verbosity >= Verbosity.verbose:
        report(text)


def debug_report(text: str) -> None:
    """Report ``text`` when the current settings are debug."""
    from hegel.hypothesis._settings import Verbosity, settings

    if settings.default.verbosity >= Verbosity.debug:
        report(text)
