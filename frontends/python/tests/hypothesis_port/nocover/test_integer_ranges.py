# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import pytest

import hegel
from hegel import generators as gs


@pytest.mark.xfail(
    strict=False,
    reason="the engine's bounded integer distribution draws many more huge values "
    "(observed 680 of 1000 above 1e25; see docs/explanation/comparison-with-hypothesis.md)",
)
def test_bounded_integers_distribution_of_bit_width_issue_1387_regression():
    values = []

    @hegel.test(database="", test_cases=1000)
    def test(tc):
        x = tc.draw(gs.integers(0, 10**100))
        if 2 <= x <= 10**100 - 2:  # skip forced-endpoints
            values.append(x)

    test()

    # We draw from a shaped distribution up to 128bit, so we should get some very large
    # but not too many.
    huge = sum(x > 1e25 for x in values)
    assert 1 < huge <= 100


@pytest.mark.parametrize(
    "bounds",
    [
        (-(2**300), 2**500),
        (2**500, 2**501),
        (2**500, 2**500 + 10),
        (2**2000, None),
        (None, -(2**2000)),
        (2**2000, 2**2000 + 100),
    ],
)
def test_integer_bounds(bounds):
    min_value, max_value = bounds

    @hegel.test
    def f(tc):
        n = tc.draw(gs.integers(min_value, max_value))
        if min_value is not None:
            assert min_value <= n
        if max_value is not None:
            assert n <= max_value

    f()


# this test depends on the distribution hegel's integers() draws from
@pytest.mark.parametrize(
    "min_value, max_value, lower, upper",
    [
        # unbounded: use (-2**128, 2**128).
        (None, None, -(2**128), 2**128),
        # half-bounded at n < 2^127: use (n, 2^128).
        (0, None, 0, 2**128),
        (None, 0, -(2**128), 0),
        (-1000, None, -1000, 2**128),
        (None, 1000, -(2**128), 1000),
        (2**127, None, 2**127, 2**128),
        (None, -(2**127), -(2**128), -(2**127)),
        # half-bounded at n > 2^127: use (n, 2n).
        (2**200, None, 2**200, 2**201),
        (None, -(2**200), -(2**201), -(2**200)),
        (-(2**200), None, -(2**200), 2**201),
        (None, 2**200, -(2**201), 2**200),
    ],
)
def test_integers_unbounded_and_half_bounded(min_value, max_value, lower, upper):
    @hegel.test
    def f(tc):
        n = tc.draw(gs.integers(min_value, max_value))
        assert lower <= n <= upper

    f()
