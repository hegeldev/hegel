"""Date and time generators.

Use them through the ``temporal`` namespace of :mod:`hegel.generators`:
``tc.draw(gs.temporal.datetimes())``.
"""

from __future__ import annotations

import datetime as dt
import os
import threading
import zoneinfo
from collections.abc import Callable, Iterable, Sequence
from dataclasses import KW_ONLY, astuple, dataclass
from importlib import resources
from typing import TYPE_CHECKING, Any

from hegel import _native
from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, _PrintsValue
from hegel.generators.combinators import as_generator
from hegel.test_case import _duration_of

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "RuleDate",
    "Span",
    "ZoneData",
    "ZoneNameInfo",
    "ZoneOracle",
    "ZoneResolution",
    "ZoneRule",
    "ZoneSource",
    "ZoneType",
    "dates",
    "datetimes",
    "default_zone_source",
    "durations",
    "instants",
    "offsets",
    "set_default_zone_source",
    "spans",
    "times",
    "tzdb_version",
    "zone_names",
    "zoned_datetimes",
    "zones",
]

DATE_LABEL = label_from_name("hegel.dates")
TIME_LABEL = label_from_name("hegel.times")
DATETIME_LABEL = label_from_name("hegel.datetimes")
ZONED_DATETIME_LABEL = label_from_name("hegel.zoned_datetimes")
INSTANT_LABEL = label_from_name("hegel.instants")
SPAN_LABEL = label_from_name("hegel.spans")
DURATION_LABEL = label_from_name("hegel.durations")
ZONE_NAME_LABEL = label_from_name("hegel.zone_names")
ZONE_LABEL = label_from_name("hegel.zones")
OFFSET_LABEL = label_from_name("hegel.offsets")

_EPOCH = dt.datetime(1970, 1, 1, tzinfo=dt.UTC)
_MICROSECOND = dt.timedelta(microseconds=1)
_SECOND = dt.timedelta(seconds=1)
_MAX_OFFSET = dt.timedelta(hours=23, minutes=59, seconds=59)
_MIN_INSTANT = dt.datetime.min.replace(tzinfo=dt.UTC)
_MAX_INSTANT = dt.datetime.max.replace(tzinfo=dt.UTC)
_I64_MIN = -(2**63)
_I64_MAX = 2**63 - 1

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


def _arguments(**values: Any) -> str:
    """The ``name=value, ...`` text of the arguments a generator's repr
    shows, skipping those that are ``None``."""
    return ", ".join(f"{name}={value!r}" for name, value in values.items() if value is not None)


def _unless(value: Any, default: Any) -> Any:
    """``value`` for a repr to show, or ``None`` when it is the default."""
    return None if value == default else value


def _check_order(min_value: Any, max_value: Any) -> None:
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")


def _members(mask: int | None, lowest: int, count: int) -> list[int] | None:
    """The values a :func:`_mask` bit set holds, or ``None`` for no set."""
    if mask is None:
        return None
    return [lowest + bit for bit in range(count) if mask >> bit & 1]


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
    return mask


class Dates(_PrintsValue[dt.date]):
    def __init__(
        self,
        min_value: dt.date | None,
        max_value: dt.date | None,
        weekdays: int | None,
        months: int | None,
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.bounds = (
            dt.date.min if min_value is None else min_value,
            dt.date.max if max_value is None else max_value,
        )
        self.weekdays = weekdays
        self.months = months
        self.options = (
            None if weekdays is None and months is None else _native.DateOptions(weekdays, months)
        )
        self.label = DATE_LABEL

    def do_draw(self, tc: TestCase) -> dt.date:
        return tc._generate_date(*self.bounds, self.options)

    def __repr__(self) -> str:
        arguments = _arguments(
            min_value=self.min_value,
            max_value=self.max_value,
            weekdays=_members(self.weekdays, 0, 7),
            months=_members(self.months, 1, 12),
        )
        return f"temporal.dates({arguments})"


def _check_date(name: str, value: Any) -> None:
    if isinstance(value, dt.datetime) or not isinstance(value, dt.date):
        raise InvalidArgument(f"{name}={value!r} must be a date (a datetime is not accepted)")


def dates(
    min_value: dt.date | None = None,
    max_value: dt.date | None = None,
    *,
    weekdays: Iterable[int] | None = None,
    months: Iterable[int] | None = None,
) -> Generator[dt.date]:
    """Generate :class:`datetime.date` objects in ``[min_value, max_value]``,
    inclusive, shrinking toward 2000-01-01 (or the bound nearest it).

    Either bound may be omitted: a missing bound is the end of the range
    :mod:`datetime` supports. One draw in twenty on average targets a date
    tests tend to get wrong: the leap days nearest the bounds, the year
    boundaries, the 2038 and 2106 overflow days and the days of an ISO
    week 53.

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
    if min_value is not None:
        _check_date("min_value", min_value)
    if max_value is not None:
        _check_date("max_value", max_value)
    weekday_mask = None if weekdays is None else _mask("weekdays", weekdays, 0, 6)
    month_mask = None if months is None else _mask("months", months, 1, 12)
    result = Dates(min_value, max_value, weekday_mask, month_mask)
    low, high = result.bounds
    _check_order(low, high)
    _native.validate_date(
        (low.year, low.month, low.day), (high.year, high.month, high.day), result.options
    )
    return result


class Times(_PrintsValue[dt.time]):
    def __init__(
        self,
        min_value: dt.time | None,
        max_value: dt.time | None,
        zones: Generator[Any] | None,
        precision: str,
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.bounds = (
            dt.time.min if min_value is None else min_value,
            dt.time.max if max_value is None else max_value,
        )
        self.zones = zones
        self.precision = precision
        self.label = TIME_LABEL if zones is None else combine_labels([TIME_LABEL, zones.label])

    def do_draw(self, tc: TestCase) -> dt.time:
        value = tc._generate_time(*self.bounds, _PRECISIONS[self.precision][0])
        if self.zones is None:
            return value
        tz = tc.draw_silent(self.zones)
        if tz is not None and not isinstance(tz, dt.tzinfo):
            raise InvalidArgument(f"zones must generate tzinfo objects, got {tz!r}")
        return value.replace(tzinfo=tz)

    def __repr__(self) -> str:
        arguments = _arguments(
            min_value=self.min_value,
            max_value=self.max_value,
            zones=self.zones,
            precision=_unless(self.precision, "microseconds"),
        )
        return f"temporal.times({arguments})"


def _time_fields(value: dt.time) -> tuple[int, int, int, int]:
    return value.hour, value.minute, value.second, value.microsecond * 1000


def times(
    min_value: dt.time | None = None,
    max_value: dt.time | None = None,
    *,
    zones: Generator[Any] | None = None,
    precision: str = "microseconds",
) -> Generator[dt.time]:
    """Generate :class:`datetime.time` objects in ``[min_value, max_value]``,
    inclusive, shrinking toward midnight.

    Either bound may be omitted, and a bound given must be naive. The
    times are naive too, unless ``zones`` is given: a generator of
    :class:`datetime.tzinfo` objects (or ``None``), one of which is
    attached to each time. ``fold`` is never set. A time has no date, so
    a zone whose offset depends on the date, as a
    :class:`zoneinfo.ZoneInfo` from :func:`zones` does, gives it no
    :meth:`~datetime.time.utcoffset`; :func:`offsets` generates zones
    that do.

    ``precision`` names the unit every drawn time is a whole number of,
    as the ``timespec`` argument of :meth:`~datetime.time.isoformat` does:
    ``"microseconds"`` (the default), ``"milliseconds"``, ``"seconds"``,
    ``"minutes"`` or ``"hours"``.

    Bounds that are not naive times, ``min_value > max_value``, bounds
    holding no time at the precision, or a ``zones`` generator producing
    something other than ``tzinfo`` objects or ``None`` are an
    :class:`~hegel.InvalidArgument`.
    """
    for name, value in (("min_value", min_value), ("max_value", max_value)):
        if value is not None:
            _check_type(name, value, dt.time)
            if value.tzinfo is not None:
                raise InvalidArgument(f"{name}={value!r} must be naive")
    engine_precision, _ = _precision(precision)
    result = Times(min_value, max_value, None if zones is None else as_generator(zones), precision)
    low, high = result.bounds
    _check_order(low, high)
    _native.validate_time(_time_fields(low), _time_fields(high), engine_precision)
    return result


def tzdb_version() -> str:
    """The IANA release of the time zone database bundled in Hegel's engine,
    such as ``"2026e"``.

    The bundled database is what :meth:`ZoneSource.bundled` draws against,
    and it always supplies the zone names' tags and shrink order. Zoned
    draws use the default zone source, :mod:`zoneinfo`'s database, whose
    release :attr:`ZoneSource.version` reports.
    """
    return _native.tzdb_version()


_MAX_ZONE_OFFSET = dt.timedelta(days=2) - _SECOND
_MAX_RULE_TIME = dt.timedelta(hours=167)


def _check_whole_seconds(name: str, value: Any, limit: dt.timedelta) -> None:
    _check_type(name, value, dt.timedelta)
    if value % _SECOND or not -limit <= value <= limit:
        raise InvalidArgument(
            f"{name}={value!r} must be a whole number of seconds, at most {limit} from zero"
        )


def _check_int(name: str, value: Any, lowest: int, highest: int) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or not lowest <= value <= highest:
        raise InvalidArgument(f"{name}={value!r} must be an int from {lowest} to {highest}")


@dataclass(frozen=True)
class ZoneType:
    """A local time type of a zone: its UTC offset, whether it is daylight
    saving time, and its abbreviation, such as
    ``ZoneType(dt.timedelta(hours=1), True, "BST")``. The offset is a whole
    number of seconds less than two days from zero."""

    offset: dt.timedelta
    is_dst: bool = False
    abbreviation: str = ""

    def __post_init__(self) -> None:
        _check_whole_seconds("offset", self.offset, _MAX_ZONE_OFFSET)
        _check_type("is_dst", self.is_dst, bool)
        _check_type("abbreviation", self.abbreviation, str)

    def _native(self) -> tuple[int, bool, str]:
        return self.offset // _SECOND, self.is_dst, self.abbreviation


_RULE_DAYS: dict[str, int] = {
    "julian": _native.ZONE_RULE_DAY_JULIAN,
    "zero_based": _native.ZONE_RULE_DAY_ZERO_BASED,
    "month_week_day": _native.ZONE_RULE_DAY_MONTH_WEEK_DAY,
}


@dataclass(frozen=True)
class RuleDate:
    """When a yearly daylight-saving rule changes the clocks: ``time``
    after local midnight (in the offset in force before the change) of a
    day of the year, as the ``Jn``, ``n`` and ``Mm.w.d`` forms of a POSIX
    TZ string name it. ``time`` is a whole number of seconds from -167 to
    167 hours, the range a TZif file allows. Build one with
    :meth:`julian`, :meth:`zero_based` or :meth:`month_week_day`."""

    kind: str
    day: int = 0
    month: int = 0
    week: int = 0
    weekday: int = 0
    time: dt.timedelta = dt.timedelta(hours=2)

    def __post_init__(self) -> None:
        _check_choice("kind", self.kind, _RULE_DAYS)
        if self.kind == "month_week_day":
            _check_int("month", self.month, 1, 12)
            _check_int("week", self.week, 1, 5)
            _check_int("weekday", self.weekday, 0, 6)
        else:
            _check_int("day", self.day, 1 if self.kind == "julian" else 0, 365)
        _check_whole_seconds("time", self.time, _MAX_RULE_TIME)

    @classmethod
    def julian(cls, day: int, time: dt.timedelta = dt.timedelta(hours=2)) -> RuleDate:
        """Day ``day`` of the year, from 1 to 365, never counting February 29
        (``Jn``)."""
        return cls("julian", day=day, time=time)

    @classmethod
    def zero_based(cls, day: int, time: dt.timedelta = dt.timedelta(hours=2)) -> RuleDate:
        """Day ``day`` of the year counted from 0, from 0 to 365, counting
        February 29 in leap years (``n``)."""
        return cls("zero_based", day=day, time=time)

    @classmethod
    def month_week_day(
        cls, month: int, week: int, weekday: int, time: dt.timedelta = dt.timedelta(hours=2)
    ) -> RuleDate:
        """The ``week``-th ``weekday`` of ``month`` (``Mm.w.d``): ``week``
        from 1 to 5, 5 meaning the last, and ``weekday`` numbered as
        :meth:`datetime.date.weekday` numbers it, Monday 0 to Sunday 6."""
        return cls("month_week_day", month=month, week=week, weekday=weekday, time=time)

    def _native(self) -> tuple[int, int, int, int, int, int]:
        weekday = (self.weekday + 1) % 7 if self.kind == "month_week_day" else 0
        return (
            _RULE_DAYS[self.kind],
            self.day,
            self.month,
            self.week,
            weekday,
            self.time // _SECOND,
        )


@dataclass(frozen=True)
class ZoneRule:
    """A zone's rule after its last explicit transition: ``standard``
    always, or ``standard`` with ``dst`` in force each year from
    ``dst_start`` to ``dst_end``."""

    standard: ZoneType
    dst: ZoneType | None = None
    dst_start: RuleDate | None = None
    dst_end: RuleDate | None = None

    def __post_init__(self) -> None:
        _check_type("standard", self.standard, ZoneType)
        if self.dst is None:
            if self.dst_start is not None or self.dst_end is not None:
                raise InvalidArgument("dst_start and dst_end need a dst type")
            return
        _check_type("dst", self.dst, ZoneType)
        _check_type("dst_start", self.dst_start, RuleDate)
        _check_type("dst_end", self.dst_end, RuleDate)


@dataclass(frozen=True)
class ZoneData:
    """A zone's data, given as fields, as a TZif file holds it.

    ``initial`` is in force before the first of ``transitions``, a
    sequence of ``(instant, type)`` pairs strictly ascending by
    ``instant``, in seconds since the Unix epoch, each putting the
    :class:`ZoneType` ``type`` in force from then on. After the last
    transition the zone follows ``rule``, or, with ``infer_after`` set to
    the last year the transitions cover, a yearly rule the engine infers
    from that year's transitions and the year before's; with neither, the
    last type holds for ever.
    """

    initial: ZoneType
    transitions: Sequence[tuple[int, ZoneType]] = ()
    rule: ZoneRule | None = None
    infer_after: int | None = None

    def __post_init__(self) -> None:
        _check_type("initial", self.initial, ZoneType)
        if isinstance(self.transitions, (str, bytes)) or not isinstance(self.transitions, Iterable):
            raise InvalidArgument(f"transitions={self.transitions!r} must be a sequence")
        transitions = tuple(self.transitions)
        for transition in transitions:
            if (
                not isinstance(transition, tuple)
                or len(transition) != 2
                or not isinstance(transition[1], ZoneType)
            ):
                raise InvalidArgument(f"transition {transition!r} must be an (int, ZoneType) pair")
            _check_int("instant", transition[0], _I64_MIN, _I64_MAX)
        object.__setattr__(self, "transitions", transitions)
        if self.rule is not None:
            _check_type("rule", self.rule, ZoneRule)
            if self.infer_after is not None:
                raise InvalidArgument("pass rule or infer_after, not both")
        if self.infer_after is not None:
            _check_int("infer_after", self.infer_after, -999_999, 999_999)

    def _native(self) -> Any:
        transitions = [(instant, kind._native()) for instant, kind in self.transitions]
        after, standard, dst, dates = _native.ZONE_AFTER_HOLD, None, None, None
        rule = self.rule
        if rule is not None:
            after, standard = _native.ZONE_AFTER_RULE, rule.standard._native()
            if rule.dst is not None and rule.dst_start is not None and rule.dst_end is not None:
                dst = rule.dst._native()
                dates = (rule.dst_start._native(), rule.dst_end._native())
        elif self.infer_after is not None:
            after = _native.ZONE_AFTER_INFER
        last_year = 0 if self.infer_after is None else self.infer_after
        return (self.initial._native(), transitions, after, standard, dst, dates, last_year)


_ORACLE_AFTER: dict[str, int] = {
    "hold": _native.ZONE_AFTER_HOLD,
    "infer": _native.ZONE_AFTER_INFER,
}


@dataclass(frozen=True)
class ZoneOracle:
    """A zone given by a library that answers lookups but does not expose
    its data. ``offset_at(instant)`` returns the :class:`ZoneType` in
    force at ``instant`` (seconds since the Unix epoch), and the optional
    ``next_transition(instant)`` the first instant after it at which the
    type changes, or ``None`` when it never changes again.

    The engine probes the oracle over the years ``first_year`` to
    ``last_year``, asking ``next_transition`` for each transition in turn
    or, without it, sampling ``offset_at`` once a day and bisecting each
    change to the second. Before ``first_year`` the type at its start
    holds; after ``last_year``, ``after="hold"`` holds the last type and
    ``after="infer"`` infers a yearly rule from the last two years.
    """

    offset_at: Callable[[int], ZoneType]
    _: KW_ONLY
    first_year: int
    last_year: int
    after: str
    next_transition: Callable[[int], int | None] | None = None

    def __post_init__(self) -> None:
        if not callable(self.offset_at):
            raise InvalidArgument(f"offset_at={self.offset_at!r} must be callable")
        if self.next_transition is not None and not callable(self.next_transition):
            raise InvalidArgument(f"next_transition={self.next_transition!r} must be callable")
        _check_int("first_year", self.first_year, -999_999, 999_999)
        _check_int("last_year", self.last_year, -999_999, 999_999)
        _check_choice("after", self.after, _ORACLE_AFTER)

    def _native(self) -> Any:
        offset_at = self.offset_at

        def answer(instant: int) -> tuple[int, bool, str]:
            value = offset_at(instant)
            if not isinstance(value, ZoneType):
                raise InvalidArgument(f"offset_at({instant}) returned {value!r}, not a ZoneType")
            return value._native()

        return (
            answer,
            self.next_transition,
            self.first_year,
            self.last_year,
            _ORACLE_AFTER[self.after],
        )


_ZoneEntry = bytes | ZoneData | ZoneOracle | str


def _loader_entry(name: str, value: Any) -> tuple[str, Any] | None:
    if value is None:
        return None
    if isinstance(value, bytes):
        return ("tzif", value)
    if isinstance(value, ZoneData):
        return ("zone", value._native())
    if isinstance(value, ZoneOracle):
        return ("oracle", value._native())
    if isinstance(value, str):
        return ("link", value)
    raise InvalidArgument(
        f"the loader returned {value!r} for {name!r}: it must return TZif bytes, a ZoneData, "
        "a ZoneOracle, the name of a zone to link to, or None"
    )


_KEY_BASE = os.path.normpath(os.path.join("_", "_"))[:-1]


def _is_zoneinfo_key(name: str) -> bool:
    """Whether :mod:`zoneinfo` accepts ``name`` as a key: a relative path
    that is already normalized and stays inside the directory it is
    looked up in."""
    if os.path.isabs(name):
        return False
    normalized = os.path.normpath(name)
    return len(normalized) == len(name) and os.path.normpath(
        os.path.join(_KEY_BASE, normalized)
    ).startswith(_KEY_BASE)


def _read_zoneinfo_file(name: str) -> bytes:
    for directory in zoneinfo.TZPATH:
        path = os.path.join(directory, name)
        if os.path.isfile(path):
            with open(path, "rb") as file:
                return file.read()
    *package, resource = name.split("/")
    return resources.files(".".join(["tzdata.zoneinfo", *package])).joinpath(resource).read_bytes()


def _zoneinfo_tzif(name: str) -> bytes | None:
    """The TZif data :class:`zoneinfo.ZoneInfo` would load for ``name``,
    found as it finds it: in the directories of :data:`zoneinfo.TZPATH`,
    then in the ``tzdata`` package. ``None`` when it finds none."""
    if not _is_zoneinfo_key(name):
        return None
    try:
        data = _read_zoneinfo_file(name)
    except (ImportError, OSError, UnicodeEncodeError):
        return None
    return data if data[:4] == b"TZif" else None


def _directory_version(directory: str) -> str:
    try:
        with open(os.path.join(directory, "tzdata.zi"), encoding="utf-8") as file:
            first = file.readline().split()
        if first[:2] == ["#", "version"] and len(first) == 3:
            return first[2]
    except OSError:
        pass
    try:
        with open(os.path.join(directory, "+VERSION"), encoding="utf-8") as file:
            return file.read().strip()
    except OSError:
        return ""


def _zoneinfo_version() -> str:
    """The release of the database :mod:`zoneinfo` reads: that of the first
    directory of :data:`zoneinfo.TZPATH` that exists, or of the ``tzdata``
    package when none does."""
    for directory in zoneinfo.TZPATH:
        if os.path.isdir(directory):
            return _directory_version(directory)
    try:
        import tzdata  # type: ignore[import-untyped,unused-ignore]
    except ImportError:
        return ""
    return str(tzdata.IANA_VERSION)


_KINDS: dict[int, str] = {
    _native.TIMEZONE_KIND_UNIQUE: "unique",
    _native.TIMEZONE_KIND_GAP: "gap",
    _native.TIMEZONE_KIND_FOLD: "fold",
}


@dataclass(frozen=True)
class ZoneNameInfo:
    """What a :class:`ZoneSource` knows about a time zone name, from
    :meth:`ZoneSource.info`: ``primary_name``, the zone whose data applies
    (the link target for a link, the name itself otherwise); ``is_link``
    and ``is_legacy``, which the ``allow_links`` and ``allow_legacy``
    arguments of :func:`zone_names` go by; and ``rank``, the name's place
    in the order names shrink in, 0 for ``"UTC"``."""

    primary_name: str
    is_link: bool
    is_legacy: bool
    rank: int


@dataclass(frozen=True)
class ZoneResolution:
    """How a :class:`ZoneSource`'s data reads a wall-clock time in a zone,
    from :meth:`ZoneSource.resolve`.

    ``kind`` is ``"unique"`` for a wall time the zone's clocks showed
    once, ``"gap"`` for one they skipped and ``"fold"`` for one they
    showed twice. For a gap or a fold ``offset_before`` and
    ``offset_after`` are the UTC offsets in force before and after the
    transition; otherwise both are the offset in force. ``instant`` is
    the moment the wall time denotes, in UTC, read as :mod:`datetime`
    reads it: with ``offset_before`` when its ``fold`` is 0 and
    ``offset_after`` when it is 1.
    """

    kind: str
    offset_before: dt.timedelta
    offset_after: dt.timedelta
    instant: dt.datetime


class ZoneSource:
    """The time zone data zoned draws are generated against: which zones
    exist, and their transitions and rules, so that the gaps and folds the
    engine aims at are those of the database the code under test uses.

    By default every draw uses :meth:`zoneinfo`, :mod:`zoneinfo`'s own
    database; :func:`set_default_zone_source` changes that, and
    :func:`zones`, :func:`zone_names`, :func:`datetimes` and
    :func:`zoned_datetimes` take a ``source`` for one generator.
    :meth:`bundled` is the engine's own database.

    ``ZoneSource()`` is an empty source, for an integration with another
    date and time library to fill: with :meth:`add_tzif`,
    :meth:`add_zone`, :meth:`add_oracle` and :meth:`add_link`, or lazily,
    by declaring the names it knows with :meth:`declare` and supplying each
    zone's data when a draw first needs it through :meth:`set_loader`.
    Whatever its shape, the engine checks the data, and an error in it
    raises :class:`~hegel.InvalidArgument`. :meth:`info`, :meth:`resolve`
    and :meth:`offset_at` report what the source holds, and
    ``name in source`` whether it knows a name.

    Build a source once, at module level or in a fixture, rather than in
    a test body, and fill it before building generators from it: a source
    may be filled while it is in use, but a generator reads the names its
    source knows when the generator is built, so a name added afterwards
    is drawn only by generators built after that.
    """

    def __init__(self) -> None:
        self._source = _native.ZoneSource()
        self._name = "ZoneSource()"
        self._lock = threading.Lock()
        self._prepare: Callable[[], None] | None = None
        self._empty = False

    @classmethod
    def _of(cls, native: _native.ZoneSource, name: str) -> ZoneSource:
        source = cls()
        source._source = native
        source._name = name
        return source

    @staticmethod
    def bundled() -> ZoneSource:
        """The IANA database bundled in Hegel's engine, of release
        :func:`tzdb_version`, the same on every machine. It cannot be
        changed."""
        return ZoneSource._of(_native.ZoneSource.bundled(), "ZoneSource.bundled()")

    @staticmethod
    def zoneinfo() -> ZoneSource:
        """The database :mod:`zoneinfo` reads, the default source.

        Its names are those :func:`zoneinfo.available_timezones` lists, and
        each zone's data is read from the TZif file
        :class:`zoneinfo.ZoneInfo` would load for it (from the directories
        of :data:`zoneinfo.TZPATH`, then from the ``tzdata`` package) the
        first time a draw needs it, so a zone a test draws has exactly the
        transitions :mod:`zoneinfo` gives it. The names and the
        :attr:`version` are read when the source is first used, and the
        files when each zone is, so a source made before
        :func:`zoneinfo.reset_tzpath` changes the search path reads its
        zones from the new one.

        Where :mod:`zoneinfo` finds no database (Windows without the
        ``tzdata`` package, say) the source has no names, and building a
        generator that draws its zones is an
        :class:`~hegel.InvalidArgument`.
        """
        source = ZoneSource._of(_native.ZoneSource(), "ZoneSource.zoneinfo()")
        source._source.set_loader(lambda name: _loader_entry(name, _zoneinfo_tzif(name)))

        def prepare() -> None:
            names = sorted(zoneinfo.available_timezones())
            source._empty = not names
            source._source.declare(names)
            source._source.set_version(_zoneinfo_version())

        source._prepare = prepare
        return source

    def _handle(self) -> _native.ZoneSource:
        with self._lock:
            if self._prepare is not None:
                self._prepare()
                self._prepare = None
        return self._source

    @property
    def version(self) -> str:
        """The release of the source's database: :func:`tzdb_version` for
        :meth:`bundled`; for :meth:`zoneinfo`, the release the zone files'
        directory records in its ``tzdata.zi`` or ``+VERSION``, or
        ``tzdata.IANA_VERSION`` when the files come from the ``tzdata``
        package; otherwise what was set, or ``""``. Failure output does not
        report it."""
        return self._handle().version()

    @version.setter
    def version(self, version: str) -> None:
        _check_type("version", version, str)
        self._handle().set_version(version)

    def add_tzif(self, name: str, data: bytes) -> None:
        """Add the zone ``name`` from the contents of a TZif file (version
        2 or later), such as one under ``/usr/share/zoneinfo``."""
        _check_type("name", name, str)
        _check_type("data", data, bytes)
        self._handle().add_tzif(name, data)

    def add_zone(self, name: str, data: ZoneData) -> None:
        """Add the zone ``name`` from its data given as fields."""
        _check_type("name", name, str)
        _check_type("data", data, ZoneData)
        self._handle().add_zone(name, data._native())

    def add_oracle(self, name: str, oracle: ZoneOracle) -> None:
        """Add the zone ``name`` by probing ``oracle`` now."""
        _check_type("name", name, str)
        _check_type("oracle", oracle, ZoneOracle)
        self._handle().add_oracle(name, *oracle._native())

    def add_link(self, name: str, target: str) -> None:
        """Make ``name`` another name for the zone ``target``, which need
        not have been added yet."""
        _check_type("name", name, str)
        _check_type("target", target, str)
        self._handle().add_link(name, target)

    def declare(self, names: Iterable[str]) -> None:
        """Declare names the source knows whose data comes later, from the
        loader or from an ``add_*`` call made before a draw needs it."""
        if isinstance(names, (str, bytes)) or not isinstance(names, Iterable):
            raise InvalidArgument(f"names={names!r} must be an iterable of strings")
        checked = list(names)
        for name in checked:
            _check_type("name", name, str)
        self._handle().declare(checked)

    def set_loader(self, loader: Callable[[str], _ZoneEntry | None] | None) -> None:
        """Supply a zone's data the first time a draw or query needs it:
        ``loader(name)`` returns the zone's TZif bytes, a :class:`ZoneData`,
        a :class:`ZoneOracle`, the name of the zone it is a link to, or
        ``None`` for a name it does not know.

        The loader is asked for declared names and for any other name a
        generator or query names. What it returns for a name is kept, so
        it is normally asked once per name, but it may be asked for the
        same name again: from two threads that need it at once, and after
        a call that raised. It may be called from any thread that draws,
        several at once, and must not draw itself. An exception it raises
        fails the draw or query with :class:`~hegel.InvalidArgument`, whose
        ``__cause__`` is that exception.

        A new loader replaces the previous one, and ``None`` removes it.
        The source holds the loader until then, or until nothing uses the
        source any more.
        """
        if loader is None:
            self._handle().set_loader(None)
            return
        if not callable(loader):
            raise InvalidArgument(f"loader={loader!r} must be callable")
        self._handle().set_loader(lambda name: _loader_entry(name, loader(name)))

    def info(self, name: str) -> ZoneNameInfo | None:
        """What the source knows about the name ``name``, or ``None`` for a
        name it does not know. A name the source has declared but not yet
        loaded is loaded through the loader when describing it needs its
        data."""
        _check_type("name", name, str)
        info = _native.timezone_info(name, self._handle())
        if info is None:
            return None
        primary_name, is_link, rank, tags = info
        return ZoneNameInfo(primary_name, is_link, bool(tags & _native.TIMEZONE_TAG_LEGACY), rank)

    def __contains__(self, name: object) -> bool:
        """Whether the source knows the time zone name ``name``."""
        return isinstance(name, str) and self.info(name) is not None

    def resolve(self, name: str, wall_time: dt.datetime) -> ZoneResolution:
        """Read the naive datetime ``wall_time`` as a wall-clock time in
        the zone ``name``: whether the zone's clocks showed it once, never
        or twice, the offsets around it, and the moment it denotes given
        its ``fold``. An integration checks the data it loaded with this,
        by comparing a sample of wall times with its library's answers.

        A name the source does not know, an aware ``wall_time``, or a
        moment outside the range :mod:`datetime` represents is an
        :class:`~hegel.InvalidArgument`.
        """
        _check_type("name", name, str)
        _check_type("wall_time", wall_time, dt.datetime)
        if wall_time.tzinfo is not None:
            raise InvalidArgument(f"wall_time={wall_time!r} must be naive")
        kind, before, after, _, _, (seconds, nanoseconds) = _native.timezone_resolve(
            name, _components(wall_time), self._handle()
        )
        if wall_time.fold:
            seconds += before - after
        return ZoneResolution(
            _KINDS[kind],
            dt.timedelta(seconds=before),
            dt.timedelta(seconds=after),
            _moment("wall_time", wall_time, seconds, nanoseconds),
        )

    def offset_at(self, name: str, instant: dt.datetime) -> ZoneType:
        """The :class:`ZoneType` (UTC offset, daylight-saving flag and
        abbreviation) the zone ``name`` has in force at the aware datetime
        ``instant``. A name the source does not know or a naive
        ``instant`` is an :class:`~hegel.InvalidArgument`."""
        _check_type("name", name, str)
        _check_type("instant", instant, dt.datetime)
        if instant.tzinfo is None:
            raise InvalidArgument(f"instant={instant!r} must be aware")
        offset, _, is_dst, abbreviation = _native.timezone_offset_at(
            name, _timestamp(instant), self._handle()
        )
        return ZoneType(dt.timedelta(seconds=offset), is_dst, abbreviation)

    def __repr__(self) -> str:
        return self._name


_default_source = ZoneSource.zoneinfo()
_default_lock = threading.Lock()
_native.set_default_zone_source(_default_source._source)


def default_zone_source() -> ZoneSource:
    """The zone source a generator uses when not given one:
    :meth:`ZoneSource.zoneinfo` unless :func:`set_default_zone_source`
    set another. A generator reads it once, when it is built."""
    return _default_source


def set_default_zone_source(source: ZoneSource) -> ZoneSource:
    """Make ``source`` the zone source of every generator built from now on
    without one of its own, and return the source it replaces, to restore
    afterwards.

    A generator reads the default when it is built and keeps what it
    read, so set it before building generators: at the top of
    ``conftest.py``, say, before test modules that build generators at
    import time are loaded. It is one setting for the whole process, all
    threads included.
    """
    global _default_source
    _check_type("source", source, ZoneSource)
    with _default_lock:
        previous = _default_source
        _default_source = source
        _native.set_default_zone_source(source._source)
    return previous


def _check_source(source: Any) -> ZoneSource:
    if source is None:
        return default_zone_source()
    _check_type("source", source, ZoneSource)
    return source


def _check_names(names: Any) -> tuple[str, ...]:
    if isinstance(names, (str, bytes)) or not isinstance(names, Iterable):
        raise InvalidArgument(f"names={names!r} must be an iterable of IANA time zone names")
    checked = tuple(names)
    for name in checked:
        if not isinstance(name, str):
            raise InvalidArgument(f"names={names!r} must contain strings, not {name!r}")
    return checked


class ZoneNames(_PrintsValue[str]):
    def __init__(
        self,
        names: tuple[str, ...] | None,
        allow_links: bool,
        allow_legacy: bool,
        source: ZoneSource | None,
    ) -> None:
        self.names = names
        self.allow_links = allow_links
        self.allow_legacy = allow_legacy
        self.given_source = source
        self.source = _check_source(source)
        handle = self.source._handle()
        if self.source._empty:
            raise InvalidArgument(
                "zoneinfo finds no time zones on this system. On a system without a time zone "
                "database (Windows, say) install the tzdata package."
            )
        self.spec = _native.TimeZoneSpec(
            None if names is None else list(names), allow_links, allow_legacy, handle
        )
        self.label = ZONE_NAME_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_timezone(self.spec)

    def arguments(self, **more: Any) -> str:
        return _arguments(
            names=None if self.names is None else list(self.names),
            **more,
            allow_links=_unless(self.allow_links, True),
            allow_legacy=_unless(self.allow_legacy, False),
            source=self.given_source,
        )

    def __repr__(self) -> str:
        return f"temporal.zone_names({self.arguments()})"


def _zone_names(names: Any, allow_links: Any, allow_legacy: Any, source: Any) -> ZoneNames:
    _check_type("allow_links", allow_links, bool)
    _check_type("allow_legacy", allow_legacy, bool)
    if names is None:
        return ZoneNames(None, allow_links, allow_legacy, source)
    if not allow_links or allow_legacy:
        raise InvalidArgument(
            "allow_links and allow_legacy choose among all the names of the source: "
            "they do not apply to an explicit names list, so pass one or the other"
        )
    return ZoneNames(_check_names(names), True, False, source)


def zone_names(
    *,
    names: Iterable[str] | None = None,
    allow_links: bool = True,
    allow_legacy: bool = False,
    source: ZoneSource | None = None,
) -> Generator[str]:
    """Generate IANA time zone names, such as ``"America/New_York"``,
    shrinking toward ``"UTC"``.

    The names are those the zone ``source`` knows, by default
    :func:`default_zone_source`, which is :meth:`ZoneSource.zoneinfo`
    unless changed: the names :func:`zoneinfo.available_timezones` lists,
    so every name generated can be loaded with :class:`zoneinfo.ZoneInfo`.
    On a system without a time zone database (Windows, say) install the
    ``tzdata`` package, or pass ``source=ZoneSource.bundled()`` for the
    names of the engine's own database. The ``posix/`` and ``right/``
    variants some systems carry are never generated. By default link
    names such as ``"Asia/Calcutta"`` are included and legacy names such
    as ``"EST5EDT"`` and ``"US/Eastern"`` are excluded; ``allow_links``
    and ``allow_legacy`` switch each.

    ``names`` restricts the draw to those names, each of which the source
    must know. ``allow_links`` and ``allow_legacy`` do not apply to an
    explicit list, and passing either with one is an error.

    The generator reads the default source, and the names its source
    knows, when it is built: a source changed afterwards does not change
    what it draws.

    Half the draws are uniform over the names and half weighted toward
    zones with a rare feature (a fractional offset, negative daylight
    saving, a skipped day, ...), so each is reached by a short run. A name
    shrinks to a zone a reader recognises: real zones with a whole-hour
    offset and a simple daylight-saving rule first, then the other real
    zones, then the ``Etc/GMT`` family, then the legacy names. Which
    names are links or legacy, their features and their order come from
    Hegel's bundled database, so they are the same on every machine; a
    name it lacks comes last, with the features of the source's data for
    it.

    A ``names`` list that is empty or holds a name the source does not
    know, or a source with no name the flags allow, is an
    :class:`~hegel.InvalidArgument`.
    """
    return _zone_names(names, allow_links, allow_legacy, source)


class Zones(_PrintsValue[zoneinfo.ZoneInfo]):
    def __init__(self, keys: ZoneNames, no_cache: bool) -> None:
        self.keys = keys
        self.no_cache = no_cache
        self.label = ZONE_LABEL

    def load(self, key: str) -> zoneinfo.ZoneInfo:
        try:
            return zoneinfo.ZoneInfo.no_cache(key) if self.no_cache else zoneinfo.ZoneInfo(key)
        except (zoneinfo.ZoneInfoNotFoundError, ValueError):
            raise InvalidArgument(
                f"zones() drew {key!r} from {self.keys.source!r}, but this Python's zoneinfo "
                "cannot load it; draw names that are not zoneinfo's with zone_names()"
            ) from None

    def do_draw(self, tc: TestCase) -> zoneinfo.ZoneInfo:
        return self.load(tc._generate_timezone(self.keys.spec))

    def __repr__(self) -> str:
        return f"temporal.zones({self.keys.arguments(no_cache=_unless(self.no_cache, False))})"


def zones(
    *,
    names: Iterable[str] | None = None,
    no_cache: bool = False,
    allow_links: bool = True,
    allow_legacy: bool = False,
    source: ZoneSource | None = None,
) -> Generator[zoneinfo.ZoneInfo]:
    """Generate IANA time zones, as :class:`zoneinfo.ZoneInfo` objects,
    shrinking toward UTC.

    The zones are those :func:`zone_names` names, with the same
    ``names``, ``allow_links``, ``allow_legacy`` and ``source`` arguments;
    a name in ``names`` must also be one this Python can load. With
    ``no_cache=True`` each zone is built with
    :meth:`zoneinfo.ZoneInfo.no_cache`, so it is not the same object as
    ``ZoneInfo(key)``.

    :func:`datetimes` and :func:`zoned_datetimes` draw their wall times
    against the source's data for the zone, so a source other than
    :meth:`ZoneSource.zoneinfo` aims at gaps and folds that the
    :class:`~zoneinfo.ZoneInfo` attached to the value may not have. A name
    the source draws that :mod:`zoneinfo` cannot load is an
    :class:`~hegel.InvalidArgument`.
    """
    _check_type("no_cache", no_cache, bool)
    keys = _zone_names(names, allow_links, allow_legacy, source)
    for name in keys.names or ():
        try:
            zoneinfo.ZoneInfo(name)
        except (zoneinfo.ZoneInfoNotFoundError, ValueError) as error:
            raise InvalidArgument(
                f"{name!r} is not a time zone this Python can load: {error}"
            ) from None
    return Zones(keys, no_cache)


def _instant(value: dt.datetime) -> dt.timedelta:
    """An aware datetime as the exact time since the Unix epoch."""
    return value - _EPOCH


def _timestamp(value: dt.datetime) -> tuple[int, int]:
    """An aware datetime as the engine's ``(seconds, nanoseconds)``."""
    microseconds = _instant(value) // _MICROSECOND
    seconds, remainder = divmod(microseconds, 1_000_000)
    return seconds, remainder * 1000


def _moment(name: str, value: Any, seconds: int, nanoseconds: int) -> dt.datetime:
    """The engine's ``(seconds, nanoseconds)`` as a UTC datetime, which
    the argument ``name=value`` it was computed from must leave in
    range."""
    try:
        return _EPOCH + dt.timedelta(seconds=seconds, microseconds=nanoseconds // 1000)
    except OverflowError:
        raise InvalidArgument(
            f"{name}={value!r} denotes a moment outside the range datetime represents"
        ) from None


def _is_imaginary(value: dt.datetime) -> bool:
    try:
        return value.astimezone(dt.UTC).astimezone(value.tzinfo).replace(tzinfo=None) != (
            value.replace(tzinfo=None)
        )
    except (OverflowError, ValueError):
        return False


def _is_repeated(value: dt.datetime) -> bool:
    """Whether the wall time of ``value`` happened twice in its zone, as
    far as the zone's ``tzinfo`` says: its offset depends on ``fold``,
    and the wall time is not one the clocks skipped."""
    return value.replace(fold=0).utcoffset() != value.replace(fold=1).utcoffset() and (
        not _is_imaginary(value)
    )


def _in_tzinfo(tc: TestCase, moment: dt.datetime, tz: dt.tzinfo) -> dt.datetime:
    """``moment`` in the zone ``tz``, rejecting the test case when its
    wall time there is outside the years :mod:`datetime` represents."""
    try:
        value = moment.astimezone(tz)
    except OverflowError:
        tc.reject()
    return value


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
        name: str,
        min_value: dt.datetime | None,
        max_value: dt.datetime | None,
        bounds: tuple[dt.datetime, dt.datetime],
        given_zones: Generator[Any] | None,
        zones: Generator[Any] | None,
        given_source: ZoneSource | None,
        allow_imaginary: bool,
        ambiguous: str,
        precision: str,
    ) -> None:
        self.name = name
        self.min_value = min_value
        self.max_value = max_value
        self.bounds = bounds
        self.given_zones = given_zones
        self.zones = zones
        self.given_source = given_source
        self.source = _check_source(given_source)
        self.allow_imaginary = allow_imaginary
        self.ambiguous = ambiguous
        self.precision = precision
        self.engine_precision, unit = _PRECISIONS[precision]
        self.unit = dt.timedelta(microseconds=unit)
        self.aware = bounds[0].tzinfo is not None
        self.require_zone = self.aware or name == "zoned_datetimes"
        low, high = bounds
        self.zoned_spec = (
            None
            if zones is None
            else _native.ZonedSpec(
                None if self.aware else (_components(low), _components(high)),
                (_timestamp(low), _timestamp(high)) if self.aware else None,
                self.engine_precision,
                allow_imaginary,
                _AMBIGUOUS[ambiguous],
            )
        )
        own_label = ZONED_DATETIME_LABEL if name == "zoned_datetimes" else DATETIME_LABEL
        self.label = own_label if zones is None else combine_labels([own_label, zones.label])

    def do_draw(self, tc: TestCase) -> dt.datetime:
        if self.zones is None or self.zoned_spec is None:
            return tc._generate_datetime(*self.bounds, self.engine_precision)
        if isinstance(self.zones, Zones):
            key, value = tc._generate_zoned_datetime(self.zones.keys.spec, self.zoned_spec)
            return value.replace(tzinfo=self.zones.load(key))
        tz = tc.draw_silent(self.zones)
        if tz is None:
            if self.require_zone:
                reason = "with aware bounds" if self.aware else f"from {self.name}()"
                raise InvalidArgument(
                    f"zones={self.zones!r} generated None, but {reason} every datetime "
                    "needs a tzinfo"
                )
            return tc._generate_datetime(*self.bounds, self.engine_precision)
        if not isinstance(tz, dt.tzinfo):
            raise InvalidArgument(f"zones must generate tzinfo objects, got {tz!r}")
        name = getattr(tz, "key", None)
        if isinstance(tz, zoneinfo.ZoneInfo) and isinstance(name, str):
            return self.draw_in_zone(tc, tz, name, self.zoned_spec)
        if self.aware:
            return self.draw_moment_in_tzinfo(tc, tz)
        return self.draw_wall_time_in_tzinfo(tc, tz)

    def draw_in_zone(
        self, tc: TestCase, tz: zoneinfo.ZoneInfo, key: str, zoned_spec: _native.ZonedSpec
    ) -> dt.datetime:
        """Draw in a :class:`zoneinfo.ZoneInfo` that came from a generator
        other than :func:`zones`, against the source's data for its key."""
        try:
            value = tc._generate_zoned_datetime_for_zone(self.source._handle(), key, zoned_spec)
        except InvalidArgument:
            if key in self.source:
                raise
            raise InvalidArgument(
                f"zones generated {tz!r}, but {self.source!r} has no time zone named {key!r}; "
                f"pass the zone source that has it as source= to {self.name}()"
            ) from None
        return value.replace(tzinfo=tz)

    def draw_wall_time_in_tzinfo(self, tc: TestCase, tz: dt.tzinfo) -> dt.datetime:
        """Draw a wall time within naive bounds in a ``tzinfo`` the zone
        source has no data for, going by what the ``tzinfo`` itself says
        of it."""
        value = tc._generate_datetime(*self.bounds, self.engine_precision).replace(tzinfo=tz)
        if not self.allow_imaginary and _is_imaginary(value):
            tc.reject()
        if not _is_repeated(value):
            return value
        if self.ambiguous == "both":
            return value.replace(fold=int(tc._generate_boolean()))
        return value.replace(fold=int(self.ambiguous == "later"))

    def draw_moment_in_tzinfo(self, tc: TestCase, tz: dt.tzinfo) -> dt.datetime:
        """Draw a moment within aware bounds and express it in a ``tzinfo``
        the zone source has no data for. The moment is drawn as a whole
        number of the precision's unit in UTC and moved forward to the
        next wall time in ``tz`` that is one, which is a different moment
        only where the zone's offset is not a whole number of the unit.
        The test case is rejected when that passes ``max_value`` or
        crosses a change of the zone's offset, and when ``ambiguous``
        asks for a reading of a repeated wall time that is outside the
        bounds."""
        low, high = max(self.bounds[0], _MIN_INSTANT), min(self.bounds[1], _MAX_INSTANT)
        if low > high:
            tc.reject()
        first = low - (low - _EPOCH) % self.unit
        seconds, nanoseconds = tc._generate_instant(
            _timestamp(first), _timestamp(high), self.engine_precision
        )
        moment = _EPOCH + dt.timedelta(seconds=seconds, microseconds=nanoseconds // 1000)
        value = _in_tzinfo(tc, moment, tz)
        shift = -self.excess(value) % self.unit
        if shift < low - moment:
            shift += self.unit
        if shift > high - moment:
            tc.reject()
        if shift:
            value = _in_tzinfo(tc, moment + shift, tz)
            if self.excess(value):
                tc.reject()
        if self.ambiguous != "both" and _is_repeated(value):
            value = value.replace(fold=int(self.ambiguous == "later"))
            if not low <= value <= high:
                tc.reject()
        return value

    def excess(self, value: dt.datetime) -> dt.timedelta:
        """How far the wall time of ``value`` is past a whole number of
        the precision's unit."""
        return (value.replace(tzinfo=None) - dt.datetime.min) % self.unit

    def __repr__(self) -> str:
        arguments = _arguments(
            min_value=self.min_value,
            max_value=self.max_value,
            zones=self.given_zones,
            source=self.given_source,
            allow_imaginary=_unless(self.allow_imaginary, True),
            ambiguous=_unless(self.ambiguous, "both"),
            precision=_unless(self.precision, "microseconds"),
        )
        return f"temporal.{self.name}({arguments})"


def _datetimes(
    name: str,
    min_value: dt.datetime | None,
    max_value: dt.datetime | None,
    zones: Generator[Any] | None,
    source: ZoneSource | None,
    allow_imaginary: bool,
    ambiguous: str,
    precision: str,
) -> Datetimes:
    _check_type("allow_imaginary", allow_imaginary, bool)
    _check_choice("ambiguous", ambiguous, _AMBIGUOUS)
    engine_precision, _ = _precision(precision)
    if source is not None:
        _check_type("source", source, ZoneSource)
    given = (("min_value", min_value), ("max_value", max_value))
    for bound, value in given:
        if value is not None:
            _check_type(bound, value, dt.datetime)
    aware = any(value is not None and value.tzinfo is not None for _, value in given)
    if aware:
        for bound, value in given:
            if value is not None and value.tzinfo is None:
                raise InvalidArgument(
                    f"{bound}={value!r} is naive, but the other bound is timezone-aware; "
                    "the bounds must be both naive or both aware"
                )
    low = (_MIN_INSTANT if aware else dt.datetime.min) if min_value is None else min_value
    high = (_MAX_INSTANT if aware else dt.datetime.max) if max_value is None else max_value
    if aware and _instant(high) < _instant(low):
        raise InvalidArgument(
            f"Cannot have max_value={max_value!r} < min_value={min_value!r}, "
            "comparing as moments in time"
        )
    if not aware:
        _check_order(low, high)
    given_zones = None if zones is None else as_generator(zones)
    drawn_zones = given_zones
    if isinstance(given_zones, Zones):
        if source is not None:
            raise InvalidArgument(
                f"source={source!r} cannot be given with zones={given_zones!r}, which draws "
                "from its own zone source: pass source to zones() instead"
            )
    elif given_zones is None:
        if aware or name == "zoned_datetimes":
            drawn_zones = Zones(ZoneNames(None, True, False, source), False)
        elif source is not None:
            raise InvalidArgument(
                f"source={source!r} has no effect: {name}() draws naive datetimes unless "
                "given zones or aware bounds"
            )
        else:
            _native.validate_datetime(_components(low), _components(high), engine_precision)
    return Datetimes(
        name,
        min_value,
        max_value,
        (low, high),
        given_zones,
        drawn_zones,
        source,
        allow_imaginary,
        ambiguous,
        precision,
    )


def datetimes(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    zones: Generator[Any] | None = None,
    source: ZoneSource | None = None,
    allow_imaginary: bool = True,
    ambiguous: str = "both",
    precision: str = "microseconds",
) -> Generator[dt.datetime]:
    """Generate :class:`datetime.datetime` objects, naive or aware, in
    ``[min_value, max_value]``, shrinking toward 2000-01-01 00:00 (or the
    bound nearest it).

    **Naive bounds.** With naive bounds, or none (a missing bound is
    :attr:`datetime.min <datetime.datetime.min>` or
    :attr:`datetime.max <datetime.datetime.max>`), the bounds limit the
    wall-clock time. The datetimes are naive unless ``zones`` is
    given: a generator of :class:`datetime.tzinfo` objects (or ``None``),
    such as :func:`zones`, one of which is attached to each drawn
    wall-clock time::

        aware = tc.draw(gs.temporal.datetimes(zones=gs.temporal.zones()))

    **Aware bounds.** With aware bounds the bounds are moments in time,
    compared as such however their time zones differ, and every datetime
    is aware and lies between them in a zone drawn from ``zones``,
    which defaults to :func:`zones` and must not generate ``None``::

        this_year = gs.temporal.datetimes(
            dt.datetime(2026, 1, 1, tzinfo=dt.UTC), dt.datetime(2027, 1, 1, tzinfo=dt.UTC)
        )

    One bound may be omitted; passing one naive and one aware bound is an
    error.

    **Zones and their data.** The engine draws the wall time in a zone
    against a :class:`ZoneSource`'s data for it, and which source that is
    depends on ``zones``:

    - With no ``zones`` (and aware bounds), the zones are
      ``zones(source=source)``.
    - With a :func:`zones` generator, its own source is used; set it with
      ``zones(source=...)``. Passing ``source`` here as well is an error.
    - With any other generator (``zones().filter(...)``,
      ``gs.sampled_from([...])``, ``gs.just(...)``), a
      :class:`zoneinfo.ZoneInfo` it generates is looked up by its key in
      ``source``, by default :func:`default_zone_source` as it was when
      this generator was built. A key that source does not know is an
      :class:`~hegel.InvalidArgument` naming the zone, so pass the source
      that has it.

    ``source`` without ``zones`` or aware bounds, when the datetimes are
    naive, is an error.

    **Transitions.** In a zone drawn against a source, one draw in twenty
    on average lands within a second, an hour or a day of one of the
    zone's daylight-saving or offset transitions, or of a well-known
    instant such as the Unix epoch or the end of the signed 32-bit epoch,
    so the skipped and repeated wall times where date bugs live are
    generated deliberately. A repeated wall time is drawn with either
    value of ``fold`` by default; ``ambiguous`` chooses ``"earlier"``
    (``fold=0``) or ``"later"`` (``fold=1``) instead of ``"both"``. A
    skipped wall time (one the clocks jumped over, which :mod:`datetime`
    represents but which never occurred) is drawn by default, because
    malformed timestamps are a common source of bugs; with
    ``allow_imaginary=False`` the engine draws a real wall time instead.
    With aware bounds, a zone that has no wall time between them at the
    precision rejects the test case.

    **Other tzinfo objects.** A fixed offset (:class:`datetime.timezone`)
    or any other ``tzinfo`` that is not a :class:`zoneinfo.ZoneInfo` with
    a key has no data in a zone source, so nothing aims at its
    transitions and Hegel goes by what the ``tzinfo`` itself reports.
    With naive bounds the drawn wall time is attached to it: a repeated
    one gets the ``fold`` that ``ambiguous`` asks for, and a skipped one
    rejects the test case under ``allow_imaginary=False``. With aware
    bounds a moment between them is drawn and converted to the zone, so
    it is never a skipped wall time, and a repeated one is given the
    reading ``ambiguous`` asks for; the test case is rejected when that
    reading, or the nearest wall time at the precision, lies outside the
    bounds. A ``tzinfo`` that ignores ``fold``, as ``pytz``'s do, never
    reports a repeated wall time.

    **Precision.** ``precision`` names the unit every drawn time of day
    is a whole number of, as the ``timespec`` argument of
    :meth:`~datetime.datetime.isoformat` does: ``"microseconds"`` (the
    default), ``"milliseconds"``, ``"seconds"``, ``"minutes"`` or
    ``"hours"``.

    Bounds that are not datetimes, a mix of a naive and an aware bound,
    ``min_value > max_value``, bounds holding no wall time at the
    precision, a misplaced ``source``, or a ``zones`` generator producing
    something other than ``tzinfo`` objects (or ``None`` with aware
    bounds) are an :class:`~hegel.InvalidArgument`.
    """
    return _datetimes(
        "datetimes", min_value, max_value, zones, source, allow_imaginary, ambiguous, precision
    )


def zoned_datetimes(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    zones: Generator[Any] | None = None,
    source: ZoneSource | None = None,
    allow_imaginary: bool = True,
    ambiguous: str = "both",
    precision: str = "microseconds",
) -> Generator[dt.datetime]:
    """Generate aware :class:`datetime.datetime` objects in
    ``[min_value, max_value]``, each in a zone drawn from ``zones``,
    shrinking toward 2000-01-01 00:00 (or the bound nearest it) in UTC.

    This is :func:`datetimes` for a test that wants only aware datetimes,
    and takes the same arguments: ``zones`` defaults to
    ``zones(source=source)`` and must not generate ``None``, naive bounds
    (or none) limit the wall-clock time in the drawn zone, and aware
    bounds are moments in time. The engine draws the zone and the wall
    time together, so one draw in twenty on average lands near one of the
    zone's transitions::

        meeting = tc.draw(gs.temporal.zoned_datetimes(dt.datetime(2026, 1, 1)))

    ``source`` is the :class:`ZoneSource` whose data the zones are drawn
    against, as for :func:`datetimes`: that of the default ``zones``, or
    the one a :class:`zoneinfo.ZoneInfo` from a generator other than
    :func:`zones` is looked up in. Invalid arguments are those of
    :func:`datetimes`.
    """
    return _datetimes(
        "zoned_datetimes",
        min_value,
        max_value,
        zones,
        source,
        allow_imaginary,
        ambiguous,
        precision,
    )


class Instants(_PrintsValue[dt.datetime]):
    def __init__(
        self, min_value: dt.datetime | None, max_value: dt.datetime | None, precision: str
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.precision = precision
        self.bounds = (
            _MIN_INSTANT if min_value is None else max(min_value, _MIN_INSTANT),
            _MAX_INSTANT if max_value is None else min(max_value, _MAX_INSTANT),
        )
        self.label = INSTANT_LABEL

    def do_draw(self, tc: TestCase) -> dt.datetime:
        low, high = self.bounds
        seconds, nanoseconds = tc._generate_instant(
            _timestamp(low), _timestamp(high), _PRECISIONS[self.precision][0]
        )
        return _EPOCH + dt.timedelta(seconds=seconds, microseconds=nanoseconds // 1000)

    def __repr__(self) -> str:
        arguments = _arguments(
            min_value=self.min_value,
            max_value=self.max_value,
            precision=_unless(self.precision, "microseconds"),
        )
        return f"temporal.instants({arguments})"


def instants(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    precision: str = "microseconds",
) -> Generator[dt.datetime]:
    """Generate moments in time, as :class:`datetime.datetime` objects in
    UTC (``tzinfo`` is :data:`datetime.UTC`), in ``[min_value, max_value]``,
    shrinking toward 2000-01-01 00:00 UTC (or the bound nearest it).

    Either bound may be omitted. A bound given must be aware, and bounds
    are compared as moments in time however their time zones differ; a
    missing bound is the end of the range a UTC
    :class:`~datetime.datetime` represents, and a bound beyond that range
    is clamped to it. One draw in twenty on average targets an instant
    tests tend to get wrong: the Unix epoch and its neighbours, the 32-bit
    and 64-bit second boundaries, leap days, or a transition of any time
    zone in Hegel's database.

    ``precision`` names the unit every drawn instant is a whole number of
    since the Unix epoch: ``"microseconds"`` (the default),
    ``"milliseconds"``, ``"seconds"``, ``"minutes"`` or ``"hours"``.

    Bounds that are not aware datetimes, ``min_value > max_value``, or
    bounds holding no instant at the precision in the UTC range are an
    :class:`~hegel.InvalidArgument`.
    """
    for name, value in (("min_value", min_value), ("max_value", max_value)):
        if value is not None:
            _check_type(name, value, dt.datetime)
            if value.tzinfo is None:
                raise InvalidArgument(
                    f"{name}={value!r} must be aware: instants() bounds are moments"
                )
    if (
        min_value is not None
        and max_value is not None
        and _instant(max_value) < _instant(min_value)
    ):
        raise InvalidArgument(
            f"Cannot have max_value={max_value!r} < min_value={min_value!r}, "
            "comparing as moments in time"
        )
    engine_precision, _ = _precision(precision)
    result = Instants(min_value, max_value, precision)
    low, high = result.bounds
    if low > high:
        raise InvalidArgument(
            f"no moment between min_value={min_value!r} and max_value={max_value!r} is in "
            "the range a UTC datetime represents"
        )
    _native.validate_timestamp(_timestamp(low), _timestamp(high), engine_precision)
    return result


class Durations(_PrintsValue[dt.timedelta]):
    def __init__(
        self, min_value: dt.timedelta | None, max_value: dt.timedelta | None, precision: str
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.bounds = (
            dt.timedelta.min if min_value is None else min_value,
            dt.timedelta.max if max_value is None else max_value,
        )
        self.precision = precision
        self.label = DURATION_LABEL

    def do_draw(self, tc: TestCase) -> dt.timedelta:
        return tc._generate_duration(*self.bounds, _PRECISIONS[self.precision][0])

    def __repr__(self) -> str:
        arguments = _arguments(
            min_value=self.min_value,
            max_value=self.max_value,
            precision=_unless(self.precision, "microseconds"),
        )
        return f"temporal.durations({arguments})"


def durations(
    min_value: dt.timedelta | None = None,
    max_value: dt.timedelta | None = None,
    *,
    precision: str = "microseconds",
) -> Generator[dt.timedelta]:
    """Generate :class:`datetime.timedelta` objects in
    ``[min_value, max_value]``, inclusive, shrinking toward zero. Either
    bound may be omitted: a missing bound is the end of the range
    :mod:`datetime` supports. One draw in twenty on average targets a
    round duration, such as a second, an hour, a day, 25 hours or a week,
    of either sign.

    ``precision`` names the unit every drawn delta is a whole number of:
    ``"microseconds"`` (the default), ``"milliseconds"``, ``"seconds"``,
    ``"minutes"`` or ``"hours"``.

    Bounds that are not ``timedelta`` objects, ``min_value > max_value``,
    or bounds holding no delta at the precision are an
    :class:`~hegel.InvalidArgument`.
    """
    if min_value is not None:
        _check_type("min_value", min_value, dt.timedelta)
    if max_value is not None:
        _check_type("max_value", max_value, dt.timedelta)
    engine_precision, _ = _precision(precision)
    result = Durations(min_value, max_value, precision)
    low, high = result.bounds
    _check_order(low, high)
    _native.validate_duration(_duration_of(low), _duration_of(high), engine_precision)
    return result


class Offsets(_PrintsValue[dt.timezone]):
    def __init__(
        self, min_value: dt.timedelta | None, max_value: dt.timedelta | None, granularity: str
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.bounds = (
            -_MAX_OFFSET if min_value is None else min_value,
            _MAX_OFFSET if max_value is None else max_value,
        )
        self.granularity = granularity
        self.label = OFFSET_LABEL

    def do_draw(self, tc: TestCase) -> dt.timezone:
        low, high = self.bounds
        seconds = tc._generate_offset(
            low // _SECOND, high // _SECOND, _GRANULARITIES[self.granularity][0]
        )
        return dt.timezone(dt.timedelta(seconds=seconds))

    def __repr__(self) -> str:
        arguments = _arguments(
            min_value=self.min_value,
            max_value=self.max_value,
            granularity=_unless(self.granularity, "seconds"),
        )
        return f"temporal.offsets({arguments})"


def offsets(
    min_value: dt.timedelta | None = None,
    max_value: dt.timedelta | None = None,
    *,
    granularity: str = "seconds",
) -> Generator[dt.timezone]:
    """Generate fixed UTC offsets, as :class:`datetime.timezone` objects,
    with an offset in ``[min_value, max_value]``, shrinking toward
    :attr:`datetime.timezone.utc`.

    Either bound may be omitted. The bounds are whole seconds within the
    day :class:`datetime.timezone` allows, from -23:59:59 to +23:59:59,
    which a missing bound is the end of. ``granularity`` names the unit
    every drawn offset is a whole number of: ``"hours"``,
    ``"quarter_hours"``, ``"minutes"`` or ``"seconds"`` (the default).
    One draw in twenty on average targets an offset some IANA zone has
    used, or a limit such as ±18:00 or ±00:00:01, when it lies in the
    bounds at the granularity.

    Bounds that are not whole-second ``timedelta`` objects within the
    range, ``min_value > max_value``, or bounds holding no offset at the
    granularity are an :class:`~hegel.InvalidArgument`.
    """
    for name, value in (("min_value", min_value), ("max_value", max_value)):
        if value is None:
            continue
        _check_type(name, value, dt.timedelta)
        if value.microseconds:
            raise InvalidArgument(f"{name}={value!r} must be a whole number of seconds")
        if not -_MAX_OFFSET <= value <= _MAX_OFFSET:
            raise InvalidArgument(
                f"{name}={value!r} must be between -23:59:59 and +23:59:59, the offsets "
                "datetime.timezone allows"
            )
    _check_choice("granularity", granularity, _GRANULARITIES)
    result = Offsets(min_value, max_value, granularity)
    low, high = result.bounds
    _check_order(low, high)
    _native.validate_offset(low // _SECOND, high // _SECOND, _GRANULARITIES[granularity][0])
    return result


_SPAN_UNITS: dict[str, int] = {
    "years": _native.SPAN_UNIT_YEARS,
    "months": _native.SPAN_UNIT_MONTHS,
    "weeks": _native.SPAN_UNIT_WEEKS,
    "days": _native.SPAN_UNIT_DAYS,
    "hours": _native.SPAN_UNIT_HOURS,
    "minutes": _native.SPAN_UNIT_MINUTES,
    "seconds": _native.SPAN_UNIT_SECONDS,
    "milliseconds": _native.SPAN_UNIT_MILLISECONDS,
    "microseconds": _native.SPAN_UNIT_MICROSECONDS,
    "nanoseconds": _native.SPAN_UNIT_NANOSECONDS,
}
_SPAN_SIGNS: dict[str, int] = {
    "uniform": _native.SPAN_SIGN_UNIFORM,
    "mixed": _native.SPAN_SIGN_MIXED,
    "non_negative": _native.SPAN_SIGN_NONNEGATIVE,
}


@dataclass(frozen=True)
class Span:
    """A calendar span: a count of each unit from years to nanoseconds,
    such as "1 month and 2 days", which :class:`datetime.timedelta` cannot
    represent because a month or a day has no fixed length. Each count is
    an int in the 64-bit signed range.

    ``str(span)`` is the form every Hegel frontend prints a span in: the
    nonzero units, largest first, with their suffixes, and ``"0s"`` for
    the zero span. A span with no negative unit has no signs
    (``"1y 2mo"``), one with no positive unit has one leading ``-``
    (``"-3d 4h"`` is minus three days and minus four hours), and one with
    both signs each unit (``"-3d +4h"``). The ``repr`` names the nonzero
    units as keyword arguments.
    """

    years: int = 0
    months: int = 0
    weeks: int = 0
    days: int = 0
    hours: int = 0
    minutes: int = 0
    seconds: int = 0
    milliseconds: int = 0
    microseconds: int = 0
    nanoseconds: int = 0

    def __post_init__(self) -> None:
        for unit in _SPAN_UNITS:
            _check_int(unit, getattr(self, unit), _I64_MIN, _I64_MAX)

    def __str__(self) -> str:
        return _native.format_span(astuple(self))

    def __repr__(self) -> str:
        units = ", ".join(
            f"{unit}={getattr(self, unit)!r}" for unit in _SPAN_UNITS if getattr(self, unit)
        )
        return f"Span({units})"


def _span_bounds(unit: str, value: Any) -> tuple[int, int]:
    if (
        not isinstance(value, tuple)
        or len(value) != 2
        or not all(isinstance(v, int) and not isinstance(v, bool) for v in value)
    ):
        raise InvalidArgument(f"{unit}={value!r} must be a (min, max) tuple of ints")
    low, high = value
    if not (_I64_MIN <= low <= _I64_MAX and _I64_MIN <= high <= _I64_MAX):
        raise InvalidArgument(f"{unit}={value!r} must be within the 64-bit signed range")
    return low, high


class Spans(_PrintsValue[Span]):
    def __init__(self, named: dict[str, tuple[int, int]], sign: str) -> None:
        self.named = named
        self.sign = sign
        if named:
            bounds = named
        else:
            low = 0 if sign == "non_negative" else -_I64_MAX
            bounds = dict.fromkeys(_SPAN_UNITS, (low, _I64_MAX))
        self.spec = _native.SpanSpec(
            [(_SPAN_UNITS[unit], low, high) for unit, (low, high) in bounds.items()],
            _SPAN_SIGNS[sign],
        )
        self.label = SPAN_LABEL

    def do_draw(self, tc: TestCase) -> Span:
        return Span(*tc._generate_span(self.spec))

    def __repr__(self) -> str:
        return f"temporal.spans({_arguments(**self.named, sign=_unless(self.sign, 'uniform'))})"


def spans(
    *,
    years: tuple[int, int] | None = None,
    months: tuple[int, int] | None = None,
    weeks: tuple[int, int] | None = None,
    days: tuple[int, int] | None = None,
    hours: tuple[int, int] | None = None,
    minutes: tuple[int, int] | None = None,
    seconds: tuple[int, int] | None = None,
    milliseconds: tuple[int, int] | None = None,
    microseconds: tuple[int, int] | None = None,
    nanoseconds: tuple[int, int] | None = None,
    sign: str = "uniform",
) -> Generator[Span]:
    """Generate calendar spans, as :class:`Span` objects, shrinking toward
    the zero span (or the span nearest it in each unit's bounds).

    Each unit's argument is an inclusive ``(min, max)`` bound on its
    signed value. Naming any unit draws only the named units, leaving the
    others 0, so ``spans(months=(0, 11), days=(0, 30))`` draws months and
    days; naming none draws every unit, each within the 64-bit signed
    range.

    ``sign`` says how the units' signs are drawn: ``"uniform"`` (the
    default) draws one sign for the whole span, as the JavaScript Temporal
    ``Duration`` requires; ``"mixed"`` signs each unit on its own, as
    ``java.time.Period`` allows; ``"non_negative"`` draws only
    magnitudes. Under ``"uniform"`` a negative span draws each unit's
    magnitude within ``[max(-max, 0), -min]``.

    A bound that is not a ``(min, max)`` tuple of ints with ``min <= max``
    within the 64-bit signed range, an unknown ``sign``, a negative
    ``min`` under ``"non_negative"``, or bounds that leave ``"uniform"``
    no sign every unit can take are an :class:`~hegel.InvalidArgument`.
    """
    _check_choice("sign", sign, _SPAN_SIGNS)
    arguments = (
        years,
        months,
        weeks,
        days,
        hours,
        minutes,
        seconds,
        milliseconds,
        microseconds,
        nanoseconds,
    )
    named = {
        unit: _span_bounds(unit, value)
        for unit, value in zip(_SPAN_UNITS, arguments, strict=True)
        if value is not None
    }
    return Spans(named, sign)
