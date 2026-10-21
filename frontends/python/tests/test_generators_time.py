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
from hegel.generators.time import _is_imaginary, _select_keys
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
    # Bounds whose UTC reading falls outside the type's range are clamped
    # to it.
    behind_lo = dt.datetime.max.replace(tzinfo=dt.timezone(dt.timedelta(hours=-4)))
    behind_hi = dt.datetime.max.replace(tzinfo=dt.timezone(dt.timedelta(hours=-5)))
    assert instant(behind_lo) < instant(behind_hi)
    west = dt.timezone(dt.timedelta(hours=-1))
    check_all(
        gs.datetimes(behind_lo, behind_hi, timezones=gs.just(west)),
        lambda d: d.tzinfo is west and d == last,
    )
    ahead_lo = dt.datetime.min.replace(tzinfo=dt.timezone(dt.timedelta(hours=5)))
    ahead_hi = dt.datetime.min.replace(tzinfo=dt.timezone(dt.timedelta(hours=4)))
    shifted = Shifted()
    check_all(
        gs.datetimes(ahead_lo, ahead_hi, timezones=gs.just(shifted)),
        lambda d: d.tzinfo is shifted and d == dt.datetime.min.replace(tzinfo=UTC),
    )


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
    assert re.fullmatch(r"\d{4}[a-z]", _native.tzdb_version())


def test_timezones():
    check_all(gs.timezones(), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.timezones(no_cache=True), lambda z: z is not zoneinfo.ZoneInfo(z.key))
    check_all(gs.timezones(), lambda z: z is zoneinfo.ZoneInfo(z.key))
    assert find_example(gs.timezones()) is zoneinfo.ZoneInfo("UTC")
    assert repr(gs.timezones()) == "timezones(no_cache=False, links=True, legacy=False)"
    with pytest.raises(hegel.InvalidArgument, match="no_cache=1 must be a bool"):
        gs.timezones(no_cache=1)
    with pytest.raises(hegel.InvalidArgument, match="legacy=1 must be a bool"):
        gs.timezones(legacy=1)
