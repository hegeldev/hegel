from __future__ import annotations

import threading

import pytest

import hegel
from hegel import generators as gs
from hegel.generators.recursive import DEFERRED_LABEL, Deferred
from tests.helpers import failing, notes, run_test


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


def is_json(value):
    if value is None or isinstance(value, (bool, int, str)):
        return True
    if isinstance(value, list):
        return all(is_json(v) for v in value)
    if isinstance(value, dict):
        return all(isinstance(k, str) and is_json(v) for k, v in value.items())
    return False


json = gs.recursive(
    gs.none() | gs.booleans() | gs.integers() | gs.text(),
    lambda children: gs.lists(children) | gs.dicts(gs.text(), children),
)


def depth(value):
    if isinstance(value, list):
        return 1 + max((depth(v) for v in value), default=0)
    if isinstance(value, dict):
        return 1 + max((depth(v) for v in value.values()), default=0)
    return 0


def leaves(value):
    if isinstance(value, list):
        return sum(leaves(v) for v in value)
    if isinstance(value, dict):
        return sum(leaves(v) for v in value.values())
    return 1


def test_recursive_generates_nested_values():
    seen = []
    check_all(json, lambda v: is_json(v) and seen.append(v) is None, test_cases=200)
    assert any(depth(v) >= 2 for v in seen)


def test_recursive_respects_limits():
    shallow = gs.recursive(gs.integers(), gs.lists, max_depth=1)
    check_all(shallow, lambda v: depth(v) <= 1, test_cases=100)
    few = gs.recursive(gs.integers(), lambda c: gs.lists(c, min_size=2), max_leaves=5)
    check_all(few, lambda v: leaves(v) <= 5, test_cases=200)
    assert repr(gs.recursive(gs.integers(), gs.lists)) == "recursive(integers(), 'lists')"
    with pytest.raises(hegel.InvalidArgument):
        gs.recursive(gs.integers(), gs.lists, max_depth=-1)
    with pytest.raises(hegel.InvalidArgument):
        gs.recursive(gs.integers(), gs.lists, max_leaves=0)
    with pytest.raises(hegel.InvalidArgument):
        gs.recursive(5, gs.lists)


def test_recursive_shrinks_and_prints():
    assert hegel.find_example(json, lambda v: isinstance(v, list) and v) == [None]
    assert hegel.find_example(json, lambda v: depth(v) >= 2) == [[]]

    def body(tc):
        v = tc.draw(gs.recursive(gs.just(1), lambda c: gs.lists(c, min_size=2, max_size=2)))
        assert depth(v) < 1

    assert notes(failing(body))[0] == "Falsifying example:\nv = [1, 1]"


def test_recursive_branch_must_return_generator():
    def body(tc):
        tc.draw(gs.recursive(gs.integers(), lambda c: [c]))

    failing(body, hegel.InvalidArgument, test_cases=20)


def test_recursive_retry_on_too_many_leaves_within_other_draws():
    wide = gs.recursive(gs.integers(), lambda c: gs.lists(c, min_size=3), max_leaves=4)

    def body(tc):
        outer = tc.draw(gs.tuples(gs.integers(), wide))
        assert leaves(outer[1]) <= 4
        assert tc.open_span_depth == 0

    run_test(body, test_cases=200)


def test_recursive_errors_inside_branch_propagate():
    def body(tc):
        tc.draw(gs.recursive(gs.integers().map(lambda x: 1 / 0), gs.lists))

    failing(body, ZeroDivisionError)


def test_deferred_self_reference():
    tree = gs.deferred(lambda: gs.integers() | gs.tuples(tree, tree))

    def flat(v):
        return [v] if isinstance(v, int) else flat(v[0]) + flat(v[1])

    check_all(tree, lambda v: all(isinstance(x, int) for x in flat(v)), test_cases=100)
    assert isinstance(tree, Deferred)
    assert tree.label == tree.label
    assert tree.label != DEFERRED_LABEL
    assert repr(tree) == "one_of(integers(), tuples(..., ...))"
    assert hegel.find_example(tree, lambda v: isinstance(v, tuple)) == (0, 0)

    def body(tc):
        v = tc.draw(tree)
        assert not isinstance(v, tuple)

    assert notes(failing(body))[0] == "Falsifying example:\nv = (0, 0)"


def test_deferred_label_of_direct_cycle_and_errors():
    cyclic = gs.deferred(lambda: cyclic)
    assert cyclic.label == DEFERRED_LABEL
    assert repr(gs.deferred(lambda: gs.integers())).startswith("deferred(")

    broken = gs.deferred(lambda: 1 / 0)
    assert broken.label == DEFERRED_LABEL
    with pytest.raises(ZeroDivisionError):
        hegel.find_example(broken)
    not_generator = gs.deferred(lambda: 5)
    assert not_generator.label == DEFERRED_LABEL
    with pytest.raises(hegel.InvalidArgument):
        hegel.find_example(not_generator)
    with pytest.raises(hegel.InvalidArgument):
        gs.deferred(5)


def test_deferred_mutual_recursion_and_threads():
    a = gs.deferred(lambda: gs.just("a") | b)
    b = gs.deferred(lambda: gs.just("b") | a)
    check_all(a, lambda v: v in "ab")
    labels = []
    threads = [threading.Thread(target=lambda: labels.append(a.label)) for _ in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert len(set(labels)) == 1


def test_recursive_branches_can_draw_subtrees_silently():
    nested = gs.recursive(gs.integers(), lambda c: gs.lists(c, min_size=1).map(tuple))
    check_all(nested, lambda v: isinstance(v, (int, tuple)), test_cases=100)
    wrapped = gs.recursive(gs.integers(), lambda c: c.map(lambda v: [v]))
    check_all(wrapped, lambda v: isinstance(v, (int, list)), test_cases=100)
