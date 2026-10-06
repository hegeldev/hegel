"""Date and time generators."""

from __future__ import annotations

import datetime as dt
from typing import TYPE_CHECKING, Any

from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, combine_labels, label_from_name
from hegel.generators.combinators import as_generator, just
from hegel.generators.numeric import integers

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["dates", "datetimes", "timedeltas", "times", "timezones"]

DATE_LABEL = label_from_name("hegel.dates")
TIME_LABEL = label_from_name("hegel.times")
DATETIME_LABEL = label_from_name("hegel.datetimes")
TIMEDELTA_LABEL = label_from_name("hegel.timedeltas")
TIMEZONE_LABEL = label_from_name("hegel.timezones")


def _check_type(name: str, value: Any, cls: type) -> None:
    if not isinstance(value, cls):
        raise InvalidArgument(f"{name}={value!r} must be a {cls.__name__}")


class Dates(Generator[dt.date]):
    def __init__(self, min_value: dt.date, max_value: dt.date) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.label = DATE_LABEL

    def do_draw(self, tc: TestCase) -> dt.date:
        return tc.generate_date(self.min_value, self.max_value)

    def __repr__(self) -> str:
        return f"dates(min_value={self.min_value!r}, max_value={self.max_value!r})"


def dates(min_value: dt.date = dt.date.min, max_value: dt.date = dt.date.max) -> Generator[dt.date]:
    """Generate dates in ``[min_value, max_value]``."""
    _check_type("min_value", min_value, dt.date)
    _check_type("max_value", max_value, dt.date)
    if isinstance(min_value, dt.datetime) or isinstance(max_value, dt.datetime):
        raise InvalidArgument("dates() bounds must be date objects, not datetimes")
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    return Dates(min_value, max_value)


class Times(Generator[dt.time]):
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
        value = tc.generate_time(self.min_value, self.max_value)
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
    """Generate times of day in ``[min_value, max_value]``, naive unless
    ``timezones`` generates tzinfo objects to attach."""
    _check_type("min_value", min_value, dt.time)
    _check_type("max_value", max_value, dt.time)
    if min_value.tzinfo is not None or max_value.tzinfo is not None:
        raise InvalidArgument("times() bounds must be naive")
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    return Times(min_value, max_value, None if timezones is None else as_generator(timezones))


class Datetimes(Generator[dt.datetime]):
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
        self.label = (
            DATETIME_LABEL
            if timezones is None
            else combine_labels([DATETIME_LABEL, timezones.label])
        )

    def do_draw(self, tc: TestCase) -> dt.datetime:
        value = tc.generate_datetime(self.min_value, self.max_value)
        if self.timezones is None:
            return value
        tz = tc.draw_silent(self.timezones)
        if tz is not None and not isinstance(tz, dt.tzinfo):
            raise InvalidArgument(f"timezones must generate tzinfo objects, got {tz!r}")
        result = value.replace(tzinfo=tz)
        if tz is not None and not self.allow_imaginary and _is_imaginary(result):
            tc.reject()
        return result

    def __repr__(self) -> str:
        return f"datetimes(min_value={self.min_value!r}, max_value={self.max_value!r})"


def _is_imaginary(value: dt.datetime) -> bool:
    try:
        return value.astimezone(dt.UTC).astimezone(value.tzinfo).replace(tzinfo=None) != (
            value.replace(tzinfo=None)
        )
    except (OverflowError, ValueError):  # nocov
        return False


def datetimes(
    min_value: dt.datetime = dt.datetime.min,
    max_value: dt.datetime = dt.datetime.max,
    *,
    timezones: Generator[Any] | None = None,
    allow_imaginary: bool = True,
) -> Generator[dt.datetime]:
    """Generate datetimes in ``[min_value, max_value]`` (naive bounds), with
    tzinfo drawn from ``timezones`` when given."""
    _check_type("min_value", min_value, dt.datetime)
    _check_type("max_value", max_value, dt.datetime)
    if min_value.tzinfo is not None or max_value.tzinfo is not None:
        raise InvalidArgument("datetimes() bounds must be naive")
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    return Datetimes(
        min_value,
        max_value,
        None if timezones is None else as_generator(timezones),
        bool(allow_imaginary),
    )


def _to_micros(delta: dt.timedelta) -> int:
    return (delta.days * 86400 + delta.seconds) * 1_000_000 + delta.microseconds


class Timedeltas(Generator[dt.timedelta]):
    def __init__(self, min_value: dt.timedelta, max_value: dt.timedelta) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self._micros = integers(_to_micros(min_value), _to_micros(max_value))
        self.label = TIMEDELTA_LABEL

    def do_draw(self, tc: TestCase) -> dt.timedelta:
        return dt.timedelta(microseconds=tc.draw_silent(self._micros))

    def __repr__(self) -> str:
        return f"timedeltas(min_value={self.min_value!r}, max_value={self.max_value!r})"


def timedeltas(
    min_value: dt.timedelta = dt.timedelta.min, max_value: dt.timedelta = dt.timedelta.max
) -> Generator[dt.timedelta]:
    """Generate timedeltas in ``[min_value, max_value]``."""
    _check_type("min_value", min_value, dt.timedelta)
    _check_type("max_value", max_value, dt.timedelta)
    if min_value > max_value:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    return Timedeltas(min_value, max_value)


def timezones(*, no_cache: bool = False) -> Generator[dt.tzinfo]:
    """Generate IANA timezones (``zoneinfo.ZoneInfo`` objects), shrinking
    toward UTC."""
    import zoneinfo

    from hegel.generators.combinators import sampled_from

    try:
        keys = sorted(zoneinfo.available_timezones())
    except Exception:  # nocov
        keys = []
    if not keys:
        return just(dt.UTC)  # nocov
    keys = ["UTC", *(key for key in keys if key != "UTC")]
    factory = zoneinfo.ZoneInfo.no_cache if no_cache else zoneinfo.ZoneInfo
    return sampled_from(keys).map(factory)
