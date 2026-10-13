# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_respects_min_leaves,
# test_can_set_exact_leaf_count and the min_leaves cases of test_invalid_args
# (recursive(min_leaves=...)); test_identity_extend_warns (HypothesisWarning).

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument
from tests.hypothesis_port.common.debug import (
    assert_no_examples,
    check_can_generate_examples,
    find_any,
    minimal,
)


def flatten(x):
    if isinstance(x, list):
        return sum(map(flatten, x), [])
    else:
        return [x]


@hegel.test
def test_respects_leaf_limit(tc):
    xs = tc.draw(gs.recursive(gs.booleans(), gs.lists, max_leaves=10))
    assert len(flatten(xs)) <= 10


def test_can_find_nested():
    x = minimal(
        gs.recursive(gs.booleans(), lambda x: gs.tuples(x, x)),
        lambda x: isinstance(x, tuple) and isinstance(x[0], tuple),
    )

    assert x == ((False, False), False)


def test_recursive_call_validates_expand_returns_strategies():
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.recursive(gs.booleans(), lambda x: 1))


def test_recursive_call_validates_base_is_strategy():
    with pytest.raises(InvalidArgument):
        gs.recursive(1, lambda x: gs.none())


def test_can_find_exactly_max_leaves():
    strat = gs.recursive(gs.none(), lambda x: gs.tuples(x, x), max_leaves=5)

    def enough_leaves(t):
        count = 0
        stack = [t]
        while stack:
            s = stack.pop()
            if s is None:
                count += 1
            else:
                stack.extend(s)
        return count >= 5

    find_any(strat, enough_leaves)


@hegel.test
def test_can_exclude_branching_with_max_leaves(tc):
    t = tc.draw(gs.recursive(gs.none(), lambda x: gs.tuples(x, x), max_leaves=1))
    assert t is None


@hegel.test
def test_issue_1502_regression(tc):
    tc.draw(gs.recursive(gs.none(), lambda x: gs.one_of(x, x)))


def test_recursive_can_generate_varied_structures():
    values = gs.recursive(gs.none(), gs.lists)

    find_any(values, lambda x: x is None)
    find_any(values, lambda x: isinstance(x, list))
    find_any(values, lambda x: isinstance(x, list) and any(isinstance(y, list) for y in x))


def test_recursive_can_generate_varied_structures_without_using_leaves():
    values = gs.recursive(gs.none(), lambda _: gs.lists(gs.none()))

    find_any(values, lambda x: x is None)
    find_any(values, lambda x: isinstance(x, list))
    # The bad `extend` function means we can't actually recurse!
    assert_no_examples(
        values, lambda x: isinstance(x, list) and any(isinstance(y, list) for y in x)
    )


# Validation is eager, so each invalid definition is built inside the test.
@pytest.mark.parametrize(
    "make",
    [
        lambda: gs.recursive(None, gs.lists),
        lambda: gs.recursive(gs.none(), lambda x: None),
        lambda: gs.recursive(gs.none(), gs.lists, max_leaves=-1),
        lambda: gs.recursive(gs.none(), gs.lists, max_leaves=0),
        lambda: gs.recursive(gs.none(), gs.lists, max_leaves=1.0),
    ],
)
def test_invalid_args(make):
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(make())
