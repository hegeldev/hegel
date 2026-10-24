# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original:
# test_tricky_draw_with_unusable_tzinfo_falls_back,
# test_tricky_path_is_skipped_when_provably_fruitless,
# test_fixed_offset_timezones_has_no_interesting_instants,
# test_early_transition_timezone_interesting_instants,
# test_interesting_instants_near_datetime_min,
# test_tricky_draw_rejects_out_of_bounds_fold (DatetimeStrategy internals:
# ConjectureData, _interesting_instants, tricky_possible);
# test_tricky_draws_near_datetime_max_stay_in_bounds,
# test_aware_bounds_are_compared_as_moments_in_time,
# test_aware_bounds_with_none_from_timezones_strategy_is_invalid,
# test_datetimes_stay_within_aware_bounds, test_single_aware_bound_min,
# test_single_aware_bound_max, test_shrinks_to_aware_min_bound,
# test_bounds_unrepresentable_in_timezone_are_rejected,
# test_extreme_aware_bounds_are_clamped_per_timezone,
# test_pathological_timezone_may_leave_no_valid_fold,
# test_pathological_timezone_inverted_bounds_draw_via_utc,
# test_pathological_timezone_bounds_unrepresentable_in_utc,
# test_pathological_timezone_unrepresentable_moments_are_rejected,
# test_pathological_timezone_single_bound_near_extreme (timezone-aware
# bounds); test_can_generate_datetime_with_fold_1,
# test_can_generate_time_with_fold_1 (fold=1 generation);
# test_generates_unix_rollover_adjacent_times (upweighting the 2038 rollover).

import datetime as dt
from calendar import monthrange

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument
from hegel.generators import dates, datetimes, timedeltas, times
from tests.hypothesis_port.common.compat import _num_days_in_month
from tests.hypothesis_port.common.debug import (
    assert_all_examples,
    find_any,
    minimal,
)

UTC = dt.UTC


@hegel.test
@hegel.explicit_test_case(1900)
@hegel.explicit_test_case(2000)
@hegel.explicit_test_case(2004)
@hegel.explicit_test_case(2100)
def test_num_days_in_month_matches_monthrange(tc):
    year = tc.draw(gs.integers(dt.MINYEAR, dt.MAXYEAR))
    for month in range(1, 13):
        assert _num_days_in_month(year, month) == monthrange(year, month)[1]


def test_can_find_positive_delta():
    assert minimal(timedeltas(), lambda x: x.days > 0) == dt.timedelta(1)


@pytest.mark.xfail(
    reason="Hegel shrinks a timedelta toward zero as a whole, so the simplest negative "
    "delta is a microsecond below zero, where Hypothesis shrinks days, seconds and "
    "microseconds one at a time",
    strict=True,
)
def test_can_find_negative_delta():
    assert minimal(timedeltas(max_value=dt.timedelta(10**6)), lambda x: x.days < 0) == dt.timedelta(
        -1
    )


def test_can_find_on_the_second():
    find_any(timedeltas(), lambda x: x.seconds == 0)


def test_can_find_off_the_second():
    find_any(timedeltas(), lambda x: x.seconds != 0)


def test_simplifies_towards_zero_delta():
    d = minimal(timedeltas())
    assert d.days == d.seconds == d.microseconds == 0


def test_min_value_is_respected():
    assert minimal(timedeltas(min_value=dt.timedelta(days=10))).days == 10


def test_max_value_is_respected():
    assert minimal(timedeltas(max_value=dt.timedelta(days=-10))).days == -10


# Hegel's timedeltas(val, val) draws a timedelta equal to val rather than
# handing back the bound object itself, so this checks equality.
@hegel.test(suppress_health_check=hegel.HealthCheck.all())
def test_single_timedelta(tc):
    val = tc.draw(timedeltas())
    assert_all_examples(timedeltas(val, val), lambda v: v == val, tc=tc)


def test_simplifies_towards_millenium():
    d = minimal(datetimes())
    assert d.year == 2000
    assert d.month == d.day == 1
    assert d.hour == d.minute == d.second == d.microsecond == 0


@hegel.test
def test_default_datetimes_are_naive(tc):
    d = tc.draw(datetimes())
    assert d.tzinfo is None


def test_bordering_on_a_leap_year():
    x = minimal(
        datetimes(dt.datetime.min.replace(year=2003), dt.datetime.max.replace(year=2005)),
        lambda x: x.month == 2 and x.day == 29,
        settings=hegel.Settings(test_cases=2500),
    )
    assert x.year == 2004


def test_can_find_after_the_year_2000():
    assert minimal(dates(), lambda x: x.year > 2000).year == 2001


def test_can_find_before_the_year_2000():
    assert minimal(dates(), lambda x: x.year < 2000).year == 1999


@pytest.mark.parametrize("month", range(1, 13))
def test_can_find_each_month(month):
    find_any(dates(), lambda x: x.month == month, hegel.Settings(test_cases=10**6))


def test_min_year_is_respected():
    assert minimal(dates(min_value=dt.date.min.replace(2003))).year == 2003


def test_max_year_is_respected():
    assert minimal(dates(max_value=dt.date.min.replace(1998))).year == 1998


# Hegel's dates(val, val) draws a date equal to val rather than handing back
# the bound object itself, so this checks equality.
@hegel.test
def test_single_date(tc):
    val = tc.draw(dates())
    assert tc.draw(dates(val, val)) == val


def test_can_find_midnight():
    find_any(times(), lambda x: x.hour == x.minute == x.second == 0)


def test_can_find_non_midnight():
    assert minimal(times(), lambda x: x.hour != 0).hour == 1


def test_can_find_on_the_minute():
    find_any(times(), lambda x: x.second == 0)


def test_can_find_off_the_minute():
    find_any(times(), lambda x: x.second != 0)


def test_simplifies_towards_midnight():
    d = minimal(times())
    assert d.hour == d.minute == d.second == d.microsecond == 0


def test_can_generate_naive_time():
    find_any(times(), lambda d: not d.tzinfo)


@hegel.test
def test_naive_times_are_naive(tc):
    d = tc.draw(times())
    assert d.tzinfo is None


@hegel.test
def test_allow_imaginary_is_not_an_error_for_naive_datetimes(tc):
    tc.draw(datetimes(allow_imaginary=False))


def test_generates_leap_second_adjacent_times():
    # we upweight instants near leap seconds
    find_any(datetimes(), lambda x: x.time() >= dt.time(23, 59, 58, 999999))


def test_tricky_datetimes_respect_bounds():
    lo, hi = dt.datetime(2016, 12, 31, 23), dt.datetime(2017, 1, 1, 1)
    assert_all_examples(datetimes(lo, hi), lambda x: lo <= x <= hi)


class _DatetimeSubclass(dt.datetime):
    pass


def test_tricky_draws_with_datetime_subclass_bounds():
    # Bounds like pandas.Timestamp - a datetime subclass with a narrower
    # representable range - must not leak into the tricky window computation.
    lo = _DatetimeSubclass(2016, 12, 31, 12)
    hi = _DatetimeSubclass(2017, 1, 2, 12)
    assert_all_examples(datetimes(lo, hi), lambda x: lo <= x <= hi and type(x) is dt.datetime)


def test_tricky_windows_clamp_at_either_bound():
    # Bounds closer to the 2017 leap second than the widest window, on each
    # side in turn, so both clamping branches are always exercised.
    leap = dt.datetime(2017, 1, 1)
    for lo, hi in [
        (leap - dt.timedelta(hours=12), leap + dt.timedelta(days=7)),
        (leap - dt.timedelta(days=7), leap + dt.timedelta(hours=12)),
    ]:
        assert_all_examples(datetimes(lo, hi), lambda x, lo=lo, hi=hi: lo <= x <= hi)


@pytest.mark.parametrize("aware_arg", ["min_value", "max_value"])
def test_mixing_aware_and_naive_bounds_is_invalid(aware_arg):
    kwargs = {
        "min_value": dt.datetime(2000, 1, 1),
        "max_value": dt.datetime(2001, 1, 1),
    }
    kwargs[aware_arg] = kwargs[aware_arg].replace(tzinfo=UTC)
    with pytest.raises(InvalidArgument):
        datetimes(**kwargs)
