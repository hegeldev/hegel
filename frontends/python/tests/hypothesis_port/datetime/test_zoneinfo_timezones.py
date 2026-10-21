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
# test_aware_datetimes_filter_rewriting_uses_instant_semantics,
# test_aware_datetimes_filter_rewriting_narrows_arbitrary_timezones (filter
# rewriting); test_probed_transitions_include_known_dst_change,
# test_interesting_instants_beyond_the_scan_range,
# test_probing_finds_transitions_a_week_apart,
# test_probing_a_misaligned_window_still_finds_transitions,
# test_tricky_draw_falls_back_for_utc_frame_bounds (DatetimeStrategy
# internals: transition probing, ConjectureData);
# test_timezone_keys_argument_validation,
# test_can_generate_prefixes_if_allowed_and_available,
# test_can_disallow_prefixes (timezone_keys() has no allow_prefix: the
# posix/ and right/ variants are never generated);
# test_datetimes_stay_within_aware_bounds,
# test_allow_imaginary_is_not_an_error_for_aware_bounds,
# test_bounds_at_either_side_of_a_fold, test_fold_bound_with_long_interval,
# test_bounds_inside_a_fold, test_equal_bounds_at_ambiguous_moment_preserves_fold
# and test_generates_ambiguous_datetimes_in_both_folds are covered by
# tests/test_generators_time.py against Hegel's own semantics.

import datetime as dt
import zoneinfo

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument
from tests.hypothesis_port.common.compat import _instant, datetime_does_not_exist
from tests.hypothesis_port.common.debug import find_any, minimal


def test_utc_is_minimal():
    assert minimal(gs.timezones()) is zoneinfo.ZoneInfo("UTC")


def test_can_generate_non_utc():
    find_any(gs.datetimes(timezones=gs.timezones()).filter(lambda d: d.tzinfo.key != "UTC"))


@hegel.test
def test_datetimes_stay_within_naive_bounds(tc):
    lo = tc.draw(gs.datetimes())
    hi = tc.draw(gs.datetimes())
    if lo > hi:
        lo, hi = hi, lo
    out = tc.draw(gs.datetimes(lo, hi, timezones=gs.timezones()))
    assert lo <= out.replace(tzinfo=None) <= hi


# America/New_York fell back from 02:00 EDT to 01:00 EST on 2020-11-01, and
# Europe/Dublin - which has a negative DST offset - from 02:00 UTC+1 to 01:00 GMT
# on 2020-10-25; in both, wall times between 01:00 and 02:00 are ambiguous.
FALL_BACK_DAYS = [
    ("America/New_York", dt.date(2020, 11, 1)),
    ("Europe/Dublin", dt.date(2020, 10, 25)),
]


def at(day, hour, minute=0, *, key, fold=0):
    tzinfo = zoneinfo.ZoneInfo(key)
    return dt.datetime.combine(day, dt.time(hour, minute, fold=fold), tzinfo=tzinfo)


@pytest.mark.parametrize("key, day", FALL_BACK_DAYS)
def test_fold_inverted_bounds_are_invalid(key, day):
    # fold=1 is the later moment, so these bounds are ordered by wall clock time, but not
    # by UTC (_instant) time. This ordering should be rejected.
    lo = at(day, 1, 45, key=key, fold=1)
    hi = at(day, 1, 50, key=key, fold=0)
    assert lo.replace(tzinfo=None) < hi.replace(tzinfo=None)
    assert _instant(hi) < _instant(lo)
    with pytest.raises(InvalidArgument):
        gs.datetimes(lo, hi, timezones=gs.just(lo.tzinfo))


@pytest.mark.parametrize("kwargs", [{"no_cache": 1}])
def test_timezones_argument_validation(kwargs):
    with pytest.raises(InvalidArgument):
        gs.timezones(**kwargs)


def test_generates_imaginary_datetimes():
    tz = zoneinfo.ZoneInfo("America/New_York")
    find_any(gs.datetimes(timezones=gs.just(tz)), datetime_does_not_exist)
