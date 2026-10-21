"""Date and time generators."""

from __future__ import annotations

import datetime as dt
import zoneinfo
from functools import cache
from typing import TYPE_CHECKING, Any

from hegel import _native
from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, _PrintsValue
from hegel.generators.combinators import as_generator

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["dates", "datetimes", "timedeltas", "times", "timezone_keys", "timezones"]

DATE_LABEL = label_from_name("hegel.dates")
TIME_LABEL = label_from_name("hegel.times")
DATETIME_LABEL = label_from_name("hegel.datetimes")
TIMEDELTA_LABEL = label_from_name("hegel.timedeltas")
TIMEZONE_KEY_LABEL = label_from_name("hegel.timezone_keys")
TIMEZONE_LABEL = label_from_name("hegel.timezones")

_EPOCH = dt.datetime(1970, 1, 1, tzinfo=dt.UTC)
_MICROSECOND = dt.timedelta(microseconds=1)
_ZERO = dt.timedelta(0)


def _check_type(name: str, value: Any, cls: type) -> None:
    if not isinstance(value, cls):
        raise InvalidArgument(f"{name}={value!r} must be a {cls.__name__}")


class Dates(_PrintsValue[dt.date]):
    def __init__(self, min_value: dt.date, max_value: dt.date) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.label = DATE_LABEL

    def do_draw(self, tc: TestCase) -> dt.date:
        return tc._generate_date(self.min_value, self.max_value)

    def __repr__(self) -> str:
        return f"dates(min_value={self.min_value!r}, max_value={self.max_value!r})"


def dates(min_value: dt.date = dt.date.min, max_value: dt.date = dt.date.max) -> Generator[dt.date]:
    """Generate :class:`datetime.date` objects in ``[min_value, max_value]``,
    inclusive, shrinking toward 2000-01-01 (or the bound nearest it).

    The bounds default to the whole range :mod:`datetime` supports. One
    draw in twenty on average targets a date tests tend to get wrong: the
    leap days nearest the bounds, the year boundaries, the 2038 and 2106
    overflow days and the days of an ISO week 53.

    Bounds that are not dates (a ``datetime`` is not accepted), or
    ``min_value > max_value``, are an :class:`~hegel.InvalidArgument`.
    """
    _check_type("min_value", min_value, dt.date)
    _check_type("max_value", max_value, dt.date)
    if isinstance(min_value, dt.datetime) or isinstance(max_value, dt.datetime):
        raise InvalidArgument("dates() bounds must be date objects, not datetimes")
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    return Dates(min_value, max_value)


class Times(_PrintsValue[dt.time]):
    def __init__(
        self, min_value: dt.time, max_value: dt.time, timezones: Generator[Any] | None
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.timezones = timezones
        self.label = (
            TIME_LABEL if timezones is None else combine_labels([TIME_LABEL, timezones.label])
        )

    def do_draw(self, tc: TestCase) -> dt.time:
        value = tc._generate_time(self.min_value, self.max_value)
        if self.timezones is not None:
            value = value.replace(tzinfo=tc.draw_silent(self.timezones))
        return value

    def __repr__(self) -> str:
        return f"times(min_value={self.min_value!r}, max_value={self.max_value!r})"


def times(
    min_value: dt.time = dt.time.min,
    max_value: dt.time = dt.time.max,
    *,
    timezones: Generator[Any] | None = None,
) -> Generator[dt.time]:
    """Generate :class:`datetime.time` objects in ``[min_value, max_value]``,
    inclusive, shrinking toward midnight.

    The bounds must be naive. The times are naive too, unless
    ``timezones`` is given: a generator of :class:`datetime.tzinfo` objects
    (such as :func:`timezones`), one of which is attached to each time.
    ``fold`` is never set.
    """
    _check_type("min_value", min_value, dt.time)
    _check_type("max_value", max_value, dt.time)
    if min_value.tzinfo is not None or max_value.tzinfo is not None:
        raise InvalidArgument("times() bounds must be naive")
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    return Times(min_value, max_value, None if timezones is None else as_generator(timezones))


def _select_keys(available: set[str], links: bool, legacy: bool) -> list[str]:
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


class TimeZoneKeys(_PrintsValue[str]):
    def __init__(self, links: bool, legacy: bool) -> None:
        self.links = links
        self.legacy = legacy
        self.spec = _host_spec(links, legacy)
        self.label = TIMEZONE_KEY_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_timezone(self.spec)

    def __repr__(self) -> str:
        return f"timezone_keys(links={self.links!r}, legacy={self.legacy!r})"


def timezone_keys(*, links: bool = True, legacy: bool = False) -> Generator[str]:
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

    Half the draws are uniform over the names and half weighted toward
    zones with a rare feature (a fractional offset, negative daylight
    saving, a skipped day, ...), so each is reached by a short run. A name
    shrinks to a zone a reader recognises: real zones with a whole-hour
    offset and a simple daylight-saving rule first, then the other real
    zones, then the ``Etc/GMT`` family, then the legacy names.
    """
    _check_type("links", links, bool)
    _check_type("legacy", legacy, bool)
    return TimeZoneKeys(links, legacy)


class TimeZones(_PrintsValue[zoneinfo.ZoneInfo]):
    def __init__(self, keys: TimeZoneKeys, no_cache: bool) -> None:
        self.keys = keys
        self.no_cache = no_cache
        self.label = TIMEZONE_LABEL

    def resolve(self, key: str) -> zoneinfo.ZoneInfo:
        return zoneinfo.ZoneInfo.no_cache(key) if self.no_cache else zoneinfo.ZoneInfo(key)

    def do_draw(self, tc: TestCase) -> zoneinfo.ZoneInfo:
        return self.resolve(tc._generate_timezone(self.keys.spec))

    def __repr__(self) -> str:
        return (
            f"timezones(no_cache={self.no_cache!r}, links={self.keys.links!r}, "
            f"legacy={self.keys.legacy!r})"
        )


def timezones(
    *, no_cache: bool = False, links: bool = True, legacy: bool = False
) -> Generator[zoneinfo.ZoneInfo]:
    """Generate IANA time zones, as :class:`zoneinfo.ZoneInfo` objects,
    shrinking toward UTC.

    The zones are those :func:`timezone_keys` names, with the same
    ``links`` and ``legacy`` switches. With ``no_cache=True`` each is built
    with :meth:`zoneinfo.ZoneInfo.no_cache`, so it is not the same object
    as ``ZoneInfo(key)``.
    """
    _check_type("no_cache", no_cache, bool)
    _check_type("links", links, bool)
    _check_type("legacy", legacy, bool)
    return TimeZones(TimeZoneKeys(links, legacy), no_cache)


def _instant(value: dt.datetime) -> dt.timedelta:
    """An aware datetime as the exact time since the Unix epoch."""
    return value - _EPOCH


def _timestamp(value: dt.datetime) -> tuple[int, int]:
    """An aware datetime as the engine's ``(seconds, nanoseconds)``."""
    microseconds = _instant(value) // _MICROSECOND
    seconds, remainder = divmod(microseconds, 1_000_000)
    return seconds, remainder * 1000


def _utc_wall(value: dt.datetime) -> dt.datetime:
    """The naive UTC wall time of an aware datetime, clamped into the range
    :class:`datetime.datetime` represents."""
    try:
        return value.astimezone(dt.UTC).replace(tzinfo=None)
    except OverflowError:
        offset = value.utcoffset()
        return dt.datetime.min if offset is not None and offset > _ZERO else dt.datetime.max


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


class Datetimes(_PrintsValue[dt.datetime]):
    def __init__(
        self,
        min_value: dt.datetime,
        max_value: dt.datetime,
        timezones: Generator[Any] | None,
        allow_imaginary: bool,
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.timezones = timezones
        self.allow_imaginary = allow_imaginary
        self.aware = min_value.tzinfo is not None
        if self.aware:
            bounds = _native.ZonedSpec(
                None,
                (_timestamp(min_value), _timestamp(max_value)),
                allow_imaginary,
                _native.AMBIGUOUS_BOTH,
            )
        else:
            bounds = _native.ZonedSpec(
                (_components(min_value), _components(max_value)),
                None,
                allow_imaginary,
                _native.AMBIGUOUS_BOTH,
            )
        self.zoned_spec = bounds
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
            return tc._generate_datetime(self.min_value, self.max_value)
        if not isinstance(tz, dt.tzinfo):
            raise InvalidArgument(f"timezones must generate tzinfo objects, got {tz!r}")
        spec = _engine_spec(tz)
        if spec is not None:
            return tc._generate_zoned_datetime(spec, self.zoned_spec)[1].replace(tzinfo=tz)
        if self.aware:
            utc = tc._generate_datetime(_utc_wall(self.min_value), _utc_wall(self.max_value))
            try:
                return utc.replace(tzinfo=dt.UTC).astimezone(tz)
            except OverflowError:
                tc.reject()
        value = tc._generate_datetime(self.min_value, self.max_value).replace(tzinfo=tz)
        if not self.allow_imaginary and _is_imaginary(value):
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
    is drawn with either value of ``fold``. A skipped one (a wall time the
    clocks jumped over, which :mod:`datetime` represents but which never
    occurred) is drawn by default, because malformed timestamps are a
    common source of bugs; with ``allow_imaginary=False`` the engine draws
    a real wall time instead. Fixed offsets (:class:`datetime.timezone`)
    and other ``tzinfo`` implementations have no transitions the engine
    can see: with naive bounds the drawn wall-clock time is attached to
    them as is, with ``fold`` 0, and an imaginary one rejects the test
    case under ``allow_imaginary=False``; with aware bounds the instant is
    drawn and converted.

    Bounds that are not datetimes, a mix of a naive and an aware bound,
    ``min_value > max_value``, or a ``timezones`` generator producing
    something other than ``tzinfo`` objects (or ``None`` with aware
    bounds) are an :class:`~hegel.InvalidArgument`.
    """
    _check_type("allow_imaginary", allow_imaginary, bool)
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
            TimeZones(TimeZoneKeys(True, False), False)
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
        zones = None if timezones is None else as_generator(timezones)
    return Datetimes(min_value, max_value, zones, allow_imaginary)


class Timedeltas(_PrintsValue[dt.timedelta]):
    def __init__(self, min_value: dt.timedelta, max_value: dt.timedelta) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.label = TIMEDELTA_LABEL

    def do_draw(self, tc: TestCase) -> dt.timedelta:
        return tc._generate_timedelta(self.min_value, self.max_value)

    def __repr__(self) -> str:
        return f"timedeltas(min_value={self.min_value!r}, max_value={self.max_value!r})"


def timedeltas(
    min_value: dt.timedelta = dt.timedelta.min, max_value: dt.timedelta = dt.timedelta.max
) -> Generator[dt.timedelta]:
    """Generate :class:`datetime.timedelta` objects in
    ``[min_value, max_value]``, inclusive, shrinking toward zero. The
    bounds default to the whole range :mod:`datetime` supports. One draw
    in twenty on average targets a round duration, such as a second, an
    hour, a day, 25 hours or a week, of either sign."""
    _check_type("min_value", min_value, dt.timedelta)
    _check_type("max_value", max_value, dt.timedelta)
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    return Timedeltas(min_value, max_value)
