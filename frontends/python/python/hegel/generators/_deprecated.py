"""The date and time generators' released top-level names, kept as
deprecated aliases of :mod:`hegel.generators.temporal`. Each takes the
arguments it was released with; the arguments added since are on the
``temporal`` names only."""

from __future__ import annotations

import datetime as dt
import warnings
import zoneinfo
from typing import Any

from hegel.generators import temporal
from hegel.generators.base import Generator
from hegel.generators.combinators import just

__all__ = ["dates", "datetimes", "timedeltas", "times", "timezones"]

_ZONES_NOTE = ", whose timezones= argument is named zones="


def _message(old: str, new: str, note: str = "") -> str:
    return f"gs.{old}() is deprecated; use gs.temporal.{new}() instead{note}."


def dates(min_value: dt.date | None = None, max_value: dt.date | None = None) -> Generator[dt.date]:
    """Deprecated alias of :func:`hegel.generators.temporal.dates`."""
    warnings.warn(_message("dates", "dates"), DeprecationWarning, stacklevel=2)
    return temporal.dates(min_value, max_value)


def times(
    min_value: dt.time | None = None,
    max_value: dt.time | None = None,
    *,
    timezones: Generator[Any] | None = None,
) -> Generator[dt.time]:
    """Deprecated alias of :func:`hegel.generators.temporal.times`, whose
    ``timezones`` argument is named ``zones`` there."""
    warnings.warn(_message("times", "times", _ZONES_NOTE), DeprecationWarning, stacklevel=2)
    return temporal.times(min_value, max_value, zones=timezones)


def datetimes(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    timezones: Generator[Any] | None = None,
    allow_imaginary: bool = True,
) -> Generator[dt.datetime]:
    """Deprecated alias of :func:`hegel.generators.temporal.datetimes`,
    whose ``timezones`` argument is named ``zones`` there."""
    warnings.warn(_message("datetimes", "datetimes", _ZONES_NOTE), DeprecationWarning, stacklevel=2)
    return temporal.datetimes(
        min_value, max_value, zones=timezones, allow_imaginary=allow_imaginary
    )


def timedeltas(
    min_value: dt.timedelta | None = None, max_value: dt.timedelta | None = None
) -> Generator[dt.timedelta]:
    """Deprecated alias of :func:`hegel.generators.temporal.durations`."""
    warnings.warn(_message("timedeltas", "durations"), DeprecationWarning, stacklevel=2)
    return temporal.durations(min_value, max_value)


def timezones(*, no_cache: bool = False) -> Generator[Any]:
    """Deprecated alias of :func:`hegel.generators.temporal.zones`. Where
    :mod:`zoneinfo` finds no time zone database it generates only
    :data:`datetime.UTC`, as it always has, where
    :func:`~hegel.generators.temporal.zones` raises."""
    warnings.warn(_message("timezones", "zones"), DeprecationWarning, stacklevel=2)
    if not zoneinfo.available_timezones():
        return just(dt.UTC)
    return temporal.zones(no_cache=no_cache)
