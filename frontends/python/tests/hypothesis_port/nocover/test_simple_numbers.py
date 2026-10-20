# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import math
import sys

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import minimal


def test_minimize_negative_int():
    assert minimal(gs.integers(), lambda x: x < 0) == -1
    assert minimal(gs.integers(), lambda x: x < -1) == -2


def test_positive_negative_int():
    assert minimal(gs.integers(), lambda x: x > 0) == 1
    assert minimal(gs.integers(), lambda x: x > 1) == 2


boundaries = pytest.mark.parametrize(
    "boundary",
    sorted(
        [2**i for i in range(10)]
        + [2**i - 1 for i in range(10)]
        + [2**i + 1 for i in range(10)]
        + [10**i for i in range(6)]
    ),
)


@boundaries
def test_minimizes_int_down_to_boundary(boundary):
    assert minimal(gs.integers(), lambda x: x >= boundary) == boundary


@boundaries
def test_minimizes_int_up_to_boundary(boundary):
    assert minimal(gs.integers(), lambda x: x <= -boundary) == -boundary


@boundaries
def test_minimizes_ints_from_down_to_boundary(boundary):
    def is_good(x):
        assert x >= boundary - 10
        return x >= boundary

    assert minimal(gs.integers(min_value=boundary - 10), is_good) == boundary

    assert minimal(gs.integers(min_value=boundary)) == boundary


def test_minimizes_negative_integer_range_upwards():
    assert minimal(gs.integers(min_value=-10, max_value=-1)) == -1


@boundaries
def test_minimizes_integer_range_to_boundary(boundary):
    assert minimal(gs.integers(boundary, boundary + 100)) == boundary


def test_single_integer_range_is_range():
    assert minimal(gs.integers(1, 1)) == 1


def test_minimal_small_number_in_large_range():
    assert minimal(gs.integers((-(2**32)), 2**32), lambda x: x >= 101) == 101


def test_minimal_small_sum_float_list():
    xs = minimal(gs.lists(gs.floats(), min_size=5), lambda x: sum(x) >= 1.0)
    assert xs == [0.0, 0.0, 0.0, 0.0, 1.0]


def test_minimals_boundary_floats():
    def f(x):
        print(x)
        return True

    assert minimal(gs.floats(min_value=-1, max_value=1), f) == 0


def test_minimal_non_boundary_float():
    x = minimal(gs.floats(min_value=1, max_value=9), lambda x: x > 2)
    assert x == 3  # (the smallest integer > 2)


def test_minimal_float_is_zero():
    assert minimal(gs.floats()) == 0.0


def test_minimal_asymetric_bounded_float():
    assert minimal(gs.floats(min_value=1.1, max_value=1.6)) == 1.5


def test_negative_floats_simplify_to_zero():
    assert minimal(gs.floats(), lambda x: x <= -1.0) == -1.0


def test_minimal_infinite_float_is_positive():
    assert minimal(gs.floats(), math.isinf) == math.inf


def test_can_minimal_infinite_negative_float():
    assert minimal(gs.floats(), lambda x: x < -sys.float_info.max)


def test_can_minimal_float_on_boundary_of_representable():
    minimal(gs.floats(), lambda x: x + 1 == x and not math.isinf(x))


def test_minimize_nan():
    assert math.isnan(minimal(gs.floats(), math.isnan))


def test_minimize_very_large_float():
    t = sys.float_info.max / 2
    assert minimal(gs.floats(), lambda x: x >= t) == t


def is_integral(value):
    try:
        return int(value) == value
    except (OverflowError, ValueError):
        return False


def test_can_minimal_float_far_from_integral():
    minimal(gs.floats(), lambda x: math.isfinite(x) and not is_integral(x * (2**32)))


def test_list_of_fractional_float():
    assert set(
        minimal(
            gs.lists(gs.floats(), min_size=5),
            lambda x: len([t for t in x if t >= 1.5]) >= 5,
        )
    ) == {2}


def test_minimal_fractional_float():
    assert minimal(gs.floats(), lambda x: x >= 1.5) == 2


def test_minimizes_lists_of_negative_ints_up_to_boundary():
    result = minimal(
        gs.lists(gs.integers(), min_size=10),
        lambda x: len([t for t in x if t <= -1]) >= 10,
    )
    assert result == [-1] * 10


@pytest.mark.parametrize(
    ("left", "right"),
    [(0.0, 5e-324), (-5e-324, 0.0), (-5e-324, 5e-324), (5e-324, 1e-323)],
)
def test_floats_in_constrained_range(left, right):
    @hegel.test
    def test_in_range(tc):
        r = tc.draw(gs.floats(left, right))
        assert left <= r <= right

    test_in_range()


def test_bounds_are_respected():
    assert minimal(gs.floats(min_value=1.0)) == 1.0
    assert minimal(gs.floats(max_value=-1.0)) == -1.0


@pytest.mark.parametrize("k", range(10))
def test_floats_from_zero_have_reasonable_range(k):
    n = 10**k
    assert minimal(gs.floats(min_value=0.0), lambda x: x >= n) == float(n)
    assert minimal(gs.floats(max_value=0.0), lambda x: x <= -n) == float(-n)


def test_explicit_allow_nan():
    minimal(gs.floats(allow_nan=True), math.isnan)


def test_one_sided_contains_infinity():
    minimal(gs.floats(min_value=1.0), math.isinf)
    minimal(gs.floats(max_value=1.0), math.isinf)


@hegel.test
def test_no_allow_infinity_upper(tc):
    x = tc.draw(gs.floats(min_value=0.0, allow_infinity=False))
    assert not math.isinf(x)


@hegel.test
def test_no_allow_infinity_lower(tc):
    x = tc.draw(gs.floats(max_value=0.0, allow_infinity=False))
    assert not math.isinf(x)


class TestFloatsAreFloats:
    @hegel.test
    def test_unbounded(self, tc):
        arg = tc.draw(gs.floats())
        assert isinstance(arg, float)

    @hegel.test
    def test_int_float(self, tc):
        arg = tc.draw(gs.floats(min_value=0, max_value=float(2**64 - 1)))
        assert isinstance(arg, float)

    @hegel.test
    def test_float_float(self, tc):
        arg = tc.draw(gs.floats(min_value=float(0), max_value=float(2**64 - 1)))
        assert isinstance(arg, float)
