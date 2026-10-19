# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

"""Tests for being able to generate weird and wonderful floating point numbers."""

import math
import struct
import sys

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import find_any
from tests.hypothesis_port.common.utils import fails


def float_to_int(value):
    return struct.unpack("!Q", struct.pack("!d", value))[0]


TRY_HARDER = hegel.Settings(
    test_cases=1000, suppress_health_check={hegel.HealthCheck.filter_too_much}
)


@hegel.test(settings=TRY_HARDER)
def test_is_float(tc):
    x = tc.draw(gs.floats())
    assert isinstance(x, float)


@fails
@hegel.test(settings=TRY_HARDER)
def test_inversion_is_imperfect(tc):
    x = tc.draw(gs.floats())
    tc.assume(x != 0.0)
    y = 1.0 / x
    assert x * y == 1.0


@hegel.test
def test_largest_range(tc):
    x = tc.draw(gs.floats(-sys.float_info.max, sys.float_info.max))
    assert not math.isinf(x)


@hegel.test(settings=TRY_HARDER)
def test_negation_is_self_inverse(tc):
    x = tc.draw(gs.floats())
    tc.assume(not math.isnan(x))
    y = -x
    assert -y == x


@fails
@hegel.test(settings=TRY_HARDER)
def test_is_not_nan(tc):
    xs = tc.draw(gs.lists(gs.floats()))
    assert not any(math.isnan(x) for x in xs)


@fails
@hegel.test(settings=TRY_HARDER)
def test_is_not_positive_infinite(tc):
    x = tc.draw(gs.floats())
    tc.assume(x > 0)
    assert not math.isinf(x)


@fails
@hegel.test(settings=TRY_HARDER)
def test_is_not_negative_infinite(tc):
    x = tc.draw(gs.floats())
    tc.assume(x < 0)
    assert not math.isinf(x)


@fails
@hegel.test(settings=TRY_HARDER)
def test_is_int(tc):
    x = tc.draw(gs.floats())
    tc.assume(math.isfinite(x))
    assert x == int(x)


@fails
@hegel.test(settings=TRY_HARDER)
def test_is_not_int(tc):
    x = tc.draw(gs.floats())
    tc.assume(math.isfinite(x))
    assert x != int(x)


@fails
@hegel.test(settings=TRY_HARDER)
def test_is_in_exact_int_range(tc):
    x = tc.draw(gs.floats())
    tc.assume(math.isfinite(x))
    assert x + 1 != x


@fails
@hegel.test(settings=TRY_HARDER)
def test_can_find_floats_that_do_not_round_trip_through_strings(tc):
    x = tc.draw(gs.floats())
    assert float(str(x)) == x


@fails
@hegel.test(settings=TRY_HARDER)
def test_can_find_floats_that_do_not_round_trip_through_reprs(tc):
    x = tc.draw(gs.floats())
    assert float(repr(x)) == x


finite_floats = gs.floats(allow_infinity=False, allow_nan=False)


@hegel.test
def test_floats_are_in_range(tc):
    x = tc.draw(finite_floats)
    y = tc.draw(finite_floats)
    x, y = sorted((x, y))
    tc.assume(x < y)

    t = tc.draw(gs.floats(x, y))
    assert x <= t <= y


@pytest.mark.parametrize("neg", [False, True])
@pytest.mark.parametrize("snan", [False, True])
def test_can_find_negative_and_signaling_nans(neg, snan):
    find_any(
        gs.floats().filter(math.isnan),
        lambda x: (
            snan is (float_to_int(abs(x)) != float_to_int(float("nan")))
            and neg is (math.copysign(1, x) == -1)
        ),
        settings=TRY_HARDER,
    )
