from __future__ import annotations

import datetime as dt
import zoneinfo

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import failing, find_example, run_test


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


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
    check_all(gs.times(timezones=gs.just(dt.UTC)), lambda t: t.tzinfo is dt.UTC)
    assert find_example(gs.times()) == dt.time(0, 0)
    assert repr(gs.times(lo, hi)) == f"times(min_value={lo!r}, max_value={hi!r})"
    with pytest.raises(hegel.InvalidArgument):
        gs.times(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.times(dt.time(1, tzinfo=dt.UTC))
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
    assert repr(gs.datetimes(lo, hi)) == f"datetimes(min_value={lo!r}, max_value={hi!r})"
    with pytest.raises(hegel.InvalidArgument):
        gs.datetimes(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.datetimes(dt.datetime(2020, 1, 1, tzinfo=dt.UTC))
    with pytest.raises(hegel.InvalidArgument):
        gs.datetimes(dt.date(2020, 1, 1))

    def bad_tz(tc):
        tc.draw(gs.datetimes(timezones=gs.just("UTC")))

    failing(bad_tz, hegel.InvalidArgument)


def test_imaginary_datetimes():
    london = zoneinfo.ZoneInfo("Europe/London")
    gap_start, gap_end = dt.datetime(2020, 3, 29, 1, 0), dt.datetime(2020, 3, 29, 1, 59)
    imaginary = gs.datetimes(gap_start, gap_end, timezones=gs.just(london), allow_imaginary=True)
    check_all(imaginary, lambda d: d.tzinfo is london, test_cases=5)
    real = gs.datetimes(gap_start, gap_end, timezones=gs.just(london), allow_imaginary=False)

    def body(tc):
        tc.draw(real)

    failing(body, hegel.FailedHealthCheck, test_cases=5)
    check_all(
        gs.datetimes(
            dt.datetime(2020, 6, 1),
            dt.datetime(2020, 6, 2),
            timezones=gs.just(london),
            allow_imaginary=False,
        ),
        lambda d: d.tzinfo is london,
    )


def test_timedeltas():
    check_all(gs.timedeltas(), lambda d: isinstance(d, dt.timedelta))
    lo, hi = dt.timedelta(seconds=-1), dt.timedelta(seconds=1)
    check_all(gs.timedeltas(lo, hi), lambda d: lo <= d <= hi)
    assert find_example(gs.timedeltas()) == dt.timedelta(0)
    assert repr(gs.timedeltas(lo, hi)) == f"timedeltas(min_value={lo!r}, max_value={hi!r})"
    with pytest.raises(hegel.InvalidArgument):
        gs.timedeltas(hi, lo)
    with pytest.raises(hegel.InvalidArgument):
        gs.timedeltas(1)


def test_timezones():
    check_all(gs.timezones(), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    check_all(gs.timezones(no_cache=True), lambda z: isinstance(z, zoneinfo.ZoneInfo))
    assert str(find_example(gs.timezones())) == "UTC"
