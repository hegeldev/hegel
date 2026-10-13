# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_branches_pass_through_deferred
# (SearchStrategy.branches internals).

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument
from tests.hypothesis_port.common.debug import (
    assert_no_examples,
    check_can_generate_examples,
    minimal,
)


def test_binary_tree():
    tree = gs.deferred(lambda: gs.integers() | gs.tuples(tree, tree))

    assert minimal(tree) == 0
    assert minimal(tree, lambda x: isinstance(x, tuple)) == (0, 0)


def test_mutual_recursion():
    t = gs.deferred(lambda: a | b)
    a = gs.deferred(lambda: gs.none() | gs.tuples(gs.just("a"), b))
    b = gs.deferred(lambda: gs.none() | gs.tuples(gs.just("b"), a))

    for c in ("a", "b"):
        assert minimal(t, lambda x: x is not None and x[0] == c) == (c, None)  # noqa


def test_errors_on_non_function_define():
    with pytest.raises(InvalidArgument):
        gs.deferred(1)


def test_errors_if_define_does_not_return_search_strategy():
    x = gs.deferred(lambda: 1)
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(x)


@pytest.mark.xfail(
    strict=True,
    reason="hegel's deferred() does not detect a definition as itself: drawing from "
    "deferred(lambda: x) recurses until RecursionError instead of raising InvalidArgument",
)
def test_errors_on_definition_as_self():
    x = gs.deferred(lambda: x)
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(x)


def test_can_draw_one_of_self():
    x = gs.deferred(lambda: gs.one_of(gs.booleans(), x))
    assert minimal(x) is False


def test_hidden_self_references_just_result_in_no_example():
    bad = gs.deferred(lambda: gs.none().flat_map(lambda _: bad))
    assert_no_examples(bad)


def test_self_recursive_flatmap():
    bad = gs.deferred(lambda: bad.flat_map(lambda x: gs.none()))
    assert_no_examples(bad)


def test_self_reference_through_one_of_can_detect_emptiness():
    bad = gs.deferred(lambda: gs.one_of(bad, bad))
    assert bad.is_empty


def test_self_tuple_draws_nothing():
    x = gs.deferred(lambda: gs.tuples(x))
    assert_no_examples(x)


def test_mutually_recursive_tuples_draw_nothing():
    x = gs.deferred(lambda: gs.tuples(y))
    y = gs.tuples(x)

    assert_no_examples(x)
    assert_no_examples(y)


def test_literals_strategy_is_valid():
    literals = gs.deferred(
        lambda: gs.one_of(gs.booleans(), gs.tuples(literals, literals), literals.map(lambda x: [x]))
    )

    @hegel.test
    def test(tc):
        tc.draw(literals)

    test()


def test_impossible_self_recursion():
    x = gs.deferred(lambda: gs.tuples(gs.none(), x))
    assert x.is_empty


def test_very_deep_deferral():
    # This test is designed so that the recursive properties take a very long
    # time to converge: Although we can rapidly determine them for the original
    # value, each round in the fixed point calculation only manages to update
    # a single value in the related strategies, so it takes 100 rounds to
    # update everything. Most importantly this triggers our infinite loop
    # detection heuristic and we start tracking duplicates, but we shouldn't
    # see any because this loop isn't infinite, just long.
    def strat(i):
        if i == 0:
            return gs.deferred(lambda: gs.one_of([*strategies, gs.none()]))
        else:
            return gs.deferred(lambda: gs.tuples(strategies[(i + 1) % len(strategies)]))

    strategies = list(map(strat, range(100)))

    assert not strategies[0].is_empty


def test_recursion_in_middle():
    # This test is significant because the integers().map(abs) is not checked
    # in the initial pass - when we recurse into x initially we decide that
    # x is empty, so the tuple is empty, and don't need to check the third
    # argument. Then when we do the more refined test we've discovered that x
    # is non-empty, so we need to check the non-emptiness of the last component
    # to determine the non-emptiness of the tuples.
    x = gs.deferred(lambda: gs.tuples(gs.none(), x, gs.integers().map(abs)) | gs.none())
    assert not x.is_empty
