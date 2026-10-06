"""Where Hypothesis-style output goes: ``report`` prints a line, and
``with_reporter`` redirects every line (Hegel's run output included)."""

from __future__ import annotations

import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from typing import Any

from hegel.runner import with_output_override

__all__ = ["base_report", "current_reporter", "default", "report", "with_reporter"]

_local = threading.local()


def default(value: object) -> None:
    print(value)  # noqa: T201


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
