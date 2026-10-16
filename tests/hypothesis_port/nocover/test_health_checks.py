# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_lazy_slow_initialization_issue_2108_regression
# (LazyStrategy internals); test_does_not_trigger_health_check_on_simple_strategies
# (patches ConjectureData internals).

import pytest
from pytest import raises

import hegel
from hegel import FailedHealthCheck, Phase
from hegel import generators as gs

# Hegel counts choices, not bytes: a binary() draw is one choice however
# long it is, so a large test case is a long list. A test case may make
# 2**20 choices before it is cut short (test_cases_too_large), and the
# initial test case is deemed large at about half that
# (large_initial_test_case).
LARGE = 600_000
TOO_LARGE = 2**20 + 10
large_strategy = gs.lists(gs.booleans(), min_size=LARGE, max_size=LARGE)
too_large_strategy = gs.lists(gs.booleans(), min_size=TOO_LARGE, max_size=TOO_LARGE)


@pytest.mark.slow
def test_large_data_will_fail_a_health_check():
    # Only one input in a hundred is small, so the run sees twenty oversized
    # test cases before it sees ten valid ones. Each of those is a million
    # draws through the binding, a couple of seconds, so the slowness check
    # (thirty seconds) is off to let the size check be the one that fires.
    @hegel.test(database="", suppress_health_check=[hegel.HealthCheck.too_slow])
    def test(tc):
        n = tc.draw(gs.integers(0, 99))
        tc.draw(gs.none() if n == 0 else too_large_strategy)

    with raises(FailedHealthCheck) as e:
        test()
    assert "TestCasesTooLarge" in e.value.args[0]


def test_large_base_example_fails_health_check():
    @hegel.test
    def test(tc):
        tc.draw(large_strategy)

    with pytest.raises(FailedHealthCheck) as exc:
        test()

    assert "LargeInitialTestCase" in str(exc.value)


def test_example_that_shrinks_to_overrun_fails_health_check():
    @hegel.test
    def test(tc):
        tc.draw(too_large_strategy | gs.none())

    with pytest.raises(FailedHealthCheck) as exc:
        test()

    assert "LargeInitialTestCase" in str(exc.value)


def test_does_not_trigger_health_check_when_most_examples_are_small():
    for i in range(10):
        # Setting test_cases=11 ensures we have enough examples for the
        # health checks to finish running, but cuts the generation short
        # after that point to allow this test to run in reasonable time.
        @hegel.test(database="", test_cases=11, phases=[Phase.generate], seed=i)
        def test(tc):
            tc.draw(
                gs.integers(0, 100).flat_map(
                    lambda n: gs.binary(min_size=min(n * 100, 8 * 1024), max_size=n * 100)
                )
            )

        test()
