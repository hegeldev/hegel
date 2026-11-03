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
# test_self_reference_through_one_of_can_detect_emptiness,
# test_impossible_self_recursion, test_very_deep_deferral, test_recursion_in_middle
# (Hegel generators have no is_empty).

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument
from tests.hypothesis_port.common.debug import (
    assert_no_examples,
    check_can_generate_examples,
    minimal,
)


def assert_too_large(generator):
    # Hegel abandons draws nested close to Python's recursion limit as too
    # large, so a generator with no finite values fails a health check.
    with pytest.raises(hegel.FailedHealthCheck):
        assert_no_examples(generator)


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


def test_errors_on_definition_as_self():
    x = gs.deferred(lambda: x)
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(x)


def test_can_draw_one_of_self():
    x = gs.deferred(lambda: gs.one_of(gs.booleans(), x))
    assert minimal(x) is False


def test_hidden_self_references_just_result_in_no_example():
    bad = gs.deferred(lambda: gs.none().flat_map(lambda _: bad))
    assert_too_large(bad)


def test_self_recursive_flatmap():
    bad = gs.deferred(lambda: bad.flat_map(lambda x: gs.none()))
    assert_too_large(bad)


def test_self_tuple_draws_nothing():
    x = gs.deferred(lambda: gs.tuples(x))
    assert_too_large(x)


def test_mutually_recursive_tuples_draw_nothing():
    x = gs.deferred(lambda: gs.tuples(y))
    y = gs.tuples(x)

    assert_too_large(x)
    assert_too_large(y)


def test_literals_strategy_is_valid():
    literals = gs.deferred(
        lambda: gs.one_of(gs.booleans(), gs.tuples(literals, literals), literals.map(lambda x: [x]))
    )

    @hegel.test
    def test(tc):
        tc.draw(literals)

    test()
