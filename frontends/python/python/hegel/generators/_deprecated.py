"""The date and time generators' released top-level names, kept as
deprecated aliases of :mod:`hegel.generators.temporal`."""

from __future__ import annotations

import datetime as dt
import warnings
import zoneinfo
from collections.abc import Iterable
from typing import Any

from hegel.generators import temporal
from hegel.generators.base import Generator

__all__ = ["dates", "datetimes", "timedeltas", "times", "timezones"]

_ZONES_NOTE = ", whose timezones= argument is named zones="


def _message(old: str, new: str, note: str = "") -> str:
    return f"gs.{old}() is deprecated; use gs.temporal.{new}() instead{note}."


def dates(
    min_value: dt.date = dt.date.min,
    max_value: dt.date = dt.date.max,
    *,
    weekdays: Iterable[int] | None = None,
    months: Iterable[int] | None = None,
) -> Generator[dt.date]:
    """Deprecated alias of :func:`hegel.generators.temporal.dates`."""
    warnings.warn(_message("dates", "dates"), DeprecationWarning, stacklevel=2)
    return temporal.dates(min_value, max_value, weekdays=weekdays, months=months)


def times(
    min_value: dt.time = dt.time.min,
    max_value: dt.time = dt.time.max,
    *,
    timezones: Generator[Any] | None = None,
    precision: str = "microseconds",
) -> Generator[dt.time]:
    """Deprecated alias of :func:`hegel.generators.temporal.times`, whose
    ``timezones`` argument is named ``zones`` there."""
    warnings.warn(_message("times", "times", _ZONES_NOTE), DeprecationWarning, stacklevel=2)
    return temporal.times(min_value, max_value, zones=timezones, precision=precision)


def datetimes(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    timezones: Generator[Any] | None = None,
    allow_imaginary: bool = True,
    ambiguous: str = "both",
    precision: str = "microseconds",
) -> Generator[dt.datetime]:
    """Deprecated alias of :func:`hegel.generators.temporal.datetimes`,
    whose ``timezones`` argument is named ``zones`` there."""
    warnings.warn(_message("datetimes", "datetimes", _ZONES_NOTE), DeprecationWarning, stacklevel=2)
    return temporal.datetimes(
        min_value,
        max_value,
        zones=timezones,
        allow_imaginary=allow_imaginary,
        ambiguous=ambiguous,
        precision=precision,
    )


def timedeltas(
    min_value: dt.timedelta = dt.timedelta.min,
    max_value: dt.timedelta = dt.timedelta.max,
    *,
    precision: str = "microseconds",
) -> Generator[dt.timedelta]:
    """Deprecated alias of :func:`hegel.generators.temporal.durations`."""
    warnings.warn(_message("timedeltas", "durations"), DeprecationWarning, stacklevel=2)
    return temporal.durations(min_value, max_value, precision=precision)


def timezones(
    *,
    names: Iterable[str] | None = None,
    no_cache: bool = False,
    links: bool = True,
    legacy: bool = False,
) -> Generator[zoneinfo.ZoneInfo]:
    """Deprecated alias of :func:`hegel.generators.temporal.zones`."""
    warnings.warn(_message("timezones", "zones"), DeprecationWarning, stacklevel=2)
    return temporal.zones(names=names, no_cache=no_cache, links=links, legacy=legacy)
