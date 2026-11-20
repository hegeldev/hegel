from __future__ import annotations

import datetime as dt
import io
import re
import warnings
import zoneinfo
from importlib.resources import files

import pytest

import hegel
from hegel import _native
from hegel import generators as gs
from hegel.generators.temporal import _is_imaginary, _is_repeated
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
    check_all(
        gs.temporal.dates(), lambda d: isinstance(d, dt.date) and not isinstance(d, dt.datetime)
    )
    lo, hi = dt.date(2020, 2, 28), dt.date(2020, 3, 1)
    check_all(gs.temporal.dates(lo, hi), lambda d: lo <= d <= hi)
    assert find_example(gs.temporal.dates()) == dt.date(2000, 1, 1)
    assert find_example(gs.temporal.dates(min_value=dt.date(2021, 5, 5))) == dt.date(2021, 5, 5)
    assert repr(gs.temporal.dates(lo, hi)) == f"temporal.dates(min_value={lo!r}, max_value={hi!r})"
    assert repr(gs.temporal.dates()) == "temporal.dates()"
    assert repr(gs.temporal.dates(max_value=hi)) == f"temporal.dates(max_value={hi!r})"
    check_all(gs.temporal.dates(None, hi), lambda d: d <= hi)
    check_all(gs.temporal.dates(lo, None), lambda d: d >= lo)
    assert repr(gs.temporal.dates(lo, hi, weekdays=[6, 5], months=[2, 3])) == (
        f"temporal.dates(min_value={lo!r}, max_value={hi!r}, weekdays=[5, 6], months=[2, 3])"
    )
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.dates(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.dates("2020-01-01")
    with pytest.raises(hegel.InvalidArgument, match="min_value=.* must be a date"):
        gs.temporal.dates(dt.datetime(2020, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="max_value=.* must be a date"):
        gs.temporal.dates(max_value=dt.datetime(2020, 1, 1))


def test_times():
    check_all(gs.temporal.times(), lambda t: isinstance(t, dt.time) and t.tzinfo is None)
    lo, hi = dt.time(1, 2, 3, 4), dt.time(1, 2, 3, 9)
    check_all(gs.temporal.times(lo, hi), lambda t: lo <= t <= hi)
    check_all(gs.temporal.times(zones=gs.just(UTC)), lambda t: t.tzinfo is UTC)
    assert find_example(gs.temporal.times()) == dt.time(0, 0)
    assert find_example(gs.temporal.times(), lambda t: t > dt.time(0, 0)) == dt.time(0, 0, 0, 1)
    assert repr(gs.temporal.times(lo, hi)) == f"temporal.times(min_value={lo!r}, max_value={hi!r})"
    assert repr(gs.temporal.times()) == "temporal.times()"
    assert repr(gs.temporal.times(zones=gs.none(), precision="seconds")) == (
        "temporal.times(zones=none(), precision='seconds')"
    )
    check_all(gs.temporal.times(None, hi), lambda t: t <= hi)
    check_all(gs.temporal.times(lo, None), lambda t: t >= lo)
    check_all(gs.temporal.times(zones=gs.none()), lambda t: t.tzinfo is None)
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.times(hi, lo)
    with pytest.raises(hegel.InvalidArgument, match="min_value=.* must be naive"):
        gs.temporal.times(dt.time(1, tzinfo=UTC))
    with pytest.raises(hegel.InvalidArgument, match="max_value=.* must be naive"):
        gs.temporal.times(max_value=dt.time(1, tzinfo=UTC))
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.times(5)
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.times(zones=5)

    def bad_tz(tc):
        tc.draw(gs.temporal.times(zones=gs.just("UTC")))

    assert "zones must generate tzinfo objects, got 'UTC'" in str(
        failing(bad_tz, hegel.InvalidArgument)
    )


def test_datetimes():
    check_all(gs.temporal.datetimes(), lambda d: isinstance(d, dt.datetime) and d.tzinfo is None)
    lo, hi = dt.datetime(2020, 1, 1, 1, 2, 3, 4), dt.datetime(2020, 1, 1, 1, 2, 3, 9)
    check_all(gs.temporal.datetimes(lo, hi), lambda d: lo <= d <= hi)
    check_all(
        gs.temporal.datetimes(zones=gs.temporal.zones()),
        lambda d: isinstance(d.tzinfo, zoneinfo.ZoneInfo),
    )
    check_all(gs.temporal.datetimes(zones=gs.none()), lambda d: d.tzinfo is None)
    assert find_example(gs.temporal.datetimes()) == dt.datetime(2000, 1, 1)
    assert find_example(gs.temporal.datetimes(zones=gs.temporal.zones())) == dt.datetime(
        2000, 1, 1, tzinfo=zoneinfo.ZoneInfo("UTC")
    )
    assert (
        repr(gs.temporal.datetimes(lo, hi))
        == f"temporal.datetimes(min_value={lo!r}, max_value={hi!r})"
    )
    assert repr(
        gs.temporal.datetimes(
            zones=gs.none(),
            allow_imaginary=False,
            ambiguous="later",
            precision="seconds",
        )
    ) == (
        "temporal.datetimes(zones=none(), allow_imaginary=False, ambiguous='later', "
        "precision='seconds')"
    )
    assert repr(gs.temporal.datetimes()) == "temporal.datetimes()"
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.datetimes(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.datetimes(dt.date(2020, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="allow_imaginary=0 must be a bool"):
        gs.temporal.datetimes(allow_imaginary=0)

    def bad_tz(tc):
        tc.draw(gs.temporal.datetimes(zones=gs.just("UTC")))

    failing(bad_tz, hegel.InvalidArgument)


def test_datetimes_in_a_zone_from_any_generator():
    zones = gs.sampled_from([NEW_YORK, LONDON])
    check_all(gs.temporal.datetimes(zones=zones), lambda d: d.tzinfo in (NEW_YORK, LONDON))
    fixed = dt.timezone(dt.timedelta(hours=5, minutes=45))
    lo, hi = dt.datetime(2020, 1, 1), dt.datetime(2020, 1, 2)
    check_all(
        gs.temporal.datetimes(lo, hi, zones=gs.just(fixed)),
        lambda d: d.tzinfo is fixed and lo <= d.replace(tzinfo=None) <= hi and d.fold == 0,
    )


def test_datetimes_draw_both_folds():
    # America/New_York fell back from 02:00 EDT to 01:00 EST on 2020-11-01,
    # so wall times between 01:00 and 02:00 are repeated.
    lo, hi = dt.datetime(2020, 11, 1, 1, 0), dt.datetime(2020, 11, 1, 1, 59)
    repeated = gs.temporal.datetimes(lo, hi, zones=gs.just(NEW_YORK))
    for fold in (0, 1):
        value = find_example(repeated, lambda d: d.fold == fold)
        assert value.fold == fold
        assert value.utcoffset() == dt.timedelta(hours=-4 if fold == 0 else -5)
    assert find_example(repeated) == lo.replace(tzinfo=NEW_YORK)


def test_datetimes_draw_gaps_unless_told_not_to():
    # Europe/London sprang forward from 01:00 GMT to 02:00 BST on 2020-03-29.
    lo, hi = dt.datetime(2020, 3, 29, 0, 30), dt.datetime(2020, 3, 29, 2, 30)
    imaginary = gs.temporal.datetimes(lo, hi, zones=gs.just(LONDON))
    skipped = find_example(imaginary, _is_imaginary)
    assert (
        dt.datetime(2020, 3, 29, 1, 0)
        <= skipped.replace(tzinfo=None)
        < dt.datetime(2020, 3, 29, 2, 0)
    )
    real = gs.temporal.datetimes(lo, hi, zones=gs.just(LONDON), allow_imaginary=False)
    check_all(real, lambda d: not _is_imaginary(d), test_cases=200)
    only_gap = gs.temporal.datetimes(
        dt.datetime(2020, 3, 29, 1, 0),
        dt.datetime(2020, 3, 29, 1, 59),
        zones=gs.just(LONDON),
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
            gs.temporal.datetimes(dt.datetime(1990, 1, 1), dt.datetime(2030, 1, 1), zones=zones)
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
        gs.temporal.datetimes(lo, hi),
        lambda d: (
            isinstance(d.tzinfo, zoneinfo.ZoneInfo) and instant(lo) <= instant(d) <= instant(hi)
        ),
        test_cases=200,
    )
    in_new_york = gs.temporal.datetimes(lo, hi, zones=gs.just(NEW_YORK))
    check_all(
        in_new_york, lambda d: d.tzinfo is NEW_YORK and instant(lo) <= instant(d) <= instant(hi)
    )
    for fold in (0, 1):
        assert find_example(in_new_york, lambda d: d.fold == fold).fold == fold
    fixed = dt.timezone(dt.timedelta(hours=-3))
    check_all(
        gs.temporal.datetimes(lo, hi, zones=gs.just(fixed)),
        lambda d: d.tzinfo is fixed and instant(lo) <= instant(d) <= instant(hi),
    )
    check_all(
        gs.temporal.datetimes(min_value=dt.datetime(2020, 1, 1, tzinfo=UTC)),
        lambda d: instant(d) >= instant(dt.datetime(2020, 1, 1, tzinfo=UTC)),
    )
    check_all(
        gs.temporal.datetimes(max_value=dt.datetime(2020, 1, 1, tzinfo=UTC)),
        lambda d: instant(d) <= instant(dt.datetime(2020, 1, 1, tzinfo=UTC)),
    )
    with pytest.raises(hegel.InvalidArgument, match="both naive or both aware"):
        gs.temporal.datetimes(lo, dt.datetime(2021, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="both naive or both aware"):
        gs.temporal.datetimes(dt.datetime(2019, 1, 1), hi)
    with pytest.raises(hegel.InvalidArgument, match="moments in time"):
        gs.temporal.datetimes(hi, lo)
    # fold=1 is the later moment, so these bounds are ordered by wall clock
    # time but not as instants.
    early = dt.datetime(2020, 11, 1, 1, 45, tzinfo=NEW_YORK, fold=1)
    late = dt.datetime(2020, 11, 1, 1, 50, tzinfo=NEW_YORK)
    with pytest.raises(hegel.InvalidArgument, match="moments in time"):
        gs.temporal.datetimes(early, late)

    def naive_zone(tc):
        tc.draw(gs.temporal.datetimes(lo, hi, zones=gs.none()))

    assert "every datetime needs a tzinfo" in str(failing(naive_zone, hegel.InvalidArgument))


class Shifted(dt.tzinfo):
    """A tzinfo the engine knows nothing about."""

    def utcoffset(self, value):
        return dt.timedelta(hours=1)

    def dst(self, value):
        return dt.timedelta(0)

    def tzname(self, value):
        return "SHIFT"


def unkeyed_zone(*parts: str) -> zoneinfo.ZoneInfo:
    """A ZoneInfo with real rules and no key, which no zone source can
    look up."""
    tzif = files("tzdata.zoneinfo").joinpath(*parts).read_bytes()
    zone = zoneinfo.ZoneInfo.from_file(io.BytesIO(tzif))
    assert zone.key is None
    return zone


def test_datetimes_in_zones_the_engine_cannot_see():
    anonymous = unkeyed_zone("UTC")
    shifted = Shifted()
    zones = gs.sampled_from([anonymous, shifted])
    lo, hi = dt.datetime(2020, 1, 1), dt.datetime(2020, 1, 2)
    check_all(
        gs.temporal.datetimes(lo, hi, zones=zones),
        lambda d: d.tzinfo in (anonymous, shifted) and lo <= d.replace(tzinfo=None) <= hi,
    )
    check_all(
        gs.temporal.datetimes(lo, hi, zones=zones, allow_imaginary=False),
        lambda d: not _is_imaginary(d),
    )
    alo, ahi = lo.replace(tzinfo=UTC), hi.replace(tzinfo=UTC)
    check_all(
        gs.temporal.datetimes(alo, ahi, zones=gs.just(shifted)),
        lambda d: d.tzinfo is shifted and instant(alo) <= instant(d) <= instant(ahi),
    )
    # The engine cannot see this zone's gap, so an imaginary wall time is
    # detected after the draw and rejects the test case.
    london = unkeyed_zone("Europe", "London")
    gap_lo, gap_hi = dt.datetime(2020, 3, 29, 1, 0), dt.datetime(2020, 3, 29, 1, 59)
    check_all(gs.temporal.datetimes(gap_lo, gap_hi, zones=gs.just(london)), _is_imaginary)

    def body(tc):
        tc.draw(gs.temporal.datetimes(gap_lo, gap_hi, zones=gs.just(london), allow_imaginary=False))

    failing(body, hegel.FailedHealthCheck, test_cases=5)


def test_wall_times_repeated_in_a_zone_the_engine_cannot_see():
    # The clocks went back from 02:00 BST to 01:00 GMT on 2020-10-25, so the
    # wall times from 01:00 to 01:59 happened twice.
    london = unkeyed_zone("Europe", "London")
    lo, hi = dt.datetime(2020, 10, 25, 0, 30), dt.datetime(2020, 10, 25, 2, 30)

    def repeated(value):
        return value.hour == 1

    both = gs.temporal.datetimes(lo, hi, zones=gs.just(london))
    check_all(both, lambda d: d.fold == 0 or repeated(d), test_cases=100)
    for fold, offset in ((0, 1), (1, 0)):
        value = find_example(both, lambda d: d.fold == fold and repeated(d))
        assert value.utcoffset() == dt.timedelta(hours=offset)
    assert find_example(both, repeated).fold == 0
    earlier = gs.temporal.datetimes(lo, hi, zones=gs.just(london), ambiguous="earlier")
    check_all(earlier, lambda d: d.fold == 0, test_cases=100)
    later = gs.temporal.datetimes(lo, hi, zones=gs.just(london), ambiguous="later")
    check_all(later, lambda d: d.fold == repeated(d), test_cases=100)
    assert find_example(later, repeated).utcoffset() == dt.timedelta(0)
    assert _is_repeated(dt.datetime(2020, 10, 25, 1, 30, tzinfo=london))
    assert _is_repeated(dt.datetime(2020, 10, 25, 1, 30, tzinfo=london, fold=1))
    assert not _is_repeated(dt.datetime(2020, 10, 25, 2, 30, tzinfo=london))
    # A skipped wall time also reads differently by fold, and is not a
    # repeated one.
    assert not _is_repeated(dt.datetime(2020, 3, 29, 1, 30, tzinfo=london))


def test_is_imaginary_at_the_limits_of_the_type():
    # Neither wall time can be converted to UTC, which overflows.
    west = dt.timezone(dt.timedelta(hours=-1))
    assert not _is_imaginary(dt.datetime.max.replace(tzinfo=west))
    east = dt.timezone(dt.timedelta(hours=1))
    assert not _is_imaginary(dt.datetime.min.replace(tzinfo=east))
    check_all(
        gs.temporal.datetimes(
            dt.datetime.max - dt.timedelta(minutes=30),
            zones=gs.just(west),
            allow_imaginary=False,
        ),
        lambda d: d.tzinfo is west,
    )


def test_datetimes_near_the_limits_of_the_type():
    # In a zone fourteen hours ahead of UTC the last instants of 9999 fall in
    # the year 10000, which datetime cannot hold.
    far_east = zoneinfo.ZoneInfo("Etc/GMT-14")
    last = dt.datetime.max.replace(tzinfo=UTC)

    def body(tc):
        tc.draw(gs.temporal.datetimes(last, last, zones=gs.just(far_east)))

    failing(body, hegel.FailedHealthCheck, test_cases=5)

    def fixed_body(tc):
        tc.draw(
            gs.temporal.datetimes(last, last, zones=gs.just(dt.timezone(dt.timedelta(hours=14))))
        )

    failing(fixed_body, hegel.FailedHealthCheck, test_cases=5)
    # Moments outside the range a UTC datetime represents are never drawn
    # in a zone the engine cannot see, so bounds wholly outside it reject
    # every test case.
    behind_lo = dt.datetime.max.replace(tzinfo=dt.timezone(dt.timedelta(hours=-4)))
    behind_hi = dt.datetime.max.replace(tzinfo=dt.timezone(dt.timedelta(hours=-5)))
    assert instant(behind_lo) < instant(behind_hi)
    west = dt.timezone(dt.timedelta(hours=-1))

    def behind_body(tc):
        tc.draw(gs.temporal.datetimes(behind_lo, behind_hi, zones=gs.just(west)))

    failing(behind_body, hegel.Unsatisfiable, test_cases=5)
    ahead_lo = dt.datetime.min.replace(tzinfo=dt.timezone(dt.timedelta(hours=5)))
    ahead_hi = dt.datetime.min.replace(tzinfo=dt.timezone(dt.timedelta(hours=4)))
    shifted = Shifted()

    def ahead_body(tc):
        tc.draw(gs.temporal.datetimes(ahead_lo, ahead_hi, zones=gs.just(shifted)))

    failing(ahead_body, hegel.Unsatisfiable, test_cases=5)
    # A bound beyond that range is clamped to it.
    check_all(
        gs.temporal.datetimes(behind_lo.replace(day=30), behind_hi, zones=gs.just(west)),
        lambda d: d.tzinfo is west and d.replace(tzinfo=None) >= dt.datetime(9999, 12, 31, 2),
    )
    first_days = gs.temporal.datetimes(
        ahead_lo, dt.datetime(1, 1, 2, tzinfo=UTC), zones=gs.just(shifted)
    )
    check_all(
        first_days, lambda d: d.tzinfo is shifted and d.replace(tzinfo=None) < dt.datetime(1, 1, 3)
    )
    assert find_example(first_days, lambda d: d.day == 1).day == 1


def test_durations_shrink_towards_zero():
    assert find_example(gs.temporal.durations(), lambda d: d.days > 0) == dt.timedelta(1)
    assert find_example(gs.temporal.durations(), lambda d: d < dt.timedelta(0)) == dt.timedelta(
        microseconds=-1
    )
    assert find_example(gs.temporal.durations(), lambda d: d > dt.timedelta(0)) == dt.timedelta(
        microseconds=1
    )
    assert find_example(
        gs.temporal.durations(dt.timedelta(minutes=-1), dt.timedelta(minutes=1)),
        lambda d: d < dt.timedelta(seconds=-30),
    ) == dt.timedelta(seconds=-30, microseconds=-1)


def test_durations():
    check_all(gs.temporal.durations(), lambda d: isinstance(d, dt.timedelta))
    lo, hi = dt.timedelta(seconds=-1), dt.timedelta(seconds=1)
    check_all(gs.temporal.durations(lo, hi), lambda d: lo <= d <= hi)
    assert find_example(gs.temporal.durations()) == dt.timedelta(0)
    assert (
        repr(gs.temporal.durations(lo, hi))
        == f"temporal.durations(min_value={lo!r}, max_value={hi!r})"
    )
    assert repr(gs.temporal.durations(lo, hi, precision="seconds")) == (
        f"temporal.durations(min_value={lo!r}, max_value={hi!r}, precision='seconds')"
    )
    check_all(gs.temporal.durations(lo, lo), lambda d: d == lo)
    check_all(
        gs.temporal.durations(dt.timedelta.min, dt.timedelta.min), lambda d: d == dt.timedelta.min
    )
    check_all(
        gs.temporal.durations(dt.timedelta.max, dt.timedelta.max), lambda d: d == dt.timedelta.max
    )
    bounded_above = gs.temporal.durations(max_value=dt.timedelta(10**6))
    check_all(bounded_above, lambda d: d <= dt.timedelta(10**6))
    assert find_example(bounded_above, lambda d: d > dt.timedelta(hours=1)) == dt.timedelta(
        hours=1, microseconds=1
    )
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.durations(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.durations(1)


def test_zone_names():
    check_all(gs.temporal.zone_names(), lambda key: zoneinfo.ZoneInfo(key).key == key)
    assert find_example(gs.temporal.zone_names()) == "UTC"
    assert find_example(gs.temporal.zone_names(), lambda key: key != "UTC") != "UTC"
    assert repr(gs.temporal.zone_names()) == "temporal.zone_names()"
    assert repr(gs.temporal.zone_names(allow_links=False, allow_legacy=True)) == (
        "temporal.zone_names(allow_links=False, allow_legacy=True)"
    )
    info = gs.temporal.default_zone_source().info

    check_all(gs.temporal.zone_names(), lambda key: not info(key).is_legacy)
    check_all(gs.temporal.zone_names(allow_links=False), lambda key: not info(key).is_link)
    find_example(gs.temporal.zone_names(), lambda key: info(key).is_link)
    assert find_example(gs.temporal.zone_names(allow_legacy=True), lambda key: info(key).is_legacy)
    with pytest.raises(hegel.InvalidArgument, match="allow_links=1 must be a bool"):
        gs.temporal.zone_names(allow_links=1)
    with pytest.raises(hegel.InvalidArgument, match="allow_legacy=1 must be a bool"):
        gs.temporal.zone_names(allow_legacy=1)


def test_zone_names_are_the_hosts():
    check_all(gs.temporal.zone_names(), lambda key: key in zoneinfo.available_timezones())
    assert re.fullmatch(r"\d{4}[a-z]", gs.temporal.tzdb_version())


def test_zones():
    check_all(gs.temporal.zones(), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.temporal.zones(no_cache=True), lambda z: z is not zoneinfo.ZoneInfo(z.key))
    check_all(gs.temporal.zones(), lambda z: z is zoneinfo.ZoneInfo(z.key))
    assert find_example(gs.temporal.zones()) is zoneinfo.ZoneInfo("UTC")
    assert repr(gs.temporal.zones()) == "temporal.zones()"
    assert repr(gs.temporal.zones(no_cache=True, allow_legacy=True)) == (
        "temporal.zones(no_cache=True, allow_legacy=True)"
    )
    with pytest.raises(hegel.InvalidArgument, match="no_cache=1 must be a bool"):
        gs.temporal.zones(no_cache=1)
    with pytest.raises(hegel.InvalidArgument, match="allow_legacy=1 must be a bool"):
        gs.temporal.zones(allow_legacy=1)


def test_dates_on_chosen_weekdays_and_months():
    weekends = gs.temporal.dates(weekdays=[5, 6])
    check_all(weekends, lambda d: d.weekday() in (5, 6))
    assert find_example(weekends) == dt.date(2000, 1, 1)
    assert find_example(gs.temporal.dates(weekdays=[0])) == dt.date(2000, 1, 3)
    check_all(gs.temporal.dates(months=[2]), lambda d: d.month == 2)
    assert find_example(gs.temporal.dates(months=[2]), lambda d: d.day == 29) == dt.date(
        2000, 2, 29
    )
    check_all(
        gs.temporal.dates(dt.date(2024, 1, 1), dt.date(2024, 12, 31), weekdays=[4], months=[6, 7]),
        lambda d: d.weekday() == 4 and d.month in (6, 7) and d.year == 2024,
    )
    check_all(gs.temporal.dates(weekdays=range(7), months=range(1, 13)), lambda d: True)
    with pytest.raises(hegel.InvalidArgument, match="must name at least one weekday"):
        gs.temporal.dates(weekdays=[])
    with pytest.raises(hegel.InvalidArgument, match="must name at least one month"):
        gs.temporal.dates(months=())
    with pytest.raises(hegel.InvalidArgument, match="ints from 0 to 6, not 7"):
        gs.temporal.dates(weekdays=[7])
    with pytest.raises(hegel.InvalidArgument, match="ints from 1 to 12, not 0"):
        gs.temporal.dates(months=[0])
    with pytest.raises(hegel.InvalidArgument, match="not True"):
        gs.temporal.dates(months=[True])
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of ints"):
        gs.temporal.dates(weekdays=5)
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of ints"):
        gs.temporal.dates(months="2")
    with pytest.raises(hegel.InvalidArgument, match="falls on an allowed weekday and month"):
        gs.temporal.dates(dt.date(2024, 6, 1), dt.date(2024, 6, 30), months=[2])
    with pytest.raises(hegel.InvalidArgument, match="falls on an allowed weekday and month"):
        gs.temporal.dates(dt.date(2024, 6, 1), dt.date(2024, 6, 1), weekdays=[0])
    gs.temporal.dates(dt.date(2024, 3, 1), dt.date(2025, 2, 28), weekdays=[0], months=[2])


def test_times_at_a_precision():
    check_all(gs.temporal.times(precision="seconds"), lambda t: t.microsecond == 0)
    check_all(gs.temporal.times(precision="milliseconds"), lambda t: t.microsecond % 1000 == 0)
    check_all(
        gs.temporal.times(precision="minutes"), lambda t: t.second == 0 and t.microsecond == 0
    )
    whole_hours = gs.temporal.times(precision="hours")
    check_all(whole_hours, lambda t: t == dt.time(t.hour))
    assert find_example(whole_hours, lambda t: t > dt.time(0)) == dt.time(1)
    check_all(
        gs.temporal.times(dt.time(1, 30), dt.time(1, 59), precision="minutes"),
        lambda t: t.second == 0,
    )
    with pytest.raises(hegel.InvalidArgument, match="no value at precision hours"):
        gs.temporal.times(dt.time(1, 1), dt.time(1, 59), precision="hours")
    with pytest.raises(hegel.InvalidArgument, match="precision='nanoseconds' must be one of"):
        gs.temporal.times(precision="nanoseconds")
    with pytest.raises(hegel.InvalidArgument, match="precision=1 must be one of"):
        gs.temporal.times(precision=1)


def test_datetimes_at_a_precision():
    check_all(gs.temporal.datetimes(precision="seconds"), lambda d: d.microsecond == 0)
    check_all(
        gs.temporal.datetimes(precision="hours", zones=gs.temporal.zones()),
        lambda d: d.time() == dt.time(d.hour),
    )
    check_all(
        gs.temporal.datetimes(
            precision="minutes", zones=gs.just(dt.timezone(dt.timedelta(hours=1)))
        ),
        lambda d: d.second == 0 and d.microsecond == 0,
    )
    assert find_example(gs.temporal.datetimes(precision="hours"), lambda d: d.hour) == dt.datetime(
        2000, 1, 1, 1
    )
    lo = dt.datetime(2020, 1, 1, 23, 59, 30)
    check_all(
        gs.temporal.datetimes(lo, lo + dt.timedelta(minutes=1), precision="hours"), lambda d: True
    )
    with pytest.raises(hegel.InvalidArgument, match="no value at precision minutes"):
        gs.temporal.datetimes(lo, lo + dt.timedelta(seconds=10), precision="minutes")
    with pytest.raises(hegel.InvalidArgument, match="precision='days' must be one of"):
        gs.temporal.datetimes(precision="days")
    # Whether aware bounds hold a wall time at the precision depends on the
    # zone, so the engine reports it when the datetime is drawn.
    narrow = gs.temporal.datetimes(
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
        gs.temporal.datetimes(lo, hi, zones=gs.just(NEW_YORK), ambiguous="earlier"),
        lambda d: d.fold == 0 and d.utcoffset() == dt.timedelta(hours=-4),
    )
    check_all(
        gs.temporal.datetimes(lo, hi, zones=gs.just(NEW_YORK), ambiguous="later"),
        lambda d: d.fold == 1 and d.utcoffset() == dt.timedelta(hours=-5),
    )
    # Both readings of those wall times lie in these aware bounds, the
    # earlier in the first hour and the later in the second.
    alo, ahi = dt.datetime(2020, 11, 1, 5, tzinfo=UTC), dt.datetime(2020, 11, 1, 6, 59, tzinfo=UTC)
    middle = dt.datetime(2020, 11, 1, 6, tzinfo=UTC)
    # The engine rejects the keyed zone before drawing anything, where the
    # zone it cannot see is found out after the draw.
    for zones, rejection in (
        (gs.just(NEW_YORK), hegel.Unsatisfiable),
        (gs.just(unkeyed_zone("America", "New_York")), hegel.FailedHealthCheck),
    ):
        check_all(
            gs.temporal.datetimes(alo, ahi, zones=zones, ambiguous="earlier"),
            lambda d: d.fold == 0 and alo <= d < middle,
        )
        check_all(
            gs.temporal.datetimes(alo, ahi, zones=zones, ambiguous="later"),
            lambda d: d.fold == 1 and middle <= d <= ahi,
        )
        both = gs.temporal.datetimes(alo, ahi, zones=zones)
        check_all(both, lambda d: d.fold == (d >= middle) and alo <= d <= ahi)
        for fold in (0, 1):
            assert find_example(both, lambda d: d.fold == fold).fold == fold

        # Every wall time in the later hour has its earlier reading
        # before these bounds.
        def body(tc, zones=zones):
            tc.draw(gs.temporal.datetimes(middle, ahi, zones=zones, ambiguous="earlier"))

        failing(body, rejection, test_cases=5)
    check_all(
        gs.temporal.datetimes(alo, ahi, ambiguous="later"),
        lambda d: d.fold == 1 or d.replace(fold=1).utcoffset() == d.utcoffset(),
        test_cases=200,
    )
    with pytest.raises(hegel.InvalidArgument, match="ambiguous='both_folds' must be one of"):
        gs.temporal.datetimes(ambiguous="both_folds")


def test_datetimes_with_aware_bounds_in_a_zone_the_engine_cannot_see():
    london = unkeyed_zone("Europe", "London")
    # The clocks went forward from 01:00 GMT to 02:00 BST at 01:00Z on
    # 2020-03-29, so the wall times from 01:00 to 01:59 never happened.
    lo = dt.datetime(2020, 3, 29, 0, 59, 30, tzinfo=UTC)
    hi = dt.datetime(2020, 3, 29, 1, 0, 30, tzinfo=UTC)
    in_bounds = gs.temporal.datetimes(lo, hi, zones=gs.just(london))
    check_all(
        in_bounds,
        lambda d: d.tzinfo is london and lo <= d <= hi and not _is_imaginary(d),
        test_cases=100,
    )
    assert find_example(in_bounds, lambda d: d.hour == 2).replace(tzinfo=None) == dt.datetime(
        2020, 3, 29, 2
    )
    assert find_example(in_bounds, lambda d: d.hour == 0).hour == 0
    # The clocks went back from 02:00 BST to 01:00 GMT at 01:00Z on
    # 2020-10-25, so these bounds are in order as instants but not as wall
    # times in London: they run from 01:59 BST to 01:00:30 GMT.
    lo = dt.datetime(2020, 10, 25, 0, 59, tzinfo=UTC)
    hi = dt.datetime(2020, 10, 25, 1, 0, 30, tzinfo=UTC)
    assert lo.astimezone(london).replace(tzinfo=None) > hi.astimezone(london).replace(tzinfo=None)
    across = gs.temporal.datetimes(lo, hi, zones=gs.just(london))
    check_all(across, lambda d: lo <= d <= hi and d.fold == (d.minute < 30), test_cases=100)
    for fold in (0, 1):
        assert find_example(across, lambda d: d.fold == fold).fold == fold


class HalfHour(dt.tzinfo):
    """A tzinfo the engine knows nothing about, half an hour ahead of UTC
    until 2020-06-01T00:15Z and a whole hour ahead from then on."""

    CHANGE = dt.datetime(2020, 6, 1, 0, 15)

    def utcoffset(self, value):
        after = value.replace(tzinfo=None) >= self.CHANGE + dt.timedelta(minutes=30)
        return dt.timedelta(minutes=60 if after else 30)

    def fromutc(self, value):
        after = value.replace(tzinfo=None) >= self.CHANGE
        return value + dt.timedelta(minutes=60 if after else 30)

    def dst(self, value):
        return dt.timedelta(0)

    def tzname(self, value):
        return "HALF"


def test_precision_with_aware_bounds_in_a_zone_the_engine_cannot_see():
    half = HalfHour()
    lo, hi = dt.datetime(2020, 1, 1, tzinfo=UTC), dt.datetime(2020, 1, 3, tzinfo=UTC)

    def whole_hours_within(lo, hi):
        return lambda d: d.tzinfo is half and d.time() == dt.time(d.hour) and lo <= d <= hi

    hours = gs.temporal.datetimes(lo, hi, zones=gs.just(half), precision="hours")
    check_all(hours, whole_hours_within(lo, hi), test_cases=100)
    # The first whole hour on this zone's clock is half past midnight UTC.
    assert find_example(hours) == dt.datetime(2020, 1, 1, 0, 30, tzinfo=UTC)
    # A lower bound past that one starts at the next.
    later = lo + dt.timedelta(minutes=45)
    from_later = gs.temporal.datetimes(later, hi, zones=gs.just(half), precision="hours")
    check_all(from_later, whole_hours_within(later, hi), test_cases=100)
    assert find_example(from_later) == dt.datetime(2020, 1, 1, 1, 30, tzinfo=UTC)
    # Bounds holding a whole hour on its clock and none on UTC's.
    between = gs.temporal.datetimes(
        lo + dt.timedelta(minutes=20),
        lo + dt.timedelta(minutes=40),
        zones=gs.just(half),
        precision="hours",
    )
    check_all(between, lambda d: d == dt.datetime(2020, 1, 1, 0, 30, tzinfo=UTC))
    check_all(
        gs.temporal.datetimes(lo, hi, zones=gs.just(half), precision="seconds"),
        lambda d: d.microsecond == 0 and lo <= d <= hi,
    )
    # No whole hour on its clock lies in these ten minutes.
    narrow = gs.temporal.datetimes(
        lo, lo + dt.timedelta(minutes=10), zones=gs.just(half), precision="hours"
    )

    def body(tc):
        tc.draw(narrow)

    failing(body, hegel.FailedHealthCheck, test_cases=5)
    # The offset changes between midnight UTC and the whole hour on the
    # zone's clock that would have followed it, so no value comes of
    # midnight.
    change = HalfHour.CHANGE.replace(tzinfo=UTC)
    alo, ahi = change - dt.timedelta(hours=3), change + dt.timedelta(hours=3)
    around = gs.temporal.datetimes(alo, ahi, zones=gs.just(half), precision="hours")
    check_all(around, whole_hours_within(alo, ahi), test_cases=200)
    assert find_example(around, lambda d: d >= change) == dt.datetime(2020, 6, 1, 1, tzinfo=UTC)
    before = find_example(around, lambda d: d < change and d.day == 1)
    assert before == dt.datetime(2020, 5, 31, 23, 30, tzinfo=UTC)

    def crossing(tc):
        tc.draw(
            gs.temporal.datetimes(
                change - dt.timedelta(minutes=15),
                change + dt.timedelta(minutes=40),
                zones=gs.just(half),
                precision="hours",
            )
        )

    failing(crossing, hegel.FailedHealthCheck, test_cases=5)


def test_a_zone_with_no_value_in_aware_bounds_rejects_the_test_case():
    # Asia/Kolkata is five and a half hours ahead of UTC, so no whole hour
    # on its clock lies in these ten seconds, while one does on UTC's.
    kolkata, utc = zoneinfo.ZoneInfo("Asia/Kolkata"), zoneinfo.ZoneInfo("UTC")
    lo = dt.datetime(2020, 1, 1, 12, tzinfo=UTC)
    hi = lo + dt.timedelta(seconds=10)
    for zones in (
        gs.sampled_from([kolkata, utc]),
        gs.temporal.zones(names=["Asia/Kolkata", "UTC"]),
    ):
        check_all(
            gs.temporal.datetimes(lo, hi, zones=zones, precision="hours"),
            lambda d: d.tzinfo is utc and d == lo,
        )


def test_durations_at_a_precision():
    check_all(gs.temporal.durations(precision="seconds"), lambda d: d.microseconds == 0)
    hours = gs.temporal.durations(precision="hours")
    check_all(hours, lambda d: d % dt.timedelta(hours=1) == dt.timedelta(0))
    assert find_example(hours, lambda d: d > dt.timedelta(0)) == dt.timedelta(hours=1)
    assert find_example(hours, lambda d: d < dt.timedelta(0)) == dt.timedelta(hours=-1)
    check_all(
        gs.temporal.durations(
            dt.timedelta(seconds=-90), dt.timedelta(seconds=-30), precision="minutes"
        ),
        lambda d: d == dt.timedelta(minutes=-1),
    )
    with pytest.raises(hegel.InvalidArgument, match="no value at precision minutes"):
        gs.temporal.durations(
            dt.timedelta(seconds=1), dt.timedelta(seconds=59), precision="minutes"
        )
    with pytest.raises(hegel.InvalidArgument, match="precision='weeks' must be one of"):
        gs.temporal.durations(precision="weeks")


def test_offsets():
    def offset(zone):
        return zone.utcoffset(None)

    limit = dt.timedelta(hours=23, minutes=59, seconds=59)
    check_all(
        gs.temporal.offsets(), lambda z: isinstance(z, dt.timezone) and -limit <= offset(z) <= limit
    )
    assert find_example(gs.temporal.offsets()) == dt.UTC
    assert find_example(
        gs.temporal.offsets(), lambda z: offset(z) > dt.timedelta(0)
    ) == dt.timezone(dt.timedelta(seconds=1))
    lo, hi = dt.timedelta(hours=-5), dt.timedelta(hours=1)
    check_all(gs.temporal.offsets(lo, hi), lambda z: lo <= offset(z) <= hi)
    hourly = gs.temporal.offsets(granularity="hours")
    check_all(hourly, lambda z: offset(z) % dt.timedelta(hours=1) == dt.timedelta(0))
    assert find_example(hourly, lambda z: offset(z) > dt.timedelta(0)) == dt.timezone(
        dt.timedelta(hours=1)
    )
    check_all(
        gs.temporal.offsets(granularity="quarter_hours"),
        lambda z: offset(z) % dt.timedelta(minutes=15) == dt.timedelta(0),
    )
    check_all(
        gs.temporal.offsets(granularity="minutes"),
        lambda z: offset(z) % dt.timedelta(minutes=1) == dt.timedelta(0),
    )
    check_all(gs.temporal.offsets(lo, lo), lambda z: offset(z) == lo)
    assert repr(gs.temporal.offsets()) == "temporal.offsets()"
    assert repr(gs.temporal.offsets(lo, hi, granularity="minutes")) == (
        f"temporal.offsets(min_value={lo!r}, max_value={hi!r}, granularity='minutes')"
    )
    check_all(gs.temporal.offsets(None, hi), lambda z: -limit <= offset(z) <= hi)
    check_all(gs.temporal.offsets(lo, None), lambda z: lo <= offset(z) <= limit)
    with pytest.raises(hegel.InvalidArgument):
        gs.temporal.offsets(hi, lo)
    with pytest.raises(hegel.InvalidArgument, match="must be a timedelta"):
        gs.temporal.offsets(3600)
    with pytest.raises(hegel.InvalidArgument, match="whole number of seconds"):
        gs.temporal.offsets(max_value=dt.timedelta(microseconds=1))
    with pytest.raises(hegel.InvalidArgument, match="between -23:59:59 and \\+23:59:59"):
        gs.temporal.offsets(min_value=dt.timedelta(hours=-24))
    with pytest.raises(hegel.InvalidArgument, match="between -23:59:59 and \\+23:59:59"):
        gs.temporal.offsets(max_value=dt.timedelta(hours=24))
    with pytest.raises(hegel.InvalidArgument, match="granularity='days' must be one of"):
        gs.temporal.offsets(granularity="days")
    with pytest.raises(hegel.InvalidArgument, match="no offset at granularity hours"):
        gs.temporal.offsets(dt.timedelta(minutes=1), dt.timedelta(minutes=59), granularity="hours")


def test_zone_names_from_a_list_of_names():
    names = ["Europe/London", "Asia/Calcutta", "EST5EDT", "UTC"]
    keys = gs.temporal.zone_names(names=names)
    check_all(keys, lambda key: key in names)
    for name in names:
        assert find_example(keys, lambda key: key == name) == name
    assert find_example(keys) == "UTC"
    assert repr(keys) == (
        "temporal.zone_names(names=['Europe/London', 'Asia/Calcutta', 'EST5EDT', 'UTC'])"
    )
    check_all(gs.temporal.zone_names(names=("UTC",)), lambda key: key == "UTC")
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of IANA"):
        gs.temporal.zone_names(names="Europe/London")
    with pytest.raises(hegel.InvalidArgument, match="needs at least one allowed name"):
        gs.temporal.zone_names(names=[])
    with pytest.raises(hegel.InvalidArgument, match="must contain strings, not 5"):
        gs.temporal.zone_names(names=[5])
    with pytest.raises(hegel.InvalidArgument, match='unknown time zone "Mars/Olympus_Mons"'):
        gs.temporal.zone_names(names=["UTC", "Mars/Olympus_Mons"])
    for flags in ({"allow_links": False}, {"allow_legacy": True}):
        with pytest.raises(hegel.InvalidArgument, match="do not apply to an explicit names list"):
            gs.temporal.zone_names(names=["UTC"], **flags)
        with pytest.raises(hegel.InvalidArgument, match="do not apply to an explicit names list"):
            gs.temporal.zones(names=["UTC"], **flags)


def test_zones_from_a_list_of_names(monkeypatch):
    zones = gs.temporal.zones(names=["Europe/London", "America/New_York", "UTC"])
    check_all(zones, lambda z: z in (LONDON, NEW_YORK, zoneinfo.ZoneInfo("UTC")))
    assert find_example(zones) is zoneinfo.ZoneInfo("UTC")
    assert find_example(zones, lambda z: z is not zoneinfo.ZoneInfo("UTC")) in (LONDON, NEW_YORK)
    assert repr(zones) == "temporal.zones(names=['Europe/London', 'America/New_York', 'UTC'])"
    with pytest.raises(hegel.InvalidArgument, match='unknown time zone "Mars/Olympus_Mons"'):
        gs.temporal.zones(names=["Mars/Olympus_Mons"])

    def missing(key):
        raise zoneinfo.ZoneInfoNotFoundError(key)

    monkeypatch.setattr(zoneinfo, "ZoneInfo", missing)
    with pytest.raises(hegel.InvalidArgument, match="'Europe/London' is not a time zone this"):
        gs.temporal.zones(names=["Europe/London"])


def test_from_type_knows_the_time_zone_types():
    check_all(gs.from_type(dt.tzinfo), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.from_type(zoneinfo.ZoneInfo), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.from_type(dt.timezone), lambda z: isinstance(z, dt.timezone))


@pytest.mark.parametrize(
    "typ, new",
    [
        (dt.date, "dates"),
        (dt.time, "times"),
        (dt.datetime, "datetimes"),
        (dt.timedelta, "durations"),
        (dt.tzinfo, "zones"),
        (dt.timezone, "offsets"),
        (zoneinfo.ZoneInfo, "zones"),
    ],
)
def test_from_type_uses_the_temporal_generators(typ, new):
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        generator = gs.from_type(typ)
    assert repr(generator) == repr(getattr(gs.temporal, new)())


@pytest.mark.parametrize(
    "old, new, args, kwargs",
    [
        ("dates", "dates", (dt.date(2020, 1, 1), dt.date(2021, 1, 1)), {}),
        ("dates", "dates", (), {}),
        ("times", "times", (dt.time(1), dt.time(2)), {}),
        ("times", "times", (), {}),
        ("datetimes", "datetimes", (dt.datetime(2020, 1, 1),), {"allow_imaginary": False}),
        ("datetimes", "datetimes", (), {}),
        ("timedeltas", "durations", (dt.timedelta(0), dt.timedelta(1)), {}),
        ("timedeltas", "durations", (), {}),
        ("timezones", "zones", (), {"no_cache": True}),
        ("timezones", "zones", (), {}),
    ],
)
def test_deprecated_aliases(old, new, args, kwargs):
    message = rf"gs\.{old}\(\) is deprecated; use gs\.temporal\.{new}\(\) instead"
    with pytest.warns(DeprecationWarning, match=message) as record:
        alias = getattr(gs, old)(*args, **kwargs)
    assert record[0].filename == __file__
    current = getattr(gs.temporal, new)(*args, **kwargs)
    assert repr(alias) == repr(current)
    assert repr(find_example(alias)) == repr(find_example(current))


@pytest.mark.parametrize("old", ["times", "datetimes"])
def test_deprecated_aliases_accept_timezones(old):
    with pytest.warns(DeprecationWarning, match="whose timezones= argument is named zones="):
        alias = getattr(gs, old)(timezones=gs.just(NEW_YORK))
    assert repr(alias) == repr(getattr(gs.temporal, old)(zones=gs.just(NEW_YORK)))
    check_all(alias, lambda value: value.tzinfo is NEW_YORK)


@pytest.mark.parametrize(
    "old, kwargs",
    [
        ("dates", {"weekdays": [0]}),
        ("times", {"precision": "seconds"}),
        ("times", {"zones": gs.just(NEW_YORK)}),
        ("datetimes", {"ambiguous": "earlier"}),
        ("datetimes", {"source": gs.temporal.ZoneSource.bundled()}),
        ("timedeltas", {"precision": "seconds"}),
        ("timezones", {"names": ["UTC"]}),
        ("timezones", {"allow_legacy": True}),
    ],
)
def test_deprecated_aliases_take_only_their_released_arguments(old, kwargs):
    with pytest.raises(TypeError, match="unexpected keyword argument"):
        getattr(gs, old)(**kwargs)


def test_temporal_replaces_the_deprecated_names_in_all():
    assert "temporal" in gs.__all__
    assert {"dates", "datetimes", "timedeltas", "times", "timezones"}.isdisjoint(gs.__all__)


def test_zoned_datetimes():
    check_all(gs.temporal.zoned_datetimes(), lambda d: isinstance(d.tzinfo, zoneinfo.ZoneInfo))
    assert find_example(gs.temporal.zoned_datetimes()) == dt.datetime(
        2000, 1, 1, tzinfo=zoneinfo.ZoneInfo("UTC")
    )
    lo, hi = dt.datetime(2026, 3, 8), dt.datetime(2026, 3, 9)
    check_all(
        gs.temporal.zoned_datetimes(lo, hi, zones=gs.just(NEW_YORK)),
        lambda d: d.tzinfo is NEW_YORK and lo <= d.replace(tzinfo=None) <= hi,
    )
    alo, ahi = dt.datetime(2026, 1, 1, tzinfo=UTC), dt.datetime(2026, 1, 2, tzinfo=UTC)
    check_all(
        gs.temporal.zoned_datetimes(alo, ahi, precision="seconds"),
        lambda d: alo <= d <= ahi and d.microsecond == 0,
    )
    find_example(gs.temporal.zoned_datetimes(lo, hi, zones=gs.just(NEW_YORK)), _is_imaginary)
    assert repr(gs.temporal.zoned_datetimes(lo, hi, zones=gs.just(LONDON))) == (
        f"temporal.zoned_datetimes(min_value={lo!r}, max_value={hi!r}, zones=just({LONDON!r}))"
    )
    assert repr(gs.temporal.zoned_datetimes()) == "temporal.zoned_datetimes()"
    assert repr(gs.temporal.zoned_datetimes(zones=gs.temporal.zones())) == (
        "temporal.zoned_datetimes(zones=temporal.zones())"
    )
    with pytest.raises(hegel.InvalidArgument, match="both naive or both aware"):
        gs.temporal.zoned_datetimes(lo, ahi)

    def naive_zone(tc):
        tc.draw(gs.temporal.zoned_datetimes(lo, hi, zones=gs.none()))

    message = str(failing(naive_zone, hegel.InvalidArgument))
    assert "from zoned_datetimes() every datetime needs a tzinfo" in message


def test_instants():
    check_all(gs.temporal.instants(), lambda d: d.tzinfo is UTC)
    assert find_example(gs.temporal.instants()) == dt.datetime(2000, 1, 1, tzinfo=UTC)
    assert find_example(gs.temporal.instants(), lambda d: d < EPOCH) == EPOCH - dt.timedelta(
        microseconds=1
    )
    lo = dt.datetime(2026, 3, 8, 1, tzinfo=NEW_YORK)
    hi = dt.datetime(2026, 3, 8, 12, tzinfo=LONDON)
    check_all(gs.temporal.instants(lo, hi), lambda d: lo <= d <= hi and d.tzinfo is UTC)
    check_all(
        gs.temporal.instants(lo, hi, precision="hours"),
        lambda d: d.minute == d.second == d.microsecond == 0,
    )
    tokyo = zoneinfo.ZoneInfo("Asia/Tokyo")
    early = dt.datetime.min.replace(tzinfo=tokyo)
    check_all(
        gs.temporal.instants(early, EPOCH), lambda d: dt.datetime.min.replace(tzinfo=UTC) <= d
    )
    assert repr(gs.temporal.instants(lo, hi)) == (
        f"temporal.instants(min_value={lo!r}, max_value={hi!r})"
    )
    assert repr(gs.temporal.instants()) == "temporal.instants()"
    check_all(gs.temporal.instants(None, hi), lambda d: d <= hi)
    check_all(gs.temporal.instants(lo, None), lambda d: d >= lo)
    assert repr(gs.temporal.instants(lo, hi, precision="seconds")) == (
        f"temporal.instants(min_value={lo!r}, max_value={hi!r}, precision='seconds')"
    )
    with pytest.raises(hegel.InvalidArgument, match="must be a datetime"):
        gs.temporal.instants(dt.date(2000, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="must be aware"):
        gs.temporal.instants(max_value=dt.datetime(2000, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="moments in time"):
        gs.temporal.instants(hi, lo)
    with pytest.raises(hegel.InvalidArgument, match="no value at precision hours"):
        gs.temporal.instants(
            EPOCH + dt.timedelta(minutes=1), EPOCH + dt.timedelta(minutes=2), precision="hours"
        )
    with pytest.raises(hegel.InvalidArgument, match="the range a UTC datetime represents"):
        gs.temporal.instants(early, early)


def test_span_printing():
    assert str(gs.temporal.Span()) == "0s"
    assert repr(gs.temporal.Span()) == "Span()"
    assert str(gs.temporal.Span(years=1, months=2)) == "1y 2mo"
    assert str(gs.temporal.Span(days=-3, hours=-4)) == "-3d 4h"
    assert str(gs.temporal.Span(days=-3, hours=4)) == "-3d +4h"
    assert str(gs.temporal.Span(weeks=1, microseconds=-5, nanoseconds=6)) == "+1w -5µs +6ns"
    assert str(gs.temporal.Span(minutes=7, seconds=8, milliseconds=9)) == "7m 8s 9ms"
    assert str(gs.temporal.Span(years=-(2**63), nanoseconds=2**63 - 1)) == (
        "-9223372036854775808y +9223372036854775807ns"
    )
    assert repr(gs.temporal.Span(days=-3, hours=4)) == "Span(days=-3, hours=4)"
    with pytest.raises(hegel.InvalidArgument, match="days=1.5 must be an int"):
        gs.temporal.Span(days=1.5)
    with pytest.raises(hegel.InvalidArgument, match="years=9223372036854775808 must be an int"):
        gs.temporal.Span(years=2**63)

    def body(tc):
        span = tc.draw(gs.temporal.spans(sign="mixed"))
        units = [value for value in vars(span).values() if value]
        signs = {value > 0 for value in units}
        text = str(span)
        assert len(text.split()) == max(len(units), 1)
        assert text.count("+") == (sum(value > 0 for value in units) if len(signs) == 2 else 0)
        assert text.count("-") == (
            sum(value < 0 for value in units) if len(signs) == 2 else int(signs == {False})
        )

    run_test(body, test_cases=100)


def test_spans():
    check_all(gs.temporal.spans(), lambda s: isinstance(s, gs.temporal.Span))
    assert find_example(gs.temporal.spans()) == gs.temporal.Span()
    assert find_example(gs.temporal.spans(), lambda s: s.months < 0) == gs.temporal.Span(months=-1)
    uniform = gs.temporal.spans()
    check_all(
        uniform,
        lambda s: all(v >= 0 for v in vars(s).values()) or all(v <= 0 for v in vars(s).values()),
    )
    find_example(gs.temporal.spans(sign="mixed"), lambda s: s.years > 0 and s.days < 0)
    check_all(
        gs.temporal.spans(sign="non_negative"), lambda s: all(v >= 0 for v in vars(s).values())
    )
    check_all(
        gs.temporal.spans(weeks=(0, 2), sign="non_negative"),
        lambda s: s == gs.temporal.Span(weeks=s.weeks) and 0 <= s.weeks <= 2,
    )
    days_and_hours = gs.temporal.spans(days=(0, 30), hours=(0, 23))
    check_all(
        days_and_hours,
        lambda s: (
            0 <= s.days <= 30
            and 0 <= s.hours <= 23
            and s == gs.temporal.Span(days=s.days, hours=s.hours)
        ),
    )
    negative = gs.temporal.spans(days=(-30, -1), hours=(-23, 0))
    check_all(negative, lambda s: -30 <= s.days <= -1 and -23 <= s.hours <= 0)
    assert repr(gs.temporal.spans()) == "temporal.spans()"
    assert repr(days_and_hours) == "temporal.spans(days=(0, 30), hours=(0, 23))"
    assert repr(gs.temporal.spans(years=(-1, 1), sign="mixed")) == (
        "temporal.spans(years=(-1, 1), sign='mixed')"
    )
    assert repr(gs.from_type(gs.temporal.Span)) == "temporal.spans()"


@pytest.mark.parametrize(
    "kwargs, message",
    [
        ({"days": [0, 1]}, "must be a \\(min, max\\) tuple of ints"),
        ({"days": (0, 1, 2)}, "must be a \\(min, max\\) tuple of ints"),
        ({"days": (0, True)}, "must be a \\(min, max\\) tuple of ints"),
        ({"days": (0, 1.5)}, "must be a \\(min, max\\) tuple of ints"),
        ({"days": (2, 1)}, "min_value <= max_value is required for days"),
        ({"days": (0, 2**63)}, "must be within the 64-bit signed range"),
        ({"days": (-(2**63) - 1, 0)}, "must be within the 64-bit signed range"),
        ({"days": (-1, 1), "sign": "non_negative"}, "requires min >= 0 for days"),
        ({"years": (1, 2), "days": (-2, -1)}, "needs every unit to admit one sign"),
        ({"sign": "positive"}, "sign='positive' must be one of"),
    ],
)
def test_spans_validation(kwargs, message):
    with pytest.raises(hegel.InvalidArgument, match=message):
        gs.temporal.spans(**kwargs)
