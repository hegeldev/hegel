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
from dataclasses import KW_ONLY, dataclass
from functools import cache
from importlib import resources
from typing import TYPE_CHECKING, Any

from hegel import _native
from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, _PrintsValue
from hegel.generators.combinators import as_generator

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "RuleDate",
    "Span",
    "ZoneData",
    "ZoneOracle",
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
DURATION_LABEL = label_from_name("hegel.timedeltas")
ZONE_NAME_LABEL = label_from_name("hegel.timezone_keys")
ZONE_LABEL = label_from_name("hegel.timezones")
OFFSET_LABEL = label_from_name("hegel.utc_offsets")

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


def _keywords(**values: Any) -> str:
    """The ``, name=value`` text of each argument a generator's repr shows,
    skipping those that are ``None``."""
    return "".join(f", {name}={value!r}" for name, value in values.items() if value is not None)


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
    if mask == 0:
        raise InvalidArgument(f"{name}={values!r} must not be empty")
    return mask


class Dates(_PrintsValue[dt.date]):
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
        keywords = _keywords(
            weekdays=_members(self.weekdays, 0, 7), months=_members(self.months, 1, 12)
        )
        return (
            f"temporal.dates(min_value={self.min_value!r}, max_value={self.max_value!r}{keywords})"
        )


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


class Times(_PrintsValue[dt.time]):
    def __init__(
        self,
        min_value: dt.time,
        max_value: dt.time,
        zones: Generator[Any] | None,
        precision: str,
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.zones = zones
        self.precision = precision
        self.label = TIME_LABEL if zones is None else combine_labels([TIME_LABEL, zones.label])

    def do_draw(self, tc: TestCase) -> dt.time:
        value = tc._generate_time(self.min_value, self.max_value, _PRECISIONS[self.precision][0])
        if self.zones is not None:
            value = value.replace(tzinfo=tc.draw_silent(self.zones))
        return value

    def __repr__(self) -> str:
        keywords = _keywords(
            zones=self.zones,
            precision=None if self.precision == "microseconds" else self.precision,
        )
        return (
            f"temporal.times(min_value={self.min_value!r}, max_value={self.max_value!r}{keywords})"
        )


def _time_microseconds(value: dt.time) -> int:
    return ((value.hour * 60 + value.minute) * 60 + value.second) * 1_000_000 + value.microsecond


def times(
    min_value: dt.time = dt.time.min,
    max_value: dt.time = dt.time.max,
    *,
    zones: Generator[Any] | None = None,
    precision: str = "microseconds",
) -> Generator[dt.time]:
    """Generate :class:`datetime.time` objects in ``[min_value, max_value]``,
    inclusive, shrinking toward midnight.

    The bounds must be naive. The times are naive too, unless
    ``zones`` is given: a generator of :class:`datetime.tzinfo` objects
    (such as :func:`zones`), one of which is attached to each time.
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
    _, unit = _precision(precision)
    _check_fits(
        precision, _time_microseconds(min_value), _time_microseconds(max_value), unit, "time"
    )
    return Times(min_value, max_value, None if zones is None else as_generator(zones), precision)


def tzdb_version() -> str:
    """The IANA release of the time zone database bundled in Hegel's engine,
    such as ``"2026e"``.

    The bundled database is what :meth:`ZoneSource.bundled` draws against,
    and it always supplies the zone names' tags and shrink order. Zoned
    draws use the default zone source, :mod:`zoneinfo`'s database, whose
    release :attr:`ZoneSource.version` reports.
    """
    return _native.tzdb_version()


_MAX_ZONE_OFFSET = dt.timedelta(days=2)
_MAX_RULE_TIME = dt.timedelta(hours=167)


def _whole_seconds(name: str, value: Any, limit: dt.timedelta) -> int:
    _check_type(name, value, dt.timedelta)
    if value % _SECOND or not -limit < value < limit:
        raise InvalidArgument(
            f"{name}={value!r} must be a whole number of seconds strictly within {limit} of zero"
        )
    return value // _SECOND


def _check_int(name: str, value: Any, lowest: int, highest: int) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or not lowest <= value <= highest:
        raise InvalidArgument(f"{name}={value!r} must be an int from {lowest} to {highest}")


@dataclass(frozen=True)
class ZoneType:
    """A local time type of a zone: its UTC offset, whether it is daylight
    saving time, and its abbreviation, such as
    ``ZoneType(dt.timedelta(hours=1), True, "BST")``. The offset is a whole
    number of seconds strictly within two days of zero."""

    offset: dt.timedelta
    is_dst: bool = False
    abbreviation: str = ""

    def __post_init__(self) -> None:
        _whole_seconds("offset", self.offset, _MAX_ZONE_OFFSET)
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
    TZ string name it. Build one with :meth:`julian`, :meth:`zero_based`
    or :meth:`month_week_day`."""

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
        _whole_seconds("time", self.time, _MAX_RULE_TIME)

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


class ZoneSource:
    """The time zone data zoned draws are generated against: which zones
    exist, and their transitions and rules, so that the gaps and folds the
    engine aims at are those of the database the code under test uses.

    By default every draw uses :meth:`zoneinfo`, :mod:`zoneinfo`'s own
    database; :func:`set_default_zone_source` changes that, and
    :func:`zones` and :func:`zone_names` take a ``source`` for one
    generator. :meth:`bundled` is the engine's own database.

    ``ZoneSource()`` is an empty source, for an integration with another
    date and time library to fill: with :meth:`add_tzif`,
    :meth:`add_zone`, :meth:`add_oracle` and :meth:`add_link`, or lazily,
    by declaring the names it knows with :meth:`declare` and supplying each
    zone's data when a draw first needs it through :meth:`set_loader`.
    Whatever its shape, the engine checks the data, and an error in it
    raises :class:`~hegel.InvalidArgument`. A source may be filled while
    it is in use, but a generator reads the names a source knows when it
    is built.
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
        ``None`` for a name it does not know. It is asked once per name,
        for declared names and any other name a generator names, and may be
        called from any thread that draws, but must not draw itself. An
        exception it raises fails the draw with
        :class:`~hegel.InvalidArgument`, whose ``__cause__`` is that
        exception. ``None`` removes the loader."""
        if loader is None:
            self._handle().set_loader(None)
            return
        if not callable(loader):
            raise InvalidArgument(f"loader={loader!r} must be callable")
        self._handle().set_loader(lambda name: _loader_entry(name, loader(name)))

    def __repr__(self) -> str:
        return self._name


_default_source = ZoneSource.zoneinfo()
_native.set_default_zone_source(_default_source._source)


def default_zone_source() -> ZoneSource:
    """The zone source a generator uses when not given one:
    :meth:`ZoneSource.zoneinfo` unless :func:`set_default_zone_source`
    set another."""
    return _default_source


def set_default_zone_source(source: ZoneSource) -> None:
    """Make ``source`` the zone source of every generator built from now on
    without one of its own; generators built earlier keep theirs."""
    global _default_source
    _check_type("source", source, ZoneSource)
    _default_source = source
    _native.set_default_zone_source(source._source)


def _check_source(source: Any) -> ZoneSource:
    if source is None:
        return default_zone_source()
    _check_type("source", source, ZoneSource)
    return source


@cache
def _source_spec(source: ZoneSource, links: bool, legacy: bool) -> _native.TimeZoneSpec:
    handle = source._handle()
    if source._empty:
        raise InvalidArgument(
            "zoneinfo finds no time zones on this system. On a system without a time zone "
            "database (Windows, say) install the tzdata package."
        )
    return _native.TimeZoneSpec(None, links, legacy, False, False, handle)


@cache
def _single_zone_spec(source: ZoneSource, key: str) -> _native.TimeZoneSpec | None:
    handle = source._handle()
    if _native.timezone_info(key, handle) is None:
        return None
    return _native.TimeZoneSpec([key], True, True, False, False, handle)


def _engine_spec(tz: dt.tzinfo) -> _native.TimeZoneSpec | None:
    """The spec drawing only ``tz`` from the default zone source, when that
    source knows it: a :class:`zoneinfo.ZoneInfo` built from a key the
    source has."""
    key = getattr(tz, "key", None)
    if isinstance(tz, zoneinfo.ZoneInfo) and isinstance(key, str):
        return _single_zone_spec(default_zone_source(), key)
    return None


def _check_names(names: Any, source: ZoneSource, *, loadable: bool) -> tuple[str, ...]:
    if isinstance(names, (str, bytes)) or not isinstance(names, Iterable):
        raise InvalidArgument(f"names={names!r} must be an iterable of IANA time zone names")
    checked = tuple(names)
    if not checked:
        raise InvalidArgument("names must not be empty")
    for name in checked:
        if not isinstance(name, str):
            raise InvalidArgument(f"names={names!r} must contain strings, not {name!r}")
        if _native.timezone_info(name, source._handle()) is None:
            raise InvalidArgument(f"{name!r} is not a time zone name {source!r} knows")
        if loadable:
            try:
                zoneinfo.ZoneInfo(name)
            except (zoneinfo.ZoneInfoNotFoundError, ValueError) as error:
                raise InvalidArgument(
                    f"{name!r} is not a time zone this Python can load: {error}"
                ) from None
    return checked


class ZoneNames(_PrintsValue[str]):
    def __init__(
        self,
        names: tuple[str, ...] | None,
        links: bool,
        legacy: bool,
        source: ZoneSource,
        given_source: bool,
    ) -> None:
        self.names = names
        self.links = links
        self.legacy = legacy
        self.source = source
        self.given_source = given_source
        self.spec = (
            _source_spec(source, links, legacy)
            if names is None
            else _native.TimeZoneSpec(list(names), links, legacy, False, False, source._handle())
        )
        self.label = ZONE_NAME_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_timezone(self.spec)

    def arguments(self) -> str:
        names = "" if self.names is None else f"names={list(self.names)!r}, "
        source = f", source={self.source!r}" if self.given_source else ""
        return f"{names}links={self.links!r}, legacy={self.legacy!r}{source}"

    def __repr__(self) -> str:
        return f"temporal.zone_names({self.arguments()})"


def zone_names(
    *,
    names: Iterable[str] | None = None,
    links: bool = True,
    legacy: bool = False,
    source: ZoneSource | None = None,
) -> Generator[str]:
    """Generate IANA time zone names, such as ``"America/New_York"``,
    shrinking toward ``"UTC"``.

    The names are those the zone ``source`` knows, by default
    :func:`default_zone_source`, which is :meth:`ZoneSource.zoneinfo`
    unless changed: the names :func:`zoneinfo.available_timezones` lists,
    so every name generated can be loaded with :class:`zoneinfo.ZoneInfo`.
    On a system without a time zone database (Windows, say) install the
    ``tzdata`` package. The ``posix/`` and ``right/`` variants some
    systems carry are never generated. By default link names such as
    ``"Asia/Calcutta"`` are included and legacy names such as
    ``"EST5EDT"`` and ``"US/Eastern"`` are excluded; ``links`` and
    ``legacy`` switch each.

    ``names`` restricts the draw to those names, each of which the source
    must know; ``links`` and ``legacy`` do not apply to an explicit list.

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
    """
    _check_type("links", links, bool)
    _check_type("legacy", legacy, bool)
    checked_source = _check_source(source)
    checked = None if names is None else _check_names(names, checked_source, loadable=False)
    return ZoneNames(checked, links, legacy, checked_source, source is not None)


class Zones(_PrintsValue[zoneinfo.ZoneInfo]):
    def __init__(self, keys: ZoneNames, no_cache: bool) -> None:
        self.keys = keys
        self.no_cache = no_cache
        self.label = ZONE_LABEL

    def resolve(self, key: str) -> zoneinfo.ZoneInfo:
        try:
            return zoneinfo.ZoneInfo.no_cache(key) if self.no_cache else zoneinfo.ZoneInfo(key)
        except (zoneinfo.ZoneInfoNotFoundError, ValueError):
            raise InvalidArgument(
                f"zones() drew {key!r} from {self.keys.source!r}, but this Python's zoneinfo "
                "cannot load it; draw names that are not zoneinfo's with zone_names()"
            ) from None

    def do_draw(self, tc: TestCase) -> zoneinfo.ZoneInfo:
        return self.resolve(tc._generate_timezone(self.keys.spec))

    def __repr__(self) -> str:
        return f"temporal.zones({self.keys.arguments()}, no_cache={self.no_cache!r})"


def zones(
    *,
    names: Iterable[str] | None = None,
    no_cache: bool = False,
    links: bool = True,
    legacy: bool = False,
    source: ZoneSource | None = None,
) -> Generator[zoneinfo.ZoneInfo]:
    """Generate IANA time zones, as :class:`zoneinfo.ZoneInfo` objects,
    shrinking toward UTC.

    The zones are those :func:`zone_names` names, with the same
    ``names``, ``links``, ``legacy`` and ``source`` arguments; a name in
    ``names`` must also be one this Python can load. With ``no_cache=True``
    each zone is built with :meth:`zoneinfo.ZoneInfo.no_cache`, so it is
    not the same object as ``ZoneInfo(key)``.

    :func:`datetimes` and :func:`zoned_datetimes` draw their wall times
    against the source's data for the zone, so a source other than
    :meth:`ZoneSource.zoneinfo` aims at gaps and folds that the
    :class:`~zoneinfo.ZoneInfo` attached to the value may not have. A name
    the source draws that :mod:`zoneinfo` cannot load is an
    :class:`~hegel.InvalidArgument`.
    """
    _check_type("no_cache", no_cache, bool)
    _check_type("links", links, bool)
    _check_type("legacy", legacy, bool)
    checked_source = _check_source(source)
    checked = None if names is None else _check_names(names, checked_source, loadable=True)
    return Zones(ZoneNames(checked, links, legacy, checked_source, source is not None), no_cache)


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


class Datetimes(_PrintsValue[dt.datetime]):
    def __init__(
        self,
        min_value: dt.datetime,
        max_value: dt.datetime,
        zones: Generator[Any] | None,
        allow_imaginary: bool,
        ambiguous: str,
        precision: str,
        name: str,
    ) -> None:
        self.name = name
        self.min_value = min_value
        self.max_value = max_value
        self.zones = zones
        self.allow_imaginary = allow_imaginary
        self.ambiguous = ambiguous
        self.precision = precision
        self.engine_precision = _PRECISIONS[precision][0]
        self.aware = min_value.tzinfo is not None
        self.require_zone = self.aware or name == "zoned_datetimes"
        if self.aware:
            self.zoned_spec = _native.ZonedSpec(
                None,
                (_timestamp(min_value), _timestamp(max_value)),
                self.engine_precision,
                allow_imaginary,
                _AMBIGUOUS[ambiguous],
            )
        else:
            self.zoned_spec = _native.ZonedSpec(
                (_components(min_value), _components(max_value)),
                None,
                self.engine_precision,
                allow_imaginary,
                _AMBIGUOUS[ambiguous],
            )
        own_label = ZONED_DATETIME_LABEL if name == "zoned_datetimes" else DATETIME_LABEL
        self.label = own_label if zones is None else combine_labels([own_label, zones.label])

    def do_draw(self, tc: TestCase) -> dt.datetime:
        if isinstance(self.zones, Zones):
            key, value = tc._generate_zoned_datetime(self.zones.keys.spec, self.zoned_spec)
            return value.replace(tzinfo=self.zones.resolve(key))
        tz = None if self.zones is None else tc.draw_silent(self.zones)
        if tz is None:
            if self.require_zone:
                reason = "with aware bounds" if self.aware else f"from {self.name}()"
                raise InvalidArgument(
                    f"zones={self.zones!r} generated None, but {reason} every datetime "
                    "needs a tzinfo"
                )
            return tc._generate_datetime(self.min_value, self.max_value, self.engine_precision)
        if not isinstance(tz, dt.tzinfo):
            raise InvalidArgument(f"zones must generate tzinfo objects, got {tz!r}")
        spec = _engine_spec(tz)
        if spec is not None:
            return tc._generate_zoned_datetime(spec, self.zoned_spec)[1].replace(tzinfo=tz)
        if self.aware:
            lo, hi = _local_wall(self.min_value, tz), _local_wall(self.max_value, tz)
            if lo > hi:
                tc.reject()
        else:
            lo, hi = self.min_value, self.max_value
        value = tc._generate_datetime(lo, hi, self.engine_precision).replace(tzinfo=tz)
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
        keywords = _keywords(
            zones=self.zones,
            allow_imaginary=None if self.allow_imaginary else False,
            ambiguous=None if self.ambiguous == "both" else self.ambiguous,
            precision=None if self.precision == "microseconds" else self.precision,
        )
        bounds = f"min_value={self.min_value!r}, max_value={self.max_value!r}"
        return f"temporal.{self.name}({bounds}{keywords})"


def _default_zones() -> Zones:
    return Zones(ZoneNames(None, True, False, default_zone_source(), False), False)


def _datetimes(
    name: str,
    min_value: dt.datetime | None,
    max_value: dt.datetime | None,
    zones: Generator[Any] | None,
    allow_imaginary: bool,
    ambiguous: str,
    precision: str,
) -> Datetimes:
    _check_type("allow_imaginary", allow_imaginary, bool)
    _check_choice("ambiguous", ambiguous, _AMBIGUOUS)
    _, unit = _precision(precision)
    bounds = (("min_value", min_value), ("max_value", max_value))
    for bound, value in bounds:
        if value is not None:
            _check_type(bound, value, dt.datetime)
    aware = any(value is not None and value.tzinfo is not None for _, value in bounds)
    if aware:
        for bound, value in bounds:
            if value is not None and value.tzinfo is None:
                raise InvalidArgument(
                    f"{bound}={value!r} is naive, but the other bound is timezone-aware; "
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
    source: Generator[Any] | None = None
    if zones is not None:
        source = as_generator(zones)
    elif aware or name == "zoned_datetimes":
        source = _default_zones()
    return Datetimes(min_value, max_value, source, allow_imaginary, ambiguous, precision, name)


def datetimes(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    zones: Generator[Any] | None = None,
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

    **Transitions.** A zone from :func:`zones`, or a
    :class:`zoneinfo.ZoneInfo` from any other generator, is drawn in by
    the engine, which knows its daylight-saving and offset transitions
    from the zone source's data (that of the ``source`` given to
    :func:`zones`, or for a zone from another generator that of
    :func:`default_zone_source`, by default :mod:`zoneinfo`'s own):
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
    precision, or a ``zones`` generator producing something other
    than ``tzinfo`` objects (or ``None`` with aware bounds) are an
    :class:`~hegel.InvalidArgument`.
    """
    return _datetimes(
        "datetimes", min_value, max_value, zones, allow_imaginary, ambiguous, precision
    )


def zoned_datetimes(
    min_value: dt.datetime | None = None,
    max_value: dt.datetime | None = None,
    *,
    zones: Generator[Any] | None = None,
    allow_imaginary: bool = True,
    ambiguous: str = "both",
    precision: str = "microseconds",
) -> Generator[dt.datetime]:
    """Generate aware :class:`datetime.datetime` objects in
    ``[min_value, max_value]``, each in a zone drawn from ``zones``,
    shrinking toward 2000-01-01 00:00 (or the bound nearest it) in UTC.

    This is :func:`datetimes` for a test that wants only aware datetimes,
    and takes the same arguments: ``zones`` defaults to :func:`zones` and
    must not generate ``None``, naive bounds (or none) limit the
    wall-clock time in the drawn zone, and aware bounds are moments in
    time. The engine draws the zone and the wall time together, so one
    draw in twenty on average lands near one of the zone's transitions::

        meeting = tc.draw(gs.temporal.zoned_datetimes(dt.datetime(2026, 1, 1)))

    Invalid arguments are those of :func:`datetimes`.
    """
    return _datetimes(
        "zoned_datetimes", min_value, max_value, zones, allow_imaginary, ambiguous, precision
    )


_MIN_INSTANT = dt.datetime.min.replace(tzinfo=dt.UTC)
_MAX_INSTANT = dt.datetime.max.replace(tzinfo=dt.UTC)


class Instants(_PrintsValue[dt.datetime]):
    def __init__(self, min_value: dt.datetime, max_value: dt.datetime, precision: str) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.precision = precision
        self.bounds = (
            _timestamp(max(min_value, _MIN_INSTANT)),
            _timestamp(min(max_value, _MAX_INSTANT)),
        )
        self.label = INSTANT_LABEL

    def do_draw(self, tc: TestCase) -> dt.datetime:
        seconds, nanoseconds = tc._generate_instant(*self.bounds, _PRECISIONS[self.precision][0])
        return _EPOCH + dt.timedelta(seconds=seconds, microseconds=nanoseconds // 1000)

    def __repr__(self) -> str:
        keywords = _keywords(precision=None if self.precision == "microseconds" else self.precision)
        bounds = f"min_value={self.min_value!r}, max_value={self.max_value!r}"
        return f"temporal.instants({bounds}{keywords})"


def instants(
    min_value: dt.datetime = _MIN_INSTANT,
    max_value: dt.datetime = _MAX_INSTANT,
    *,
    precision: str = "microseconds",
) -> Generator[dt.datetime]:
    """Generate moments in time, as :class:`datetime.datetime` objects in
    UTC (``tzinfo`` is :data:`datetime.UTC`), in ``[min_value, max_value]``,
    shrinking toward 2000-01-01 00:00 UTC (or the bound nearest it).

    The bounds must be aware, and are compared as moments in time however
    their time zones differ; they default to the whole range a UTC
    :class:`~datetime.datetime` represents, and a bound beyond it is
    clamped to it. One draw in twenty on average targets an instant tests
    tend to get wrong: the Unix epoch and its neighbours, the 32-bit and
    64-bit second boundaries, leap days, or a transition of any time zone
    in Hegel's database.

    ``precision`` names the unit every drawn instant is a whole number of
    since the Unix epoch: ``"microseconds"`` (the default),
    ``"milliseconds"``, ``"seconds"``, ``"minutes"`` or ``"hours"``.

    Bounds that are not aware datetimes, ``min_value > max_value``, or
    bounds holding no instant at the precision in the UTC range are an
    :class:`~hegel.InvalidArgument`.
    """
    _check_type("min_value", min_value, dt.datetime)
    _check_type("max_value", max_value, dt.datetime)
    for name, value in (("min_value", min_value), ("max_value", max_value)):
        if value.tzinfo is None:
            raise InvalidArgument(f"{name}={value!r} must be aware: instants() bounds are moments")
    if _instant(max_value) < _instant(min_value):
        raise InvalidArgument(
            f"Cannot have max_value={max_value!r} < min_value={min_value!r}, "
            "comparing as moments in time"
        )
    _, unit = _precision(precision)
    _check_fits(
        precision,
        _instant(max(min_value, _MIN_INSTANT)) // _MICROSECOND,
        _instant(min(max_value, _MAX_INSTANT)) // _MICROSECOND,
        unit,
        "instant",
    )
    return Instants(min_value, max_value, precision)


class Durations(_PrintsValue[dt.timedelta]):
    def __init__(self, min_value: dt.timedelta, max_value: dt.timedelta, precision: str) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.precision = precision
        self.label = DURATION_LABEL

    def do_draw(self, tc: TestCase) -> dt.timedelta:
        return tc._generate_timedelta(
            self.min_value, self.max_value, _PRECISIONS[self.precision][0]
        )

    def __repr__(self) -> str:
        keywords = _keywords(precision=None if self.precision == "microseconds" else self.precision)
        bounds = f"min_value={self.min_value!r}, max_value={self.max_value!r}"
        return f"temporal.durations({bounds}{keywords})"


def durations(
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
    _, unit = _precision(precision)
    _check_fits(precision, min_value // _MICROSECOND, max_value // _MICROSECOND, unit, "timedelta")
    return Durations(min_value, max_value, precision)


class Offsets(_PrintsValue[dt.timezone]):
    def __init__(self, min_value: dt.timedelta, max_value: dt.timedelta, granularity: str) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.granularity = granularity
        self.label = OFFSET_LABEL

    def do_draw(self, tc: TestCase) -> dt.timezone:
        seconds = tc._generate_offset(
            self.min_value // _SECOND,
            self.max_value // _SECOND,
            _GRANULARITIES[self.granularity][0],
        )
        return dt.timezone(dt.timedelta(seconds=seconds))

    def __repr__(self) -> str:
        return (
            f"temporal.offsets(min_value={self.min_value!r}, max_value={self.max_value!r}, "
            f"granularity={self.granularity!r})"
        )


def offsets(
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
    return Offsets(min_value, max_value, granularity)


_SPAN_UNITS = (
    "years",
    "months",
    "weeks",
    "days",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
    "microseconds",
    "nanoseconds",
)
_SPAN_SUFFIXES = ("y", "mo", "w", "d", "h", "m", "s", "ms", "µs", "ns")
_SPAN_SIGNS: dict[str, int] = {
    "uniform": _native.SPAN_SIGN_UNIFORM,
    "mixed": _native.SPAN_SIGN_MIXED,
    "non_negative": _native.SPAN_SIGN_NONNEGATIVE,
}
_I64_MIN = -(2**63)
_I64_MAX = 2**63 - 1


@dataclass(frozen=True)
class Span:
    """A calendar span: a count of each unit from years to nanoseconds,
    such as "1 month and 2 days", which :class:`datetime.timedelta` cannot
    represent because a month or a day has no fixed length.

    ``str(span)`` is the form every Hegel frontend prints a span in, the
    nonzero units with their suffixes (``"1y 2mo"``, ``"-3d 4h"`` when
    every unit is negative or zero, ``"0s"`` for the zero span); the
    ``repr`` names the nonzero units as keyword arguments.
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

    def __str__(self) -> str:
        values = [getattr(self, unit) for unit in _SPAN_UNITS]
        negative = any(value < 0 for value in values) and all(value <= 0 for value in values)
        text = " ".join(
            f"{abs(value) if negative else value}{suffix}"
            for value, suffix in zip(values, _SPAN_SUFFIXES, strict=True)
            if value
        )
        if negative:
            return f"-{text}"
        return text or "0s"

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
    if not _I64_MIN <= low <= high <= _I64_MAX:
        raise InvalidArgument(
            f"{unit}={value!r} must have min <= max, both within the 64-bit signed range"
        )
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
            [(index, *bounds[unit]) for index, unit in enumerate(_SPAN_UNITS) if unit in bounds],
            _SPAN_SIGNS[sign],
        )
        self.label = SPAN_LABEL

    def do_draw(self, tc: TestCase) -> Span:
        return Span(*tc._generate_span(self.spec))

    def __repr__(self) -> str:
        arguments = [f"{unit}={value!r}" for unit, value in self.named.items()]
        if self.sign != "uniform":
            arguments.append(f"sign={self.sign!r}")
        return f"temporal.spans({', '.join(arguments)})"


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
    if sign == "non_negative":
        for unit, (low, _) in named.items():
            if low < 0:
                raise InvalidArgument(
                    f"{unit}={named[unit]!r} has a negative min, which sign='non_negative' "
                    "does not allow"
                )
    if sign == "uniform" and not (
        all(high >= 0 for _, high in named.values()) or all(low <= 0 for low, _ in named.values())
    ):
        raise InvalidArgument(
            f"no sign is one every unit of {_keywords(**named)[2:]} can take, which "
            "sign='uniform' needs; pass sign='mixed' to sign each unit on its own"
        )
    return Spans(named, sign)
