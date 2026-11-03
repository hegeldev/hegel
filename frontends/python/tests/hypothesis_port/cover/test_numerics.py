# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_fuzz_fractions_bounds,
# test_fraction_addition_is_well_behaved (fractions()); test_fuzz_decimals_bounds,
# test_all_decimals_can_be_exact_floats, test_decimals_include_nan,
# test_decimals_include_inf, test_decimals_can_disallow_nan,
# test_decimals_can_disallow_inf, test_decimals_have_correct_places,
# test_works_with_few_values, test_issue_725_regression, test_issue_4651_regression,
# test_issue_739_regression, test_consistent_decimal_error,
# test_minimal_nonfinite_decimal_is_inf, test_decimals_warns_for_inexact_numeric_bounds
# (decimals()).

from math import copysign, inf, isinf

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.compat import next_down, next_up


@hegel.test(suppress_health_check=hegel.HealthCheck.all())
def test_fuzz_floats_bounds(tc):
    width = tc.draw(gs.sampled_from([64, 32]))
    bound = gs.none() | gs.floats(allow_nan=False, width=width)
    low, high = tc.draw(gs.tuples(bound, bound), name="low, high")
    if low is not None and high is not None and low > high:
        low, high = high, low
    if low is not None and high is not None and low > high:
        low, high = high, low
    # An interval that is just one infinity breaks the engine; see
    # test_float_nastiness.py::test_interval_of_a_single_infinity.
    tc.assume(low != high or low is None or not isinf(low))
    exmin = low is not None and low != inf and tc.draw(gs.booleans(), name="ex_min")
    exmax = high is not None and high != -inf and tc.draw(gs.booleans(), name="ex_max")

    if low is not None and high is not None:
        lo = next_up(low, width) if exmin else low
        hi = next_down(high, width) if exmax else high
        # There must actually be floats between these bounds
        tc.assume(lo <= hi)
        if lo == hi == 0:
            tc.assume(not exmin and not exmax and copysign(1.0, lo) <= copysign(1.0, hi))

    kwargs = {"width": width}
    if low is not None:
        kwargs["min_value_exclusive" if exmin else "min_value"] = low
    if high is not None:
        kwargs["max_value_exclusive" if exmax else "max_value"] = high
    s = gs.floats(**kwargs)
    val = tc.draw(s, name="value")
    tc.assume(val)  # positive/negative zero is an issue

    if low is not None:
        assert low <= val
    if high is not None:
        assert val <= high
    if exmin:
        assert low != val
    if exmax:
        assert high != val


@pytest.mark.parametrize(
    "kwargs",
    [
        {"min_value": inf, "allow_infinity": False},
        {"min_value_exclusive": next_down(inf), "allow_infinity": False},
        {"max_value": -inf, "allow_infinity": False},
        {"max_value_exclusive": next_up(-inf), "allow_infinity": False},
    ],
)
def test_floats_message(kwargs):
    # https://github.com/HypothesisWorks/hypothesis/issues/3207
    with pytest.raises(hegel.InvalidArgument, match="allow_infinity=False"):
        gs.floats(**kwargs)
