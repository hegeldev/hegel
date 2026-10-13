# Dropped from the Hypothesis original:
# test_run_stateful_test_with_transformed_bundles_concurrently (Bundle,
# initialize, consumes, multiple; Hegel's pools are per-instance);
# test_single_thread_can_raise_deadline_exceeded,
# test_deadline_exceeded_not_raised_under_concurrent_threads,
# test_deadline_exceeded_can_be_raised_after_threads (deadline);
# test_recursive_property_concurrent, test_one_of_branches_lock
# (SearchStrategy internals: validate(), branches).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import contextlib
import sys
from threading import Thread

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import check_can_generate_examples
from tests.hypothesis_port.common.utils import run_concurrently


def test_can_run_given_in_thread():
    has_run_successfully = False

    @hegel.test
    def test(tc):
        nonlocal has_run_successfully
        tc.draw(gs.integers())
        has_run_successfully = True

    t = Thread(target=test)
    t.start()
    t.join()
    assert has_run_successfully


def test_run_stateful_test_concurrently():
    class MyStateMachine(hegel.StateMachine):
        def __init__(self):
            super().__init__()

        @hegel.rule()
        def my_rule(self, tc):
            tc.draw(gs.integers())

        @hegel.invariant()
        def my_invariant(self, tc):
            pass

    TestMyStateful = hegel.state_machine_test(MyStateMachine)
    run_concurrently(TestMyStateful, n=2)


def do_work(*, multiplier=1):
    # arbitrary moderately-expensive work
    for x in range(500 * multiplier):
        _y = x**x


def test_run_different_tests_in_threads():
    @hegel.test
    def test1(tc):
        tc.draw(gs.integers())
        do_work()

    @hegel.test
    def test2(tc):
        tc.draw(gs.integers())
        do_work()

    thread1 = Thread(target=test1)
    thread2 = Thread(target=test2)

    thread1.start()
    thread2.start()
    thread1.join()
    thread2.join()


def test_run_given_concurrently():
    @hegel.test
    def test(tc):
        magnitude = tc.draw(gs.integers(-5, 5).map(lambda x: 10**x))
        assert magnitude != 0
        tc.draw(gs.floats(min_value=-magnitude, max_value=magnitude))

    run_concurrently(test, n=2)


def test_stackframes_restores_original_recursion_limit():
    original_recursionlimit = sys.getrecursionlimit()

    def test():
        with contextlib.nullcontext():
            do_work()

            # also mix in a hegel test; why not.
            @hegel.test(test_cases=10)
            def test(tc):
                tc.draw(gs.integers())

            test()

    threads = []
    for _ in range(4):
        threads.append(Thread(target=test))
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(timeout=10)

    assert sys.getrecursionlimit() == original_recursionlimit


@pytest.mark.parametrize(
    "make_generator",
    [
        # Hegel validates eagerly, so the generators are built inside the test.
        lambda: gs.recursive(gs.none(), gs.lists, max_leaves=-1),
        lambda: gs.recursive(gs.none(), gs.lists, max_leaves=0),
        lambda: gs.recursive(gs.none(), gs.lists, max_leaves=1.0),
    ],
)
def test_handles_invalid_args_cleanly(make_generator):
    # we previously had a race in SearchStrategy.validate, where one thread would
    # set `validate_called = True` (which it has to do first for recursive
    # strategies), then another thread would try to generate before the validation
    # finished and errored, and would get into weird technically-valid states
    # like interpreting 1.0 as 1. I saw FlakyStrategyDefinition here because the
    # validating + errored thread drew zero choices, but the other thread drew
    # 1 choice, for the same shared strategy.

    def check():
        with pytest.raises(hegel.InvalidArgument):
            check_can_generate_examples(make_generator())

    run_concurrently(check, n=4)
