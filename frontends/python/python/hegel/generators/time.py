"""Date and time generators."""

from __future__ import annotations

import datetime as dt
import zoneinfo
from collections.abc import Iterable
from functools import cache
from typing import TYPE_CHECKING, Any

from hegel import _native
from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator
from hegel.generators.combinators import as_generator

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "dates",
    "datetimes",
    "timedeltas",
    "times",
    "timezone_keys",
    "timezones",
    "utc_offsets",
]

DATE_LABEL = label_from_name("hegel.dates")
TIME_LABEL = label_from_name("hegel.times")
DATETIME_LABEL = label_from_name("hegel.datetimes")
TIMEDELTA_LABEL = label_from_name("hegel.timedeltas")
TIMEZONE_KEY_LABEL = label_from_name("hegel.timezone_keys")
TIMEZONE_LABEL = label_from_name("hegel.timezones")
UTC_OFFSET_LABEL = label_from_name("hegel.utc_offsets")

_EPOCH = dt.datetime(1970, 1, 1, tzinfo=dt.UTC)
_MICROSECOND = dt.timedelta(microseconds=1)
_SECOND = dt.timedelta(seconds=1)
_MAX_OFFSET = dt.timedelta(hours=23, minutes=59, seconds=59)

_PRECISIONS: dict[str, tuple[int, int]] = {
    "microseconds": (_native.PRECISION_MICROSECONDS, 1),
    "milliseconds": (_native.PRECISION_MILLISECONDS, 1_000),
    "seconds": (_native.PRECISION_SECONDS, 1_000_000),
    "minutes": (_native.PRECISION_MINUTES, 60_000_000),
    "hours": (_native.PRECISION_HOURS, 3_600_000_000),
}
_GRANULARITIES: dict[str, tuple[int, int]] = {
    "hours": (_native.GRANULARITY_HOURS, 3600),
    "quarter_hours": (_native.GRANULARITY_QUARTER_HOURS, 900),
    "minutes": (_native.GRANULARITY_MINUTES, 60),
    "seconds": (_native.GRANULARITY_SECONDS, 1),
}
_AMBIGUOUS: dict[str, int] = {
    "both": _native.AMBIGUOUS_BOTH,
    "earlier": _native.AMBIGUOUS_EARLIER,
    "later": _native.AMBIGUOUS_LATER,
}


def _check_type(name: str, value: Any, cls: type) -> None:
    if not isinstance(value, cls):
        raise InvalidArgument(f"{name}={value!r} must be a {cls.__name__}")


def _check_choice(name: str, value: Any, choices: Iterable[str]) -> None:
    if not isinstance(value, str) or value not in choices:
        options = ", ".join(repr(choice) for choice in choices)
        raise InvalidArgument(f"{name}={value!r} must be one of {options}")


def _precision(precision: Any) -> tuple[int, int]:
    """The engine constant and the unit in microseconds of a ``precision``
    argument, one of the ``timespec`` values of ``isoformat``."""
    _check_choice("precision", precision, _PRECISIONS)
    return _PRECISIONS[precision]


def _check_fits(unit_name: str, min_units: int, max_units: int, unit: int, what: str) -> None:
    """Raise unless some whole multiple of ``unit`` lies in
    ``[min_units, max_units]``."""
    if -(-min_units // unit) * unit > max_units:
        raise InvalidArgument(
            f"no {what} between min_value and max_value is a whole number of {unit_name}"
        )


def _mask(name: str, values: Any, lowest: int, highest: int) -> int:
    """A bit set over ``values``, each an int in ``[lowest, highest]``, with
    bit 0 for ``lowest``."""
    if isinstance(values, (str, bytes)) or not isinstance(values, Iterable):
        raise InvalidArgument(f"{name}={values!r} must be an iterable of ints")
    mask = 0
    for value in values:
        if not isinstance(value, int) or isinstance(value, bool) or not lowest <= value <= highest:
            raise InvalidArgument(
                f"{name}={values!r} must contain ints from {lowest} to {highest}, not {value!r}"
            )
        mask |= 1 << (value - lowest)
    if mask == 0:
        raise InvalidArgument(f"{name}={values!r} must not be empty")
    return mask


class Dates(Generator[dt.date]):
    def __init__(
        self, min_value: dt.date, max_value: dt.date, weekdays: int | None, months: int | None
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.weekdays = weekdays
        self.months = months
        self.options = (
            None if weekdays is None and months is None else _native.DateOptions(weekdays, months)
        )
        self.label = DATE_LABEL

    def allows(self, value: dt.date) -> bool:
        return (self.weekdays is None or bool(self.weekdays >> value.weekday() & 1)) and (
            self.months is None or bool(self.months >> (value.month - 1) & 1)
        )

    def do_draw(self, tc: TestCase) -> dt.date:
        return tc._generate_date(self.min_value, self.max_value, self.options)

    def __repr__(self) -> str:
        return f"dates(min_value={self.min_value!r}, max_value={self.max_value!r})"


def dates(
    min_value: dt.date = dt.date.min,
    max_value: dt.date = dt.date.max,
    *,
    weekdays: Iterable[int] | None = None,
    months: Iterable[int] | None = None,
) -> Generator[dt.date]:
    """Generate :class:`datetime.date` objects in ``[min_value, max_value]``,
    inclusive, shrinking toward 2000-01-01 (or the bound nearest it).

    The bounds default to the whole range :mod:`datetime` supports. One
    draw in twenty on average targets a date tests tend to get wrong: the
    leap days nearest the bounds, the year boundaries, the 2038 and 2106
    overflow days and the days of an ISO week 53.

    ``weekdays`` restricts the draw to those days of the week, numbered as
    :meth:`datetime.date.weekday` does (Monday is 0, Sunday 6, so
    ``weekdays=[5, 6]`` draws weekends), and ``months`` to those months,
    numbered 1 to 12. A drawn date outside them is moved to the nearest
    allowed date inside the bounds rather than rejected, so the draw never
    filters, and the dates still shrink toward 2000-01-01.

    Bounds that are not dates (a ``datetime`` is not accepted),
    ``min_value > max_value``, an empty or out-of-range ``weekdays`` or
    ``months``, or bounds holding no allowed date are an
    :class:`~hegel.InvalidArgument`.
    """
    _check_type("min_value", min_value, dt.date)
    _check_type("max_value", max_value, dt.date)
    if isinstance(min_value, dt.datetime) or isinstance(max_value, dt.datetime):
        raise InvalidArgument("dates() bounds must be date objects, not datetimes")
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    weekday_mask = None if weekdays is None else _mask("weekdays", weekdays, 0, 6)
    month_mask = None if months is None else _mask("months", months, 1, 12)
    result = Dates(min_value, max_value, weekday_mask, month_mask)
    if result.options is not None:
        candidate = min_value
        while not result.allows(candidate):
            if candidate == max_value or candidate - min_value > dt.timedelta(days=400):
                raise InvalidArgument(
                    f"no date between min_value={min_value!r} and max_value={max_value!r} "
                    f"falls on weekdays={weekdays!r} and months={months!r}"
                )
            candidate += dt.timedelta(days=1)
    return result


class Times(Generator[dt.time]):
    def __init__(
        self,
        min_value: dt.time,
        max_value: dt.time,
        timezones: Generator[Any] | None,
        precision: int,
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.timezones = timezones
        self.precision = precision
        self.label = (
            TIME_LABEL if timezones is None else combine_labels([TIME_LABEL, timezones.label])
        )

    def do_draw(self, tc: TestCase) -> dt.time:
        value = tc._generate_time(self.min_value, self.max_value, self.precision)
        if self.timezones is not None:
            value = value.replace(tzinfo=tc.draw_silent(self.timezones))
        return value

    def __repr__(self) -> str:
        return f"times(min_value={self.min_value!r}, max_value={self.max_value!r})"


def _time_microseconds(value: dt.time) -> int:
    return ((value.hour * 60 + value.minute) * 60 + value.second) * 1_000_000 + value.microsecond


def times(
    min_value: dt.time = dt.time.min,
    max_value: dt.time = dt.time.max,
    *,
    timezones: Generator[Any] | None = None,
    precision: str = "microseconds",
) -> Generator[dt.time]:
    """Generate :class:`datetime.time` objects in ``[min_value, max_value]``,
    inclusive, shrinking toward midnight.

    The bounds must be naive. The times are naive too, unless
    ``timezones`` is given: a generator of :class:`datetime.tzinfo` objects
    (such as :func:`timezones`), one of which is attached to each time.
    ``fold`` is never set.

    ``precision`` names the unit every drawn time is a whole number of,
    as the ``timespec`` argument of :meth:`~datetime.time.isoformat` does:
    ``"microseconds"`` (the default), ``"milliseconds"``, ``"seconds"``,
    ``"minutes"`` or ``"hours"``. Bounds holding no time at the precision
    are an :class:`~hegel.InvalidArgument`.
    """
    _check_type("min_value", min_value, dt.time)
    _check_type("max_value", max_value, dt.time)
    if min_value.tzinfo is not None or max_value.tzinfo is not None:
        raise InvalidArgument("times() bounds must be naive")
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    constant, unit = _precision(precision)
    _check_fits(
        precision, _time_microseconds(min_value), _time_microseconds(max_value), unit, "time"
    )
    return Times(
        min_value, max_value, None if timezones is None else as_generator(timezones), constant
    )


def _select_keys(available: Iterable[str], links: bool, legacy: bool) -> list[str]:
    """The IANA names in ``available`` the engine's database also knows,
    sorted, without link names unless ``links`` and without legacy names
    unless ``legacy``."""
    keys = []
    for key in sorted(available):
        info = _native.timezone_info(key)
        if info is None:
            continue
        _, is_link, _, tags = info
        if (is_link and not links) or (tags & _native.TIMEZONE_TAG_LEGACY and not legacy):
            continue
        keys.append(key)
    if not keys:
        raise InvalidArgument(
            "no IANA time zone this Python can load is in Hegel's time zone database. "
            "On a system without a time zone database (Windows, say) install the "
            "tzdata package."
        )
    return keys


@cache
def _host_spec(links: bool, legacy: bool) -> _native.TimeZoneSpec:
    keys = _select_keys(zoneinfo.available_timezones(), links, legacy)
    return _native.TimeZoneSpec(keys, links, legacy, False, False)


@cache
def _single_zone_spec(key: str) -> _native.TimeZoneSpec | None:
    if _native.timezone_info(key) is None:
        return None
    return _native.TimeZoneSpec([key], True, True, False, False)


def _engine_spec(tz: dt.tzinfo) -> _native.TimeZoneSpec | None:
    """The spec drawing only ``tz``, when the engine's database has it:
    a :class:`zoneinfo.ZoneInfo` built from a key the engine knows."""
    key = getattr(tz, "key", None)
    if isinstance(tz, zoneinfo.ZoneInfo) and isinstance(key, str):
        return _single_zone_spec(key)
    return None


def _check_names(names: Any, *, loadable: bool) -> tuple[str, ...]:
    if isinstance(names, (str, bytes)) or not isinstance(names, Iterable):
        raise InvalidArgument(f"names={names!r} must be an iterable of IANA time zone names")
    checked = tuple(names)
    if not checked:
        raise InvalidArgument("names must not be empty")
    for name in checked:
        if not isinstance(name, str):
            raise InvalidArgument(f"names={names!r} must contain strings, not {name!r}")
        if _native.timezone_info(name) is None:
            raise InvalidArgument(f"{name!r} is not a name in Hegel's time zone database")
        if loadable:
            try:
                zoneinfo.ZoneInfo(name)
            except (zoneinfo.ZoneInfoNotFoundError, ValueError) as error:
                raise InvalidArgument(
                    f"{name!r} is not a time zone this Python can load: {error}"
                ) from None
    return checked


class TimeZoneKeys(Generator[str]):
    def __init__(self, names: tuple[str, ...] | None, links: bool, legacy: bool) -> None:
        self.names = names
        self.links = links
        self.legacy = legacy
        self.spec = (
            _host_spec(links, legacy)
            if names is None
            else _native.TimeZoneSpec(list(names), links, legacy, False, False)
        )
        self.label = TIMEZONE_KEY_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_timezone(self.spec)

    def arguments(self) -> str:
        names = "" if self.names is None else f"names={list(self.names)!r}, "
        return f"{names}links={self.links!r}, legacy={self.legacy!r}"

    def __repr__(self) -> str:
        return f"timezone_keys({self.arguments()})"


def timezone_keys(
    *, names: Iterable[str] | None = None, links: bool = True, legacy: bool = False
) -> Generator[str]:
    """Generate IANA time zone names, such as ``"America/New_York"``,
    shrinking toward ``"UTC"``.

    The names are those of Hegel's bundled time zone database that
    :func:`zoneinfo.available_timezones` also lists, so every name
    generated can be loaded with :class:`zoneinfo.ZoneInfo`; on a system
    without a time zone database (Windows, say) install the ``tzdata``
    package. The ``posix/`` and ``right/`` variants some systems carry are
    never generated. By default link names such as ``"Asia/Calcutta"`` are
    included and legacy names such as ``"EST5EDT"`` and ``"US/Eastern"``
    are excluded; ``links`` and ``legacy`` switch each.

    ``names`` restricts the draw to those names, each of which must be in
    Hegel's database; ``links`` and ``legacy`` do not apply to an explicit
    list.

    Half the draws are uniform over the names and half weighted toward
    zones with a rare feature (a fractional offset, negative daylight
    saving, a skipped day, ...), so each is reached by a short run. A name
    shrinks to a zone a reader recognises: real zones with a whole-hour
    offset and a simple daylight-saving rule first, then the other real
    zones, then the ``Etc/GMT`` family, then the legacy names.
    """
    _check_type("links", links, bool)
    _check_type("legacy", legacy, bool)
    checked = None if names is None else _check_names(names, loadable=False)
    return TimeZoneKeys(checked, links, legacy)


class TimeZones(Generator[zoneinfo.ZoneInfo]):
    def __init__(self, keys: TimeZoneKeys, no_cache: bool) -> None:
        self.keys = keys
        self.no_cache = no_cache
        self.label = TIMEZONE_LABEL

    def resolve(self, key: str) -> zoneinfo.ZoneInfo:
        return zoneinfo.ZoneInfo.no_cache(key) if self.no_cache else zoneinfo.ZoneInfo(key)

    def do_draw(self, tc: TestCase) -> zoneinfo.ZoneInfo:
        return self.resolve(tc._generate_timezone(self.keys.spec))

    def __repr__(self) -> str:
        return f"timezones({self.keys.arguments()}, no_cache={self.no_cache!r})"


def timezones(
    *,
    names: Iterable[str] | None = None,
    no_cache: bool = False,
    links: bool = True,
    legacy: bool = False,
) -> Generator[zoneinfo.ZoneInfo]:
    """Generate IANA time zones, as :class:`zoneinfo.ZoneInfo` objects,
    shrinking toward UTC.

    The zones are those :func:`timezone_keys` names, with the same
    ``names``, ``links`` and ``legacy`` arguments; a name in ``names``
    must also be one this Python can load. With ``no_cache=True`` each
    zone is built with :meth:`zoneinfo.ZoneInfo.no_cache`, so it is not
    the same object as ``ZoneInfo(key)``.
    """
    _check_type("no_cache", no_cache, bool)
    _check_type("links", links, bool)
    _check_type("legacy", legacy, bool)
    checked = None if names is None else _check_names(names, loadable=True)
    return TimeZones(TimeZoneKeys(checked, links, legacy), no_cache)


def _instant(value: dt.datetime) -> dt.timedelta:
    """An aware datetime as the exact time since the Unix epoch."""
    return value - _EPOCH


def _timestamp(value: dt.datetime) -> tuple[int, int]:
    """An aware datetime as the engine's ``(seconds, nanoseconds)``."""
    microseconds = _instant(value) // _MICROSECOND
    seconds, remainder = divmod(microseconds, 1_000_000)
    return seconds, remainder * 1000


def _local_wall(value: dt.datetime, tz: dt.tzinfo) -> dt.datetime:
    """The naive wall time of an aware datetime in ``tz``, clamped into the
    range :class:`datetime.datetime` represents."""
    try:
        return value.astimezone(tz).replace(tzinfo=None)
    except OverflowError:
        return dt.datetime.max if value.year > 5000 else dt.datetime.min


def _is_imaginary(value: dt.datetime) -> bool:
    try:
        return value.astimezone(dt.UTC).astimezone(value.tzinfo).replace(tzinfo=None) != (
            value.replace(tzinfo=None)
        )
    except (OverflowError, ValueError):  # nocov
        return False


def _components(value: dt.datetime) -> tuple[int, int, int, int, int, int, int]:
    return (
        value.year,
        value.month,
        value.day,
        value.hour,
        value.minute,
        value.second,
        value.microsecond * 1000,
    )


class Datetimes(Generator[dt.datetime]):
    def __init__(
        self,
        min_value: dt.datetime,
        max_value: dt.datetime,
        timezones: Generator[Any] | None,
        allow_imaginary: bool,
        ambiguous: int,
        precision: int,
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.timezones = timezones
        self.allow_imaginary = allow_imaginary
        self.ambiguous = ambiguous
        self.precision = precision
        self.aware = min_value.tzinfo is not None
        if self.aware:
            self.zoned_spec = _native.ZonedSpec(
                None,
                (_timestamp(min_value), _timestamp(max_value)),
                precision,
                allow_imaginary,
                ambiguous,
            )
        else:
            self.zoned_spec = _native.ZonedSpec(
                (_components(min_value), _components(max_value)),
                None,
                precision,
                allow_imaginary,
                ambiguous,
            )
        self.label = (
            DATETIME_LABEL
            if timezones is None
            else combine_labels([DATETIME_LABEL, timezones.label])
        )

    def do_draw(self, tc: TestCase) -> dt.datetime:
        if isinstance(self.timezones, TimeZones):
            key, value = tc._generate_zoned_datetime(self.timezones.keys.spec, self.zoned_spec)
            return value.replace(tzinfo=self.timezones.resolve(key))
        tz = None if self.timezones is None else tc.draw_silent(self.timezones)
        if tz is None:
            if self.aware:
                raise InvalidArgument(
                    f"timezones={self.timezones!r} generated None, but with aware bounds "
                    "every datetime needs a tzinfo"
                )
            return tc._generate_datetime(self.min_value, self.max_value, self.precision)
        if not isinstance(tz, dt.tzinfo):
            raise InvalidArgument(f"timezones must generate tzinfo objects, got {tz!r}")
        spec = _engine_spec(tz)
        if spec is not None:
            return tc._generate_zoned_datetime(spec, self.zoned_spec)[1].replace(tzinfo=tz)
        if self.aware:
            lo, hi = _local_wall(self.min_value, tz), _local_wall(self.max_value, tz)
            if lo > hi:
                tc.reject()
        else:
            lo, hi = self.min_value, self.max_value
        value = tc._generate_datetime(lo, hi, self.precision).replace(tzinfo=tz)
        if not self.allow_imaginary and _is_imaginary(value):
            tc.reject()
        if self.aware:
            instant = _instant(value)
            if (lo != dt.datetime.min and instant < _instant(self.min_value)) or (
                hi != dt.datetime.max and instant > _instant(self.max_value)
            ):
                tc.reject()
        return value

    def __repr__(self) -> str:
        return f"datetimes(min_value={self.min_value!r}, max_value={self.max_value!r})"


def datetimes(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    timezones: Generator[Any] | None = None,
    allow_imaginary: bool = True,
    ambiguous: str = "both",
    precision: str = "microseconds",
) -> Generator[dt.datetime]:
    """Generate :class:`datetime.datetime` objects, naive or aware, in
    ``[min_value, max_value]``, shrinking toward 2000-01-01 00:00 (or the
    bound nearest it).

    **Naive bounds.** With naive bounds, or none (the defaults are
    :attr:`datetime.min <datetime.datetime.min>` and
    :attr:`datetime.max <datetime.datetime.max>`), the bounds limit the
    wall-clock time. The datetimes are naive unless ``timezones`` is
    given: a generator of :class:`datetime.tzinfo` objects (or ``None``),
    such as :func:`timezones`, one of which is attached to each drawn
    wall-clock time::

        aware = tc.draw(gs.datetimes(timezones=gs.timezones()))

    **Aware bounds.** With aware bounds the bounds are moments in time,
    compared as such however their time zones differ, and every datetime
    is aware and lies between them in a zone drawn from ``timezones``,
    which defaults to :func:`timezones` and must not generate ``None``::

        this_year = gs.datetimes(
            dt.datetime(2026, 1, 1, tzinfo=dt.UTC), dt.datetime(2027, 1, 1, tzinfo=dt.UTC)
        )

    One bound may be omitted; passing one naive and one aware bound is an
    error.

    **Transitions.** A zone from :func:`timezones`, or a
    :class:`zoneinfo.ZoneInfo` from any other generator, is drawn in by
    the engine, which knows its daylight-saving and offset transitions:
    one draw in twenty on average lands within a second, an hour or a day
    of one, or of a well-known instant such as the Unix epoch or the end
    of the signed 32-bit epoch, so the skipped and repeated wall times
    where date bugs live are generated deliberately. A repeated wall time
    is drawn with either value of ``fold`` by default; ``ambiguous``
    chooses ``"earlier"`` (``fold=0``) or ``"later"`` (``fold=1``)
    instead of ``"both"``. A skipped wall time (one the clocks jumped
    over, which :mod:`datetime` represents but which never occurred) is
    drawn by default, because malformed timestamps are a common source of
    bugs; with ``allow_imaginary=False`` the engine draws a real wall time
    instead. Fixed offsets (:class:`datetime.timezone`) and other
    ``tzinfo`` implementations have no transitions the engine can see:
    the drawn wall-clock time is attached to them as is, with ``fold`` 0
    whatever ``ambiguous`` says, and an imaginary one rejects the test
    case under ``allow_imaginary=False``; with aware bounds a wall time
    whose instant falls outside them rejects the test case too.

    **Precision.** ``precision`` names the unit every drawn time of day
    is a whole number of, as the ``timespec`` argument of
    :meth:`~datetime.datetime.isoformat` does: ``"microseconds"`` (the
    default), ``"milliseconds"``, ``"seconds"``, ``"minutes"`` or
    ``"hours"``.

    Bounds that are not datetimes, a mix of a naive and an aware bound,
    ``min_value > max_value``, bounds holding no wall time at the
    precision, or a ``timezones`` generator producing something other
    than ``tzinfo`` objects (or ``None`` with aware bounds) are an
    :class:`~hegel.InvalidArgument`.
    """
    _check_type("allow_imaginary", allow_imaginary, bool)
    _check_choice("ambiguous", ambiguous, _AMBIGUOUS)
    precision_constant, unit = _precision(precision)
    bounds = (("min_value", min_value), ("max_value", max_value))
    for name, value in bounds:
        if value is not None:
            _check_type(name, value, dt.datetime)
    aware = any(value is not None and value.tzinfo is not None for _, value in bounds)
    zones: Generator[Any] | None
    if aware:
        for name, value in bounds:
            if value is not None and value.tzinfo is None:
                raise InvalidArgument(
                    f"{name}={value!r} is naive, but the other bound is timezone-aware; "
                    "the bounds must be both naive or both aware"
                )
        if min_value is None:
            min_value = dt.datetime.min.replace(tzinfo=dt.UTC)
        if max_value is None:
            max_value = dt.datetime.max.replace(tzinfo=dt.UTC)
        if _instant(max_value) < _instant(min_value):
            raise InvalidArgument(
                f"Cannot have max_value={max_value!r} < min_value={min_value!r}, "
                "comparing as moments in time"
            )
        zones = (
            TimeZones(TimeZoneKeys(None, True, False), False)
            if timezones is None
            else as_generator(timezones)
        )
    else:
        if min_value is None:
            min_value = dt.datetime.min
        if max_value is None:
            max_value = dt.datetime.max
        if min_value > max_value:
            raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
        _check_fits(
            precision,
            (min_value - dt.datetime.min) // _MICROSECOND,
            (max_value - dt.datetime.min) // _MICROSECOND,
            unit,
            "datetime",
        )
        zones = None if timezones is None else as_generator(timezones)
    return Datetimes(
        min_value, max_value, zones, allow_imaginary, _AMBIGUOUS[ambiguous], precision_constant
    )


class Timedeltas(Generator[dt.timedelta]):
    def __init__(self, min_value: dt.timedelta, max_value: dt.timedelta, precision: int) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.precision = precision
        self.label = TIMEDELTA_LABEL

    def do_draw(self, tc: TestCase) -> dt.timedelta:
        return tc._generate_timedelta(self.min_value, self.max_value, self.precision)

    def __repr__(self) -> str:
        return f"timedeltas(min_value={self.min_value!r}, max_value={self.max_value!r})"


def timedeltas(
    min_value: dt.timedelta = dt.timedelta.min,
    max_value: dt.timedelta = dt.timedelta.max,
    *,
    precision: str = "microseconds",
) -> Generator[dt.timedelta]:
    """Generate :class:`datetime.timedelta` objects in
    ``[min_value, max_value]``, inclusive, shrinking toward zero. The
    bounds default to the whole range :mod:`datetime` supports. One draw
    in twenty on average targets a round duration, such as a second, an
    hour, a day, 25 hours or a week, of either sign.

    ``precision`` names the unit every drawn delta is a whole number of:
    ``"microseconds"`` (the default), ``"milliseconds"``, ``"seconds"``,
    ``"minutes"`` or ``"hours"``. Bounds holding no delta at the
    precision are an :class:`~hegel.InvalidArgument`.
    """
    _check_type("min_value", min_value, dt.timedelta)
    _check_type("max_value", max_value, dt.timedelta)
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    constant, unit = _precision(precision)
    _check_fits(precision, min_value // _MICROSECOND, max_value // _MICROSECOND, unit, "timedelta")
    return Timedeltas(min_value, max_value, constant)


class UtcOffsets(Generator[dt.timezone]):
    def __init__(self, min_value: dt.timedelta, max_value: dt.timedelta, granularity: str) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.granularity = granularity
        self.label = UTC_OFFSET_LABEL

    def do_draw(self, tc: TestCase) -> dt.timezone:
        seconds = tc._generate_offset(
            self.min_value // _SECOND,
            self.max_value // _SECOND,
            _GRANULARITIES[self.granularity][0],
        )
        return dt.timezone(dt.timedelta(seconds=seconds))

    def __repr__(self) -> str:
        return (
            f"utc_offsets(min_value={self.min_value!r}, max_value={self.max_value!r}, "
            f"granularity={self.granularity!r})"
        )


def utc_offsets(
    min_value: dt.timedelta = -_MAX_OFFSET,
    max_value: dt.timedelta = _MAX_OFFSET,
    *,
    granularity: str = "seconds",
) -> Generator[dt.timezone]:
    """Generate fixed UTC offsets, as :class:`datetime.timezone` objects,
    with an offset in ``[min_value, max_value]``, shrinking toward
    :attr:`datetime.timezone.utc`.

    The bounds are whole seconds within the day :class:`datetime.timezone`
    allows, from -23:59:59 to +23:59:59 (the defaults). ``granularity``
    names the unit every drawn offset is a whole number of:
    ``"hours"``, ``"quarter_hours"``, ``"minutes"`` or ``"seconds"`` (the
    default). One draw in twenty on average targets an offset some IANA
    zone has used, or a limit such as ±18:00 or ±00:00:01, when it lies in
    the bounds at the granularity.

    Bounds that are not whole-second ``timedelta`` objects within the
    range, ``min_value > max_value``, or bounds holding no offset at the
    granularity are an :class:`~hegel.InvalidArgument`.
    """
    _check_type("min_value", min_value, dt.timedelta)
    _check_type("max_value", max_value, dt.timedelta)
    for name, value in (("min_value", min_value), ("max_value", max_value)):
        if value.microseconds:
            raise InvalidArgument(f"{name}={value!r} must be a whole number of seconds")
        if not -_MAX_OFFSET <= value <= _MAX_OFFSET:
            raise InvalidArgument(
                f"{name}={value!r} must be between -23:59:59 and +23:59:59, the offsets "
                "datetime.timezone allows"
            )
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    _check_choice("granularity", granularity, _GRANULARITIES)
    _check_fits(
        granularity,
        min_value // _SECOND,
        max_value // _SECOND,
        _GRANULARITIES[granularity][1],
        "offset",
    )
    return UtcOffsets(min_value, max_value, granularity)
