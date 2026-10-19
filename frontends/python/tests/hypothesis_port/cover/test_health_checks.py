# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_slow_generation_inline_fails_a_health_check
# (st.data() draws are no different from any other draw in Hegel; a duplicate of
# test_slow_generation_fails_a_health_check); test_the_slow_test_health_check_can_be_disabled
# (deadline); test_returning_non_none_is_forbidden, test_stateful_returnvalue_healthcheck
# (HealthCheck.return_value); test_differing_executors_fails_health_check
# (HealthCheck.differing_executors); test_can_suppress_nested_given,
# test_cant_suppress_nested_given_on_inner, test_suppress_triply_nested_given
# (HealthCheck.nested_given: a nested Hegel test is always an error).

import time

import pytest

import hegel
from hegel import FailedHealthCheck, HealthCheck, InvalidArgument, Settings
from hegel import generators as gs
from tests.hypothesis_port.common.utils import no_shrink

HEALTH_CHECK_SETTINGS = Settings(test_cases=11, database="", suppress_health_check=())

# Hegel's too_slow health check fires when fewer than ten valid test cases
# have taken thirty seconds of real time between them, so tripping it takes
# a while (Hypothesis's suite patches time.sleep; this one cannot, since the
# engine keeps its own clock).
SLOW_SECONDS = 3.5
SLOW_CASES = 9


@pytest.mark.slow
def test_slow_generation_fails_a_health_check():
    @hegel.test(settings=HEALTH_CHECK_SETTINGS, test_cases=SLOW_CASES)
    def test(tc):
        tc.draw(gs.integers().map(lambda x: time.sleep(SLOW_SECONDS)))

    with pytest.raises(FailedHealthCheck, match="TooSlow"):
        test()


def test_default_health_check_can_weaken_specific():
    import random

    @hegel.test(settings=HEALTH_CHECK_SETTINGS, suppress_health_check=list(HealthCheck))
    def test(tc):
        x = tc.draw(gs.lists(gs.integers(), min_size=1))
        random.choice(x)

    test()


def test_suppressing_filtering_health_check():
    forbidden = set()

    def unhealthy_filter(x):
        if len(forbidden) < 200:
            forbidden.add(x)
        return x not in forbidden

    @hegel.test(settings=HEALTH_CHECK_SETTINGS)
    def test1(tc):
        tc.draw(gs.integers().filter(unhealthy_filter))
        raise ValueError

    with pytest.raises(FailedHealthCheck):
        test1()

    forbidden = set()

    @hegel.test(suppress_health_check=[HealthCheck.filter_too_much, HealthCheck.too_slow])
    def test2(tc):
        tc.draw(gs.integers().filter(unhealthy_filter))
        raise ValueError

    with pytest.raises(ValueError):
        test2()


def test_filtering_everything_fails_a_health_check():
    @hegel.test(database="", suppress_health_check=())
    def test(tc):
        tc.draw(gs.integers().filter(lambda x: False))

    with pytest.raises(FailedHealthCheck, match="(?i)filter"):
        test()


def fails_regularly():
    # The original draws two bytes and assumes they equal 3; the same odds as
    # a filter over a 16-bit integer.
    return gs.integers(0, 2**16 - 1).filter(lambda b: b == 3)


def test_filtering_most_things_fails_a_health_check():
    @hegel.test(database="", phases=no_shrink, suppress_health_check=())
    def test(tc):
        tc.draw(fails_regularly())

    with pytest.raises(FailedHealthCheck, match="(?i)filter"):
        test()


@pytest.mark.slow
def test_the_slow_test_health_only_runs_if_health_checks_are_on():
    @hegel.test(suppress_health_check=list(HealthCheck), test_cases=SLOW_CASES)
    def a(tc):
        tc.draw(gs.integers())
        time.sleep(SLOW_SECONDS)

    a()


def test_it_is_an_error_to_suppress_non_iterables():
    with pytest.raises(InvalidArgument):
        Settings(suppress_health_check=1)


def test_it_is_an_error_to_suppress_non_healthchecks():
    with pytest.raises(InvalidArgument):
        Settings(suppress_health_check=["notahealthcheck"])


def test_nested_given_raises_healthcheck():
    @hegel.test
    def f(tc):
        tc.draw(gs.integers())

        @hegel.test
        def g(tc):
            tc.draw(gs.integers())

        g()

    with pytest.raises(InvalidArgument):
        f()


def test_triply_nested_given_raises_healthcheck():
    @hegel.test(test_cases=10)
    def f(tc):
        tc.draw(gs.integers())

        @hegel.test(test_cases=10)
        def g(tc):
            tc.draw(gs.integers())

            @hegel.test(test_cases=10)
            def h(tc):
                tc.draw(gs.integers())

            h()

        g()

    with pytest.raises(InvalidArgument):
        f()
