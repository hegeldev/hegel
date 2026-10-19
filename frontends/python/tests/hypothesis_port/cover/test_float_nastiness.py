# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_float16_can_exclude_infinity, the
# width=16 cases of test_out_of_range and test_fuzzing_floats_bounds (floats(width=16)).

import math
import sys

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.compat import (
    float_of,
    float_to_int,
    int_to_float,
    is_negative,
    next_down,
    next_up,
)
from tests.hypothesis_port.common.debug import find_any, minimal


@pytest.mark.parametrize(
    ("lower", "upper"),
    [
        # Exact values don't matter, but they're large enough so that x + y = inf.
        (9.9792015476736e291, 1.7976931348623157e308),
        (-sys.float_info.max, sys.float_info.max),
    ],
)
@hegel.test
def test_floats_are_in_range(tc, lower, upper):
    t = tc.draw(gs.floats(lower, upper))
    assert lower <= t <= upper


@pytest.mark.parametrize("sign", [-1, 1])
def test_can_generate_both_zeros(sign):
    assert minimal(gs.floats(), lambda x: math.copysign(1, x) == sign) == sign * 0.0


@pytest.mark.parametrize(
    ("l", "r"),
    [(-1.0, 1.0), (-0.0, 1.0), (-1.0, 0.0), (-sys.float_info.min, sys.float_info.min)],
)
@pytest.mark.parametrize("sign", [-1, 1])
def test_can_generate_both_zeros_when_in_interval(l, r, sign):
    assert minimal(gs.floats(l, r), lambda x: math.copysign(1, x) == sign) == sign * 0.0


@hegel.test
def test_does_not_generate_negative_if_right_boundary_is_positive(tc):
    x = tc.draw(gs.floats(0.0, 1.0))
    assert math.copysign(1, x) == 1


@hegel.test
def test_does_not_generate_positive_if_right_boundary_is_negative(tc):
    x = tc.draw(gs.floats(-1.0, -0.0))
    assert math.copysign(1, x) == -1


def test_half_bounded_generates_zero():
    find_any(gs.floats(min_value=-1.0), lambda x: x == 0.0)
    find_any(gs.floats(max_value=1.0), lambda x: x == 0.0)


@hegel.test
def test_half_bounded_respects_sign_of_upper_bound(tc):
    x = tc.draw(gs.floats(max_value=-0.0))
    assert math.copysign(1, x) == -1


@hegel.test
def test_half_bounded_respects_sign_of_lower_bound(tc):
    x = tc.draw(gs.floats(min_value=0.0))
    assert math.copysign(1, x) == 1


@hegel.test
def test_filter_nan(tc):
    x = tc.draw(gs.floats(allow_nan=False))
    assert not math.isnan(x)


@hegel.test
def test_filter_infinity(tc):
    x = tc.draw(gs.floats(allow_infinity=False))
    assert not math.isinf(x)


def test_can_guard_against_draws_of_nan():
    """In this test we create a NaN value that naturally "tries" to shrink into
    the first generator, where it is not permitted. This tests a case that is
    very unlikely to happen in random generation: When the unconstrained first
    branch of generating a float just happens to produce a NaN value.

    Here what happens is that we get a NaN from the *second* generator,
    but this then shrinks into its unconstrained branch. The natural
    thing to happen is then to try to zero the branch parameter of the
    one_of, but that will put an illegal value there, so it's not
    allowed to happen.
    """
    tagged_floats = gs.one_of(
        gs.tuples(gs.just(0), gs.floats(allow_nan=False)),
        gs.tuples(gs.just(1), gs.floats(allow_nan=True)),
    )

    tag, _f = minimal(tagged_floats, lambda x: math.isnan(x[1]))
    assert tag == 1


def test_very_narrow_interval():
    upper_bound = -1.0
    lower_bound = int_to_float(float_to_int(upper_bound) + 10)
    assert lower_bound < upper_bound

    @hegel.test
    def test(tc):
        f = tc.draw(gs.floats(lower_bound, upper_bound))
        assert lower_bound <= f <= upper_bound

    test()


@hegel.test
def test_up_means_greater(tc):
    x = tc.draw(gs.floats())
    hi = next_up(x)
    if not x < hi:
        assert (
            (math.isnan(x) and math.isnan(hi))
            or (x > 0 and math.isinf(x))
            or (x == hi == 0 and is_negative(x) and not is_negative(hi))
        )


@hegel.test
def test_down_means_lesser(tc):
    x = tc.draw(gs.floats())
    lo = next_down(x)
    if not x > lo:
        assert (
            (math.isnan(x) and math.isnan(lo))
            or (x < 0 and math.isinf(x))
            or (x == lo == 0 and is_negative(lo) and not is_negative(x))
        )


@hegel.test
def test_updown_roundtrip(tc):
    val = tc.draw(gs.floats(allow_nan=False, allow_infinity=False))
    assert val == next_up(next_down(val))
    assert val == next_down(next_up(val))


@hegel.test
def test_float32_can_exclude_infinity(tc):
    x = tc.draw(gs.floats(width=32, allow_infinity=False))
    assert not math.isinf(x)


@pytest.mark.parametrize(
    "kwargs",
    [
        {"min_value": 10**40, "width": 32},
        {"max_value": 10**40, "width": 32},
        {"min_value": 10**400, "width": 64},
        {"max_value": 10**400, "width": 64},
        {"min_value": 10**400},
        {"max_value": 10**400},
    ],
)
def test_out_of_range(kwargs):
    with pytest.raises(OverflowError):
        gs.floats(**kwargs)


def test_disallowed_width():
    with pytest.raises(hegel.InvalidArgument):
        gs.floats(width=128)


def test_no_single_floats_in_range():
    low = 2.0**25 + 1
    high = low + 2
    gs.floats(low, high)  # Note: OK for 64bit floats
    with pytest.raises(hegel.InvalidArgument):
        gs.floats(low, high, width=32)


# If the floats() generator adds random floats to a value as large as 10^304
# without handling overflow, we are very likely to generate infinity.
@hegel.test
def test_finite_min_bound_does_not_overflow(tc):
    x = tc.draw(gs.floats(min_value=1e304, allow_infinity=False))
    assert not math.isinf(x)


@hegel.test
def test_finite_max_bound_does_not_overflow(tc):
    x = tc.draw(gs.floats(max_value=-1e304, allow_infinity=False))
    assert not math.isinf(x)


@hegel.test
def test_can_exclude_endpoints(tc):
    x = tc.draw(gs.floats(0, 1, exclude_min=True, exclude_max=True))
    assert 0 < x < 1


@hegel.test
def test_can_exclude_neg_infinite_endpoint(tc):
    x = tc.draw(gs.floats(-math.inf, -1e307, exclude_min=True))
    assert not math.isinf(x)


@hegel.test
def test_can_exclude_pos_infinite_endpoint(tc):
    x = tc.draw(gs.floats(1e307, math.inf, exclude_max=True))
    assert not math.isinf(x)


def test_exclude_infinite_endpoint_is_invalid():
    with pytest.raises(hegel.InvalidArgument):
        gs.floats(min_value=math.inf, exclude_min=True)
    with pytest.raises(hegel.InvalidArgument):
        gs.floats(max_value=-math.inf, exclude_max=True)


@pytest.mark.parametrize("lo,hi", [(True, False), (False, True), (True, True)])
@hegel.test
def test_exclude_entire_interval(tc, lo, hi):
    bound = tc.draw(gs.floats(allow_nan=False, allow_infinity=False).filter(bool))
    with pytest.raises(hegel.InvalidArgument, match=r"exclude_min/exclude_max leave no"):
        gs.floats(bound, bound, exclude_min=lo, exclude_max=hi)


def test_zero_intervals_are_OK():
    gs.floats(0.0, 0.0)
    gs.floats(-0.0, 0.0)
    gs.floats(-0.0, -0.0)


@pytest.mark.parametrize("lo", [0.0, -0.0])
@pytest.mark.parametrize("hi", [0.0, -0.0])
@pytest.mark.parametrize("exmin,exmax", [(True, False), (False, True), (True, True)])
def test_cannot_exclude_endpoint_with_zero_interval(lo, hi, exmin, exmax):
    with pytest.raises(hegel.InvalidArgument):
        gs.floats(lo, hi, exclude_min=exmin, exclude_max=exmax)


WIDTHS = (64, 32)


@pytest.mark.parametrize("width", WIDTHS)
@pytest.mark.parametrize("bound", [math.inf, -math.inf])
def test_interval_of_a_single_infinity(bound, width):
    assert find_any(gs.floats(bound, bound, width=width)) == bound


@pytest.mark.parametrize("nonfloat", [gs.nothing(), gs.none()])
@hegel.test
def test_fuzzing_floats_bounds(tc, nonfloat):
    width = tc.draw(gs.sampled_from(WIDTHS))
    lo = tc.draw(nonfloat | gs.floats(allow_nan=False, width=width), name="lo")
    hi = tc.draw(nonfloat | gs.floats(allow_nan=False, width=width), name="hi")
    if lo is not None and hi is not None and lo > hi:
        lo, hi = hi, lo
    tc.assume(lo != 0 or hi != 0)
    # An interval that is just one infinity breaks the engine; see
    # test_interval_of_a_single_infinity.
    tc.assume(lo is None or lo != hi or not math.isinf(lo))
    value = tc.draw(
        gs.floats(min_value=lo, max_value=hi, width=width, allow_nan=False),
        name="value",
    )
    assert value == float_of(value, width=width)
    assert lo is None or lo <= value
    assert hi is None or value <= hi
