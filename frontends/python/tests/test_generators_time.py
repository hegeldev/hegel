from __future__ import annotations

import datetime as dt
import io
import re
import zoneinfo
from importlib.resources import files

import pytest

import hegel
from hegel import _native
from hegel import generators as gs
from hegel.generators.time import _is_imaginary, _local_wall, _select_keys
from tests.helpers import failing, find_example, run_test

UTC = dt.UTC
NEW_YORK = zoneinfo.ZoneInfo("America/New_York")
LONDON = zoneinfo.ZoneInfo("Europe/London")


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


EPOCH = dt.datetime(1970, 1, 1, tzinfo=UTC)


def instant(value):
    return value - EPOCH


def test_dates():
    check_all(gs.dates(), lambda d: isinstance(d, dt.date) and not isinstance(d, dt.datetime))
    lo, hi = dt.date(2020, 2, 28), dt.date(2020, 3, 1)
    check_all(gs.dates(lo, hi), lambda d: lo <= d <= hi)
    assert find_example(gs.dates()) == dt.date(2000, 1, 1)
    assert find_example(gs.dates(min_value=dt.date(2021, 5, 5))) == dt.date(2021, 5, 5)
    assert repr(gs.dates(lo, hi)) == f"dates(min_value={lo!r}, max_value={hi!r})"
    with pytest.raises(hegel.InvalidArgument):
        gs.dates(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.dates("2020-01-01")
    with pytest.raises(hegel.InvalidArgument):
        gs.dates(dt.datetime(2020, 1, 1))


def test_times():
    check_all(gs.times(), lambda t: isinstance(t, dt.time) and t.tzinfo is None)
    lo, hi = dt.time(1, 2, 3, 4), dt.time(1, 2, 3, 9)
    check_all(gs.times(lo, hi), lambda t: lo <= t <= hi)
    check_all(gs.times(timezones=gs.just(UTC)), lambda t: t.tzinfo is UTC)
    assert find_example(gs.times()) == dt.time(0, 0)
    assert find_example(gs.times(), lambda t: t > dt.time(0, 0)) == dt.time(0, 0, 0, 1)
    assert repr(gs.times(lo, hi)) == f"times(min_value={lo!r}, max_value={hi!r})"
    with pytest.raises(hegel.InvalidArgument):
        gs.times(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.times(dt.time(1, tzinfo=UTC))
    with pytest.raises(hegel.InvalidArgument):
        gs.times(5)
    with pytest.raises(hegel.InvalidArgument):
        gs.times(timezones=5)


def test_datetimes():
    check_all(gs.datetimes(), lambda d: isinstance(d, dt.datetime) and d.tzinfo is None)
    lo, hi = dt.datetime(2020, 1, 1, 1, 2, 3, 4), dt.datetime(2020, 1, 1, 1, 2, 3, 9)
    check_all(gs.datetimes(lo, hi), lambda d: lo <= d <= hi)
    check_all(
        gs.datetimes(timezones=gs.timezones()), lambda d: isinstance(d.tzinfo, zoneinfo.ZoneInfo)
    )
    check_all(gs.datetimes(timezones=gs.none()), lambda d: d.tzinfo is None)
    assert find_example(gs.datetimes()) == dt.datetime(2000, 1, 1)
    assert find_example(gs.datetimes(timezones=gs.timezones())) == dt.datetime(
        2000, 1, 1, tzinfo=zoneinfo.ZoneInfo("UTC")
    )
    assert repr(gs.datetimes(lo, hi)) == f"datetimes(min_value={lo!r}, max_value={hi!r})"
    with pytest.raises(hegel.InvalidArgument):
        gs.datetimes(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.datetimes(dt.date(2020, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="allow_imaginary=0 must be a bool"):
        gs.datetimes(allow_imaginary=0)

    def bad_tz(tc):
        tc.draw(gs.datetimes(timezones=gs.just("UTC")))

    failing(bad_tz, hegel.InvalidArgument)


def test_datetimes_in_a_zone_from_any_generator():
    zones = gs.sampled_from([NEW_YORK, LONDON])
    check_all(gs.datetimes(timezones=zones), lambda d: d.tzinfo in (NEW_YORK, LONDON))
    fixed = dt.timezone(dt.timedelta(hours=5, minutes=45))
    lo, hi = dt.datetime(2020, 1, 1), dt.datetime(2020, 1, 2)
    check_all(
        gs.datetimes(lo, hi, timezones=gs.just(fixed)),
        lambda d: d.tzinfo is fixed and lo <= d.replace(tzinfo=None) <= hi and d.fold == 0,
    )


def test_datetimes_draw_both_folds():
    # America/New_York fell back from 02:00 EDT to 01:00 EST on 2020-11-01,
    # so wall times between 01:00 and 02:00 are repeated.
    lo, hi = dt.datetime(2020, 11, 1, 1, 0), dt.datetime(2020, 11, 1, 1, 59)
    repeated = gs.datetimes(lo, hi, timezones=gs.just(NEW_YORK))
    for fold in (0, 1):
        value = find_example(repeated, lambda d: d.fold == fold)
        assert value.fold == fold
        assert value.utcoffset() == dt.timedelta(hours=-4 if fold == 0 else -5)
    assert find_example(repeated) == lo.replace(tzinfo=NEW_YORK)


def test_datetimes_draw_gaps_unless_told_not_to():
    # Europe/London sprang forward from 01:00 GMT to 02:00 BST on 2020-03-29.
    lo, hi = dt.datetime(2020, 3, 29, 0, 30), dt.datetime(2020, 3, 29, 2, 30)
    imaginary = gs.datetimes(lo, hi, timezones=gs.just(LONDON))
    skipped = find_example(imaginary, _is_imaginary)
    assert (
        dt.datetime(2020, 3, 29, 1, 0)
        <= skipped.replace(tzinfo=None)
        < dt.datetime(2020, 3, 29, 2, 0)
    )
    real = gs.datetimes(lo, hi, timezones=gs.just(LONDON), allow_imaginary=False)
    check_all(real, lambda d: not _is_imaginary(d), test_cases=200)
    only_gap = gs.datetimes(
        dt.datetime(2020, 3, 29, 1, 0),
        dt.datetime(2020, 3, 29, 1, 59),
        timezones=gs.just(LONDON),
        allow_imaginary=False,
    )

    def body(tc):
        tc.draw(only_gap)

    failing(body, hegel.FailedHealthCheck, test_cases=5)


def test_datetimes_agree_with_zoneinfo_on_the_offset():
    zones = gs.sampled_from(
        [NEW_YORK, LONDON, zoneinfo.ZoneInfo("Europe/Dublin"), zoneinfo.ZoneInfo("Pacific/Apia")]
    )

    def body(tc):
        value = tc.draw(
            gs.datetimes(dt.datetime(1990, 1, 1), dt.datetime(2030, 1, 1), timezones=zones)
        )
        kind, before, after, offset, known, _ = _native.timezone_resolve(
            value.tzinfo.key, (*value.timetuple()[:6], value.microsecond * 1000)
        )
        assert known
        if kind == _native.TIMEZONE_KIND_UNIQUE:
            assert value.fold == 0
            expected = offset
        elif kind == _native.TIMEZONE_KIND_FOLD:
            expected = after if value.fold else before
        else:
            assert value.fold == 0
            expected = before
        assert value.utcoffset() == dt.timedelta(seconds=expected), (value, kind)

    run_test(body, test_cases=300)


def test_datetimes_with_aware_bounds():
    lo = dt.datetime(2020, 11, 1, 5, 30, tzinfo=UTC)
    hi = dt.datetime(2020, 11, 1, 3, 0, tzinfo=NEW_YORK)
    assert instant(lo) < instant(hi)
    check_all(
        gs.datetimes(lo, hi),
        lambda d: (
            isinstance(d.tzinfo, zoneinfo.ZoneInfo) and instant(lo) <= instant(d) <= instant(hi)
        ),
        test_cases=200,
    )
    in_new_york = gs.datetimes(lo, hi, timezones=gs.just(NEW_YORK))
    check_all(
        in_new_york, lambda d: d.tzinfo is NEW_YORK and instant(lo) <= instant(d) <= instant(hi)
    )
    for fold in (0, 1):
        assert find_example(in_new_york, lambda d: d.fold == fold).fold == fold
    fixed = dt.timezone(dt.timedelta(hours=-3))
    check_all(
        gs.datetimes(lo, hi, timezones=gs.just(fixed)),
        lambda d: d.tzinfo is fixed and instant(lo) <= instant(d) <= instant(hi),
    )
    check_all(
        gs.datetimes(min_value=dt.datetime(2020, 1, 1, tzinfo=UTC)),
        lambda d: instant(d) >= instant(dt.datetime(2020, 1, 1, tzinfo=UTC)),
    )
    check_all(
        gs.datetimes(max_value=dt.datetime(2020, 1, 1, tzinfo=UTC)),
        lambda d: instant(d) <= instant(dt.datetime(2020, 1, 1, tzinfo=UTC)),
    )
    with pytest.raises(hegel.InvalidArgument, match="both naive or both aware"):
        gs.datetimes(lo, dt.datetime(2021, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="both naive or both aware"):
        gs.datetimes(dt.datetime(2019, 1, 1), hi)
    with pytest.raises(hegel.InvalidArgument, match="moments in time"):
        gs.datetimes(hi, lo)
    # fold=1 is the later moment, so these bounds are ordered by wall clock
    # time but not as instants.
    early = dt.datetime(2020, 11, 1, 1, 45, tzinfo=NEW_YORK, fold=1)
    late = dt.datetime(2020, 11, 1, 1, 50, tzinfo=NEW_YORK)
    with pytest.raises(hegel.InvalidArgument, match="moments in time"):
        gs.datetimes(early, late)

    def naive_zone(tc):
        tc.draw(gs.datetimes(lo, hi, timezones=gs.none()))

    assert "every datetime needs a tzinfo" in str(failing(naive_zone, hegel.InvalidArgument))


class Shifted(dt.tzinfo):
    """A tzinfo the engine knows nothing about."""

    def utcoffset(self, value):
        return dt.timedelta(hours=1)

    def dst(self, value):
        return dt.timedelta(0)

    def tzname(self, value):
        return "SHIFT"


def test_datetimes_in_zones_the_engine_cannot_see():
    tzif = (files("tzdata.zoneinfo") / "UTC").read_bytes()
    foreign = zoneinfo.ZoneInfo.from_file(io.BytesIO(tzif), key="Mars/Olympus_Mons")
    anonymous = zoneinfo.ZoneInfo.from_file(io.BytesIO(tzif))
    assert anonymous.key is None
    shifted = Shifted()
    zones = gs.sampled_from([foreign, anonymous, shifted])
    lo, hi = dt.datetime(2020, 1, 1), dt.datetime(2020, 1, 2)
    check_all(
        gs.datetimes(lo, hi, timezones=zones),
        lambda d: d.tzinfo in (foreign, anonymous, shifted) and lo <= d.replace(tzinfo=None) <= hi,
    )
    check_all(
        gs.datetimes(lo, hi, timezones=zones, allow_imaginary=False),
        lambda d: not _is_imaginary(d),
    )
    alo, ahi = lo.replace(tzinfo=UTC), hi.replace(tzinfo=UTC)
    check_all(
        gs.datetimes(alo, ahi, timezones=gs.just(shifted)),
        lambda d: d.tzinfo is shifted and instant(alo) <= instant(d) <= instant(ahi),
    )
    # The engine cannot see this zone's gap, so an imaginary wall time is
    # detected after the draw and rejects the test case.
    london_tzif = (files("tzdata.zoneinfo") / "Europe" / "London").read_bytes()
    foreign_london = zoneinfo.ZoneInfo.from_file(io.BytesIO(london_tzif), key="Mars/London")
    gap_lo, gap_hi = dt.datetime(2020, 3, 29, 1, 0), dt.datetime(2020, 3, 29, 1, 59)
    check_all(gs.datetimes(gap_lo, gap_hi, timezones=gs.just(foreign_london)), _is_imaginary)

    def body(tc):
        tc.draw(
            gs.datetimes(gap_lo, gap_hi, timezones=gs.just(foreign_london), allow_imaginary=False)
        )

    failing(body, hegel.FailedHealthCheck, test_cases=5)


def test_datetimes_near_the_limits_of_the_type():
    # In a zone fourteen hours ahead of UTC the last instants of 9999 fall in
    # the year 10000, which datetime cannot hold.
    far_east = zoneinfo.ZoneInfo("Etc/GMT-14")
    last = dt.datetime.max.replace(tzinfo=UTC)

    def body(tc):
        tc.draw(gs.datetimes(last, last, timezones=gs.just(far_east)))

    failing(body, hegel.FailedHealthCheck, test_cases=5)

    def fixed_body(tc):
        tc.draw(gs.datetimes(last, last, timezones=gs.just(dt.timezone(dt.timedelta(hours=14)))))

    failing(fixed_body, hegel.FailedHealthCheck, test_cases=5)
    # Instants that no wall time of a zone the engine cannot see represents
    # reject every test case too.
    behind_lo = dt.datetime.max.replace(tzinfo=dt.timezone(dt.timedelta(hours=-4)))
    behind_hi = dt.datetime.max.replace(tzinfo=dt.timezone(dt.timedelta(hours=-5)))
    assert instant(behind_lo) < instant(behind_hi)
    west = dt.timezone(dt.timedelta(hours=-1))

    def behind_body(tc):
        tc.draw(gs.datetimes(behind_lo, behind_hi, timezones=gs.just(west)))

    failing(behind_body, hegel.FailedHealthCheck, test_cases=5)
    ahead_lo = dt.datetime.min.replace(tzinfo=dt.timezone(dt.timedelta(hours=5)))
    ahead_hi = dt.datetime.min.replace(tzinfo=dt.timezone(dt.timedelta(hours=4)))
    shifted = Shifted()

    def ahead_body(tc):
        tc.draw(gs.datetimes(ahead_lo, ahead_hi, timezones=gs.just(shifted)))

    failing(ahead_body, hegel.FailedHealthCheck, test_cases=5)
    # A bound beyond the wall times the zone represents is clamped to them.
    check_all(
        gs.datetimes(behind_lo.replace(day=30), behind_hi, timezones=gs.just(west)),
        lambda d: d.tzinfo is west and d.replace(tzinfo=None) >= dt.datetime(9999, 12, 30, 3),
    )
    first_days = gs.datetimes(
        ahead_lo, dt.datetime(1, 1, 2, tzinfo=UTC), timezones=gs.just(shifted)
    )
    check_all(
        first_days, lambda d: d.tzinfo is shifted and d.replace(tzinfo=None) < dt.datetime(1, 1, 3)
    )
    assert find_example(first_days, lambda d: d.day == 1).day == 1


def test_timedeltas_shrink_towards_zero():
    assert find_example(gs.timedeltas(), lambda d: d.days > 0) == dt.timedelta(1)
    assert find_example(gs.timedeltas(), lambda d: d < dt.timedelta(0)) == dt.timedelta(
        microseconds=-1
    )
    assert find_example(gs.timedeltas(), lambda d: d > dt.timedelta(0)) == dt.timedelta(
        microseconds=1
    )
    assert find_example(
        gs.timedeltas(dt.timedelta(minutes=-1), dt.timedelta(minutes=1)),
        lambda d: d < dt.timedelta(seconds=-30),
    ) == dt.timedelta(seconds=-30, microseconds=-1)


def test_timedeltas():
    check_all(gs.timedeltas(), lambda d: isinstance(d, dt.timedelta))
    lo, hi = dt.timedelta(seconds=-1), dt.timedelta(seconds=1)
    check_all(gs.timedeltas(lo, hi), lambda d: lo <= d <= hi)
    assert find_example(gs.timedeltas()) == dt.timedelta(0)
    assert repr(gs.timedeltas(lo, hi)) == f"timedeltas(min_value={lo!r}, max_value={hi!r})"
    check_all(gs.timedeltas(lo, lo), lambda d: d == lo)
    check_all(gs.timedeltas(dt.timedelta.min, dt.timedelta.min), lambda d: d == dt.timedelta.min)
    check_all(gs.timedeltas(dt.timedelta.max, dt.timedelta.max), lambda d: d == dt.timedelta.max)
    bounded_above = gs.timedeltas(max_value=dt.timedelta(10**6))
    check_all(bounded_above, lambda d: d <= dt.timedelta(10**6))
    assert find_example(bounded_above, lambda d: d > dt.timedelta(hours=1)) == dt.timedelta(
        hours=1, microseconds=1
    )
    with pytest.raises(hegel.InvalidArgument):
        gs.timedeltas(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.timedeltas(1)


def test_timezone_keys():
    check_all(gs.timezone_keys(), lambda key: zoneinfo.ZoneInfo(key).key == key)
    assert find_example(gs.timezone_keys()) == "UTC"
    assert find_example(gs.timezone_keys(), lambda key: key != "UTC") != "UTC"
    assert repr(gs.timezone_keys()) == "timezone_keys(links=True, legacy=False)"

    def tags_of(key):
        info = _native.timezone_info(key)
        assert info is not None
        return info

    check_all(gs.timezone_keys(), lambda key: not tags_of(key)[3] & _native.TIMEZONE_TAG_LEGACY)
    check_all(gs.timezone_keys(links=False), lambda key: not tags_of(key)[1])
    find_example(gs.timezone_keys(), lambda key: tags_of(key)[1])
    assert find_example(
        gs.timezone_keys(legacy=True), lambda key: tags_of(key)[3] & _native.TIMEZONE_TAG_LEGACY
    )
    with pytest.raises(hegel.InvalidArgument, match="links=1 must be a bool"):
        gs.timezone_keys(links=1)
    with pytest.raises(hegel.InvalidArgument, match="legacy=1 must be a bool"):
        gs.timezone_keys(legacy=1)


def test_timezone_keys_are_the_hosts():
    assert _select_keys({"UTC", "Mars/Olympus_Mons", "Europe/London"}, True, False) == [
        "Europe/London",
        "UTC",
    ]
    with pytest.raises(hegel.InvalidArgument, match="install the tzdata package"):
        _select_keys({"Mars/Olympus_Mons"}, True, False)
    assert re.fullmatch(r"\d{4}[a-z]", gs.tzdb_version())


def test_timezones():
    check_all(gs.timezones(), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.timezones(no_cache=True), lambda z: z is not zoneinfo.ZoneInfo(z.key))
    check_all(gs.timezones(), lambda z: z is zoneinfo.ZoneInfo(z.key))
    assert find_example(gs.timezones()) is zoneinfo.ZoneInfo("UTC")
    assert repr(gs.timezones()) == "timezones(links=True, legacy=False, no_cache=False)"
    with pytest.raises(hegel.InvalidArgument, match="no_cache=1 must be a bool"):
        gs.timezones(no_cache=1)
    with pytest.raises(hegel.InvalidArgument, match="legacy=1 must be a bool"):
        gs.timezones(legacy=1)


def foreign_zone(key: str, *parts: str) -> zoneinfo.ZoneInfo:
    """A ZoneInfo with real rules under a key the engine does not know."""
    tzif = files("tzdata.zoneinfo").joinpath(*parts).read_bytes()
    return zoneinfo.ZoneInfo.from_file(io.BytesIO(tzif), key=key)


def test_dates_on_chosen_weekdays_and_months():
    weekends = gs.dates(weekdays=[5, 6])
    check_all(weekends, lambda d: d.weekday() in (5, 6))
    assert find_example(weekends) == dt.date(2000, 1, 1)
    assert find_example(gs.dates(weekdays=[0])) == dt.date(2000, 1, 3)
    check_all(gs.dates(months=[2]), lambda d: d.month == 2)
    assert find_example(gs.dates(months=[2]), lambda d: d.day == 29) == dt.date(2000, 2, 29)
    check_all(
        gs.dates(dt.date(2024, 1, 1), dt.date(2024, 12, 31), weekdays=[4], months=[6, 7]),
        lambda d: d.weekday() == 4 and d.month in (6, 7) and d.year == 2024,
    )
    check_all(gs.dates(weekdays=range(7), months=range(1, 13)), lambda d: True)
    with pytest.raises(hegel.InvalidArgument, match="must not be empty"):
        gs.dates(weekdays=[])
    with pytest.raises(hegel.InvalidArgument, match="ints from 0 to 6, not 7"):
        gs.dates(weekdays=[7])
    with pytest.raises(hegel.InvalidArgument, match="ints from 1 to 12, not 0"):
        gs.dates(months=[0])
    with pytest.raises(hegel.InvalidArgument, match="not True"):
        gs.dates(months=[True])
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of ints"):
        gs.dates(weekdays=5)
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of ints"):
        gs.dates(months="2")
    with pytest.raises(hegel.InvalidArgument, match="no date between"):
        gs.dates(dt.date(2024, 6, 1), dt.date(2024, 6, 30), months=[2])
    with pytest.raises(hegel.InvalidArgument, match="no date between"):
        gs.dates(dt.date(2024, 6, 1), dt.date(2024, 6, 1), weekdays=[0])
    gs.dates(dt.date(2024, 3, 1), dt.date(2025, 2, 28), weekdays=[0], months=[2])


def test_times_at_a_precision():
    check_all(gs.times(precision="seconds"), lambda t: t.microsecond == 0)
    check_all(gs.times(precision="milliseconds"), lambda t: t.microsecond % 1000 == 0)
    check_all(gs.times(precision="minutes"), lambda t: t.second == 0 and t.microsecond == 0)
    whole_hours = gs.times(precision="hours")
    check_all(whole_hours, lambda t: t == dt.time(t.hour))
    assert find_example(whole_hours, lambda t: t > dt.time(0)) == dt.time(1)
    check_all(
        gs.times(dt.time(1, 30), dt.time(1, 59), precision="minutes"), lambda t: t.second == 0
    )
    with pytest.raises(hegel.InvalidArgument, match="whole number of hours"):
        gs.times(dt.time(1, 1), dt.time(1, 59), precision="hours")
    with pytest.raises(hegel.InvalidArgument, match="precision='nanoseconds' must be one of"):
        gs.times(precision="nanoseconds")
    with pytest.raises(hegel.InvalidArgument, match="precision=1 must be one of"):
        gs.times(precision=1)


def test_datetimes_at_a_precision():
    check_all(gs.datetimes(precision="seconds"), lambda d: d.microsecond == 0)
    check_all(
        gs.datetimes(precision="hours", timezones=gs.timezones()),
        lambda d: d.time() == dt.time(d.hour),
    )
    check_all(
        gs.datetimes(precision="minutes", timezones=gs.just(dt.timezone(dt.timedelta(hours=1)))),
        lambda d: d.second == 0 and d.microsecond == 0,
    )
    assert find_example(gs.datetimes(precision="hours"), lambda d: d.hour) == dt.datetime(
        2000, 1, 1, 1
    )
    lo = dt.datetime(2020, 1, 1, 23, 59, 30)
    check_all(gs.datetimes(lo, lo + dt.timedelta(minutes=1), precision="hours"), lambda d: True)
    with pytest.raises(hegel.InvalidArgument, match="whole number of minutes"):
        gs.datetimes(lo, lo + dt.timedelta(seconds=10), precision="minutes")
    with pytest.raises(hegel.InvalidArgument, match="precision='days' must be one of"):
        gs.datetimes(precision="days")
    # Whether aware bounds hold a wall time at the precision depends on the
    # zone, so the engine reports it when the datetime is drawn.
    narrow = gs.datetimes(
        lo.replace(tzinfo=UTC),
        lo.replace(tzinfo=UTC) + dt.timedelta(seconds=10),
        precision="minutes",
    )

    def body(tc):
        tc.draw(narrow)

    failing(body, hegel.InvalidArgument)


def test_datetimes_choose_the_fold():
    lo, hi = dt.datetime(2020, 11, 1, 1, 0), dt.datetime(2020, 11, 1, 1, 59)
    check_all(
        gs.datetimes(lo, hi, timezones=gs.just(NEW_YORK), ambiguous="earlier"),
        lambda d: d.fold == 0 and d.utcoffset() == dt.timedelta(hours=-4),
    )
    check_all(
        gs.datetimes(lo, hi, timezones=gs.just(NEW_YORK), ambiguous="later"),
        lambda d: d.fold == 1 and d.utcoffset() == dt.timedelta(hours=-5),
    )
    alo, ahi = dt.datetime(2020, 11, 1, 5, tzinfo=UTC), dt.datetime(2020, 11, 1, 6, 59, tzinfo=UTC)
    check_all(gs.datetimes(alo, ahi, ambiguous="later"), lambda d: d.fold in (0, 1))
    with pytest.raises(hegel.InvalidArgument, match="ambiguous='both_folds' must be one of"):
        gs.datetimes(ambiguous="both_folds")


def test_datetimes_with_aware_bounds_in_a_zone_the_engine_cannot_see():
    london = foreign_zone("Mars/London", "Europe", "London")
    # The clocks went forward from 01:00 GMT to 02:00 BST at 01:00Z on
    # 2020-03-29, so the wall times from 01:00 to 01:59 never happened and
    # read, with fold 0, as instants past the upper bound.
    lo = dt.datetime(2020, 3, 29, 0, 59, 30, tzinfo=UTC)
    hi = dt.datetime(2020, 3, 29, 1, 0, 30, tzinfo=UTC)
    in_bounds = gs.datetimes(lo, hi, timezones=gs.just(london))
    check_all(in_bounds, lambda d: instant(lo) <= instant(d) <= instant(hi), test_cases=100)
    assert find_example(in_bounds, lambda d: d.hour == 2).replace(tzinfo=None) == dt.datetime(
        2020, 3, 29, 2
    )
    check_all(
        gs.datetimes(lo, hi, timezones=gs.just(london), allow_imaginary=False),
        lambda d: not _is_imaginary(d),
    )
    # The clocks went back from 02:00 BST to 01:00 GMT at 01:00Z on
    # 2020-10-25, so these bounds are in order as instants but not as wall
    # times in London.
    lo = dt.datetime(2020, 10, 25, 0, 59, tzinfo=UTC)
    hi = dt.datetime(2020, 10, 25, 1, 0, 30, tzinfo=UTC)
    assert _local_wall(lo, london) > _local_wall(hi, london)

    def body(tc):
        tc.draw(gs.datetimes(lo, hi, timezones=gs.just(london)))

    failing(body, hegel.Unsatisfiable, test_cases=5)


def test_timedeltas_at_a_precision():
    check_all(gs.timedeltas(precision="seconds"), lambda d: d.microseconds == 0)
    hours = gs.timedeltas(precision="hours")
    check_all(hours, lambda d: d % dt.timedelta(hours=1) == dt.timedelta(0))
    assert find_example(hours, lambda d: d > dt.timedelta(0)) == dt.timedelta(hours=1)
    assert find_example(hours, lambda d: d < dt.timedelta(0)) == dt.timedelta(hours=-1)
    check_all(
        gs.timedeltas(dt.timedelta(seconds=-90), dt.timedelta(seconds=-30), precision="minutes"),
        lambda d: d == dt.timedelta(minutes=-1),
    )
    with pytest.raises(hegel.InvalidArgument, match="whole number of minutes"):
        gs.timedeltas(dt.timedelta(seconds=1), dt.timedelta(seconds=59), precision="minutes")
    with pytest.raises(hegel.InvalidArgument, match="precision='weeks' must be one of"):
        gs.timedeltas(precision="weeks")


def test_utc_offsets():
    def offset(zone):
        return zone.utcoffset(None)

    limit = dt.timedelta(hours=23, minutes=59, seconds=59)
    check_all(
        gs.utc_offsets(), lambda z: isinstance(z, dt.timezone) and -limit <= offset(z) <= limit
    )
    assert find_example(gs.utc_offsets()) == dt.UTC
    assert find_example(gs.utc_offsets(), lambda z: offset(z) > dt.timedelta(0)) == dt.timezone(
        dt.timedelta(seconds=1)
    )
    lo, hi = dt.timedelta(hours=-5), dt.timedelta(hours=1)
    check_all(gs.utc_offsets(lo, hi), lambda z: lo <= offset(z) <= hi)
    hourly = gs.utc_offsets(granularity="hours")
    check_all(hourly, lambda z: offset(z) % dt.timedelta(hours=1) == dt.timedelta(0))
    assert find_example(hourly, lambda z: offset(z) > dt.timedelta(0)) == dt.timezone(
        dt.timedelta(hours=1)
    )
    check_all(
        gs.utc_offsets(granularity="quarter_hours"),
        lambda z: offset(z) % dt.timedelta(minutes=15) == dt.timedelta(0),
    )
    check_all(
        gs.utc_offsets(granularity="minutes"),
        lambda z: offset(z) % dt.timedelta(minutes=1) == dt.timedelta(0),
    )
    check_all(gs.utc_offsets(lo, lo), lambda z: offset(z) == lo)
    assert (
        repr(gs.utc_offsets(lo, hi))
        == f"utc_offsets(min_value={lo!r}, max_value={hi!r}, granularity='seconds')"
    )
    with pytest.raises(hegel.InvalidArgument):
        gs.utc_offsets(hi, lo)
    with pytest.raises(hegel.InvalidArgument, match="must be a timedelta"):
        gs.utc_offsets(3600)
    with pytest.raises(hegel.InvalidArgument, match="whole number of seconds"):
        gs.utc_offsets(max_value=dt.timedelta(microseconds=1))
    with pytest.raises(hegel.InvalidArgument, match="between -23:59:59 and \\+23:59:59"):
        gs.utc_offsets(min_value=dt.timedelta(hours=-24))
    with pytest.raises(hegel.InvalidArgument, match="between -23:59:59 and \\+23:59:59"):
        gs.utc_offsets(max_value=dt.timedelta(hours=24))
    with pytest.raises(hegel.InvalidArgument, match="granularity='days' must be one of"):
        gs.utc_offsets(granularity="days")
    with pytest.raises(hegel.InvalidArgument, match="whole number of hours"):
        gs.utc_offsets(dt.timedelta(minutes=1), dt.timedelta(minutes=59), granularity="hours")


def test_timezone_keys_from_a_list_of_names():
    names = ["Europe/London", "Asia/Calcutta", "EST5EDT", "UTC"]
    keys = gs.timezone_keys(names=names)
    check_all(keys, lambda key: key in names)
    for name in names:
        assert find_example(keys, lambda key: key == name) == name
    assert find_example(keys) == "UTC"
    assert repr(keys) == (
        "timezone_keys(names=['Europe/London', 'Asia/Calcutta', 'EST5EDT', 'UTC'], links=True, "
        "legacy=False)"
    )
    check_all(gs.timezone_keys(names=("UTC",)), lambda key: key == "UTC")
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of IANA"):
        gs.timezone_keys(names="Europe/London")
    with pytest.raises(hegel.InvalidArgument, match="must not be empty"):
        gs.timezone_keys(names=[])
    with pytest.raises(hegel.InvalidArgument, match="must contain strings, not 5"):
        gs.timezone_keys(names=[5])
    with pytest.raises(hegel.InvalidArgument, match="'Mars/Olympus_Mons' is not a name"):
        gs.timezone_keys(names=["UTC", "Mars/Olympus_Mons"])


def test_timezones_from_a_list_of_names(monkeypatch):
    zones = gs.timezones(names=["Europe/London", "America/New_York", "UTC"])
    check_all(zones, lambda z: z in (LONDON, NEW_YORK, zoneinfo.ZoneInfo("UTC")))
    assert find_example(zones) is zoneinfo.ZoneInfo("UTC")
    assert find_example(zones, lambda z: z is not zoneinfo.ZoneInfo("UTC")) in (LONDON, NEW_YORK)
    assert repr(zones) == (
        "timezones(names=['Europe/London', 'America/New_York', 'UTC'], links=True, legacy=False, "
        "no_cache=False)"
    )
    with pytest.raises(hegel.InvalidArgument, match="'Mars/Olympus_Mons' is not a name"):
        gs.timezones(names=["Mars/Olympus_Mons"])

    def missing(key):
        raise zoneinfo.ZoneInfoNotFoundError(key)

    monkeypatch.setattr(zoneinfo, "ZoneInfo", missing)
    with pytest.raises(hegel.InvalidArgument, match="'Europe/London' is not a time zone this"):
        gs.timezones(names=["Europe/London"])


def test_from_type_knows_the_time_zone_types():
    check_all(gs.from_type(dt.tzinfo), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.from_type(zoneinfo.ZoneInfo), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.from_type(dt.timezone), lambda z: isinstance(z, dt.timezone))
