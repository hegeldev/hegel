"""Deprecation warnings in the Hypothesis style."""

from __future__ import annotations

import datetime
import warnings

from hegel.hypothesis.errors import HypothesisDeprecationWarning


def note_deprecation(message: str, *, since: str, has_codemod: bool, stacklevel: int = 0) -> None:
    """Warn about deprecated usage, as Hypothesis does: ``since`` is the ISO
    date of the deprecation (or ``"RELEASEDAY"`` before release)."""
    if since != "RELEASEDAY":
        date = datetime.date.fromisoformat(since)
        assert datetime.date(2021, 1, 1) <= date
    if has_codemod:
        message += (
            "\n    The `hypothesis codemod` command-line tool can automatically "
            "refactor your code to fix this warning."
        )
    warnings.warn(HypothesisDeprecationWarning(message), stacklevel=2 + stacklevel)
