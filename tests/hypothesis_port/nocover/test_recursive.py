# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original:
# test_gc_hooks_do_not_cause_unraisable_recursionerror (Hypothesis's GC hooks).

import sys
import threading

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import find_any, minimal
from tests.hypothesis_port.common.utils import flaky


def test_can_generate_with_large_branching():
    def flatten(x):
        if isinstance(x, list):
            return sum(map(flatten, x), [])
        else:
            return [x]

    size = 20

    xs = minimal(
        gs.recursive(
            gs.integers(),
            lambda x: gs.lists(x, min_size=size // 2),
            max_leaves=size * 2,
        ),
        lambda x: isinstance(x, list) and len(flatten(x)) >= size,
    )
    assert flatten(xs) == [0] * size


def test_can_generate_some_depth_with_large_branching():
    def depth(x):
        if x and isinstance(x, list):
            return 1 + max(map(depth, x))
        else:
            return 1

    xs = minimal(gs.recursive(gs.integers(), gs.lists), lambda x: depth(x) > 1)
    assert xs in ([0], [[]])


def test_can_find_quite_broad_lists():
    def breadth(x):
        if isinstance(x, list):
            return sum(map(breadth, x))
        else:
            return 1

    target = 10

    broad = minimal(
        gs.recursive(gs.booleans(), lambda x: gs.lists(x, max_size=target // 2)),
        lambda x: breadth(x) >= target,
        settings=hegel.Settings(test_cases=10000),
    )
    assert breadth(broad) == target


def test_drawing_many_near_boundary():
    size = 4
    elems = gs.recursive(
        gs.booleans(),
        lambda x: gs.lists(x, min_size=2 * (size - 1), max_size=2 * size).map(tuple),
        max_leaves=2 * size - 1,
    )
    ls = minimal(gs.lists(elems), lambda x: len(set(x)) >= size)
    assert len(ls) == size


def test_can_use_recursive_data_in_sets():
    nested_sets = gs.recursive(gs.booleans(), gs.frozensets, max_leaves=3)
    find_any(nested_sets)

    def flatten(x):
        if isinstance(x, bool):
            return frozenset((x,))
        else:
            result = frozenset()
            for t in x:
                result |= flatten(t)
                if len(result) == 2:
                    break
            return result

    x = minimal(nested_sets, lambda x: len(flatten(x)) == 2)
    assert x in (
        frozenset((False, True)),
        frozenset((False, frozenset((True,)))),
        frozenset((frozenset((False, True)),)),
    )


@flaky(max_runs=2, min_passes=1)
def test_can_form_sets_of_recursive_data():
    size = 3

    trees = gs.sets(
        gs.recursive(
            gs.booleans(),
            lambda x: gs.lists(x, min_size=size).map(tuple),
            max_leaves=20,
        )
    )
    xs = minimal(trees, lambda x: len(x) >= size)
    assert len(xs) == size


def test_drawing_from_recursive_strategy_is_thread_safe():
    shared_generator = gs.recursive(gs.integers(), lambda s: gs.lists(s, max_size=2), max_leaves=20)
    errors = []

    @hegel.test(database="", suppress_health_check=[hegel.HealthCheck.too_slow])
    def test(tc):
        try:
            tc.draw(shared_generator)
        except Exception as exc:
            errors.append(exc)

    original_recursionlimit = sys.getrecursionlimit()
    threads = []
    for _ in range(4):
        threads.append(threading.Thread(target=test))

    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    assert sys.getrecursionlimit() == original_recursionlimit
    assert not errors


SELF_REF = gs.recursive(
    gs.deferred(lambda: gs.booleans() | SELF_REF),
    lambda s: gs.lists(s, min_size=1),
)


@hegel.test(
    suppress_health_check=[hegel.HealthCheck.too_slow, hegel.HealthCheck.filter_too_much],
    test_cases=5,
)
def test_self_ref_regression(tc):
    # See https://github.com/HypothesisWorks/hypothesis/issues/2794
    tc.draw(SELF_REF)
