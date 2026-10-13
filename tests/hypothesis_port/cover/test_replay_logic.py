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
from hegel import MultipleFailures
from hegel import generators as gs


def test_does_not_shrink_on_replay(tmp_path):
    call_count = 0

    is_first = True
    last = None

    @hegel.test(
        database=tmp_path,
        report_multiple_failures=False,
        derandomize=False,
        test_cases=1000,
    )
    def test(tc):
        nonlocal call_count, is_first, last
        ls = tc.draw(gs.lists(gs.integers(), unique=True, min_size=3))
        if is_first and last is not None:
            assert ls == last
        is_first = False
        last = ls
        call_count += 1
        raise AssertionError

    with pytest.raises(AssertionError):
        test()

    assert last is not None

    call_count = 0
    is_first = True

    with pytest.raises(AssertionError):
        test()

    # The replayed example fails, and is run once more to report it.
    assert call_count == 2


def test_does_not_shrink_on_replay_with_multiple_bugs(tmp_path):
    call_count = 0
    raised = False
    marker = 1000093

    @hegel.test(
        database=tmp_path,
        report_multiple_failures=True,
        derandomize=False,
        test_cases=1000,
    )
    def test(tc):
        nonlocal call_count, raised
        n = tc.draw(gs.integers())
        call_count += 1
        if n >= marker:
            raised = True
            raise AssertionError
        elif n < marker and raised:
            raise AssertionError

    with pytest.raises(MultipleFailures):
        test()

    call_count = 0

    with pytest.raises(MultipleFailures):
        test()

    assert call_count == 4


def test_will_always_shrink_if_previous_example_does_not_replay(tmp_path):
    good = set()
    last = None

    @hegel.test(
        database=tmp_path,
        report_multiple_failures=True,
        derandomize=False,
        test_cases=1000,
    )
    def test(tc):
        nonlocal last
        n = tc.draw(gs.integers(min_value=0))
        if n not in good:
            last = n
            raise AssertionError

    for i in range(20):
        with pytest.raises(AssertionError):
            test()
        assert last == i
        good.add(last)


def test_will_shrink_if_the_previous_example_does_not_look_right(tmp_path):
    last = None

    first_test = True

    @hegel.test(database=tmp_path, report_multiple_failures=True, derandomize=False)
    def test(tc):
        nonlocal last
        m = tc.draw(gs.integers())
        last = m
        if first_test:
            tc.draw(gs.integers())
            assert m < 10000
        else:
            raise AssertionError

    with pytest.raises(AssertionError):
        test()

    assert last is not None
    assert last > 0

    first_test = False

    with pytest.raises(AssertionError):
        test()

    assert last == 0
