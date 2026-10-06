# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import time

import pytest
from pytest import raises

from hegel.hypothesis import HealthCheck, Phase, given, seed, settings, strategies as st
from hegel.hypothesis.errors import FailedHealthCheck

pytestmark = pytest.mark.skipif(
    settings.get_current_profile_name() == "crosshair",
    reason="slow - large number of symbolics",
)

large_strategy = st.binary(min_size=7000, max_size=7000)
too_large_strategy = st.tuples(large_strategy, large_strategy)


@pytest.mark.xfail(
    strict=True, reason="hegel has no data_too_large / large_base_example health checks"
)
def test_large_data_will_fail_a_health_check():
    @given(st.none() | too_large_strategy)
    @settings(database=None)
    def test(x):
        pass

    with raises(FailedHealthCheck) as e:
        test()
    assert "maximum allowed entropy" in e.value.args[0]


@pytest.mark.xfail(
    strict=True, reason="hegel has no data_too_large / large_base_example health checks"
)
def test_large_base_example_fails_health_check():
    @given(large_strategy)
    def test(b):
        pass

    with pytest.raises(FailedHealthCheck) as exc:
        test()

    assert str(HealthCheck.large_base_example) in str(exc.value)


@pytest.mark.xfail(
    strict=True, reason="hegel has no data_too_large / large_base_example health checks"
)
def test_example_that_shrinks_to_overrun_fails_health_check():
    @given(too_large_strategy | st.none())
    def test(b):
        pass

    with pytest.raises(FailedHealthCheck) as exc:
        test()

    assert str(HealthCheck.large_base_example) in str(exc.value)


slow_down_init = True


def slow_init_integers(*args, **kwargs):
    # This mimics st.characters() or st.text(), which perform some
    # expensive Unicode calculations when the cache is empty.
    global slow_down_init
    if slow_down_init:
        time.sleep(0.5)  # We monkeypatch time, so this is fast
        slow_down_init = False
    return st.integers(*args, **kwargs)


# Dropped test_lazy_slow_initialization_issue_2108_regression: LazyStrategy is internal


# Dropped test_does_not_trigger_health_check_on_simple_strategies: patches ConjectureData internals


def test_does_not_trigger_health_check_when_most_examples_are_small():
    for i in range(10):

        @seed(i)
        # Setting max_examples=11 ensures we have enough examples for the
        # health checks to finish running, but cuts the generation short
        # after that point to allow this test to run in reasonable time.
        @settings(database=None, max_examples=11, phases=[Phase.generate])
        @given(
            st.integers(0, 100).flatmap(
                lambda n: st.binary(min_size=min(n * 100, 8 * 1024), max_size=n * 100)
            )
        )
        def test(b):
            pass

        test()
