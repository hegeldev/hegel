# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_fixed_dictionaries_vary_key_order
# (fixed_dicts varying key order).

from collections import OrderedDict

import pytest

import hegel
from hegel.generators import (
    booleans,
    dicts,
    fixed_dicts,
    frozensets,
    integers,
    lists,
    none,
    nothing,
    sets,
    text,
    tuples,
)
from tests.hypothesis_port.common.debug import find_any, minimal
from tests.hypothesis_port.common.utils import flaky


@pytest.mark.parametrize(
    ("col", "strat"),
    [
        ((), tuples()),
        ([], lists(none(), max_size=0)),
        (set(), sets(none(), max_size=0)),
        (frozenset(), frozensets(none(), max_size=0)),
        ({}, fixed_dicts({})),
        ({}, fixed_dicts({}, optional={})),
        (OrderedDict(), fixed_dicts(OrderedDict(), optional=OrderedDict())),
        ({}, fixed_dicts({}, optional={1: booleans()})),
        ({0: False}, fixed_dicts({0: booleans()}, optional={1: booleans()})),
        ({}, fixed_dicts({}, optional={(): booleans(), 0: booleans()})),
        ([], lists(nothing())),
        ([], lists(nothing(), unique=True)),
    ],
)
def test_find_empty_collection_gives_empty(col, strat):
    assert minimal(strat) == col


@pytest.mark.parametrize(
    ("coltype", "strat"), [(list, lists), (set, sets), (frozenset, frozensets)]
)
def test_find_non_empty_collection_gives_single_zero(coltype, strat):
    assert minimal(strat(integers()), bool) == coltype((0,))


@pytest.mark.parametrize(
    ("coltype", "strat"), [(list, lists), (set, sets), (frozenset, frozensets)]
)
def test_minimizes_to_empty(coltype, strat):
    assert minimal(strat(integers())) == coltype()


def test_minimizes_list_of_lists():
    xs = minimal(lists(lists(booleans())), lambda x: any(x) and not all(x))
    xs.sort()
    assert xs == [[], [False]]


@hegel.test(test_cases=100)
def test_sets_are_size_bounded(tc):
    xs = tc.draw(sets(integers(0, 100), min_size=2, max_size=10))
    assert 2 <= len(xs) <= 10


@hegel.test
def test_fixed_dictionaries_with_optional_and_empty_keys(tc):
    d = tc.draw(fixed_dicts({}, optional={0: booleans(), 1: nothing(), 2: booleans()}))
    assert 1 not in d


@pytest.mark.parametrize("n", range(10))
def test_lists_of_fixed_length(n):
    assert minimal(lists(integers(), min_size=n, max_size=n)) == [0] * n


@pytest.mark.parametrize("n", range(10))
def test_sets_of_fixed_length(n):
    x = minimal(sets(integers(), min_size=n, max_size=n))
    assert len(x) == n

    if n == 0:
        assert x == set()
    else:
        assert x == set(range(min(x), min(x) + n))


@pytest.mark.parametrize("n", range(10))
def test_dictionaries_of_fixed_length(n):
    x = set(minimal(dicts(integers(), booleans(), min_size=n, max_size=n)))

    if not n:
        assert x == set()
    else:
        assert x == set(range(min(x), min(x) + n))


@pytest.mark.parametrize("n", range(10))
def test_lists_of_lower_bounded_length(n):
    l = minimal(lists(integers(), min_size=n), lambda x: sum(x) >= 2 * n)
    assert l == [] if n == 0 else [0] * (n - 1) + [n * 2]


@flaky(min_passes=1, max_runs=3)
def test_can_find_unique_lists_of_non_set_order():
    # This test checks that our strategy for unique lists doesn't accidentally
    # depend on the iteration order of sets.
    #
    # Unfortunately, that means that *this* test has to rely on set iteration
    # order. That makes it tricky to debug on CPython, because set iteration
    # order changes every time the process is launched.
    #
    # To get around this, define the PYTHONHASHSEED environment variable to
    # a consistent value. This could be 0, or it could be the PYTHONHASHSEED
    # value listed in a failure log from CI.

    ls = minimal(
        lists(text(), min_size=2, unique=True),
        lambda x: list(set(reversed(x))) != x,  # noqa: C414  # yes, reverse inside set
    )
    assert len(set(ls)) == len(ls)
    assert len(ls) == 2


def test_can_draw_empty_list_from_unsatisfiable_strategy():
    assert find_any(lists(integers().filter(lambda s: False))) == []


def test_can_draw_empty_set_from_unsatisfiable_strategy():
    assert find_any(sets(integers().filter(lambda s: False))) == set()


@hegel.test
def test_small_sized_sets(tc):
    tc.draw(lists(sets(none()), min_size=10))


def test_minimize_dicts_with_incompatible_keys():
    strat = fixed_dicts({1: booleans(), "hi": lists(booleans())})
    assert minimal(strat) == {1: False, "hi": []}


@hegel.test
def test_lists_unique_by_tuple_funcs(tc):
    ls = tc.draw(
        lists(
            tuples(integers(), integers()),
            min_size=2,
            unique_by=(lambda x: x[0], lambda x: x[1]),
        )
    )
    firstitems, seconditems = zip(*ls, strict=True)
    assert len(set(firstitems)) == len(firstitems)
    assert len(set(seconditems)) == len(seconditems)
