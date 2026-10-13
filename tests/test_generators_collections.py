from __future__ import annotations

from collections.abc import Iterator

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import failing, find_example, notes, run_test


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


def test_lists():
    check_all(gs.lists(gs.integers()), lambda xs: isinstance(xs, list))
    check_all(gs.lists(gs.integers(), min_size=2, max_size=4), lambda xs: 2 <= len(xs) <= 4)
    check_all(gs.lists(gs.integers(0, 3), unique=True), lambda xs: len(set(xs)) == len(xs))
    check_all(
        gs.lists(gs.integers(0, 20), unique_by=lambda x: x % 3),
        lambda xs: len({x % 3 for x in xs}) == len(xs),
    )
    check_all(
        gs.lists(
            gs.tuples(gs.integers(0, 3), gs.integers(0, 3)),
            unique_by=(lambda p: p[0], lambda p: p[1]),
        ),
        lambda ps: len({p[0] for p in ps}) == len(ps) == len({p[1] for p in ps}),
    )
    assert repr(gs.lists(gs.just(1))) == "lists(just(1), min_size=0, max_size=None)"
    assert find_example(gs.lists(gs.integers()), lambda xs: len(xs) > 2) == [0, 0, 0]
    assert find_example(gs.lists(gs.integers(0, 3), unique=True, min_size=3)) == [0, 1, 2]


def test_lists_errors():
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), unique=True, unique_by=id)
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), unique_by=())
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), unique_by=(1,))
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), unique_by=1)
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), unique_by="ab")
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), min_size=-1)
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), max_size=-1)
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), min_size=True)
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.integers(), min_size=3, max_size=2)
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(5)


def test_unique_lists_report_only_accepted_elements():
    def body(tc):
        xs = tc.draw(gs.lists(gs.integers(0, 1), unique=True, min_size=2))
        assert xs != [0, 1]

    assert notes(failing(body))[0] == "Falsifying example:\nxs = [0, 1]"


def test_sets_and_frozensets():
    check_all(gs.sets(gs.integers(0, 5)), lambda s: isinstance(s, set))
    check_all(gs.sets(gs.integers(0, 5), min_size=2, max_size=3), lambda s: 2 <= len(s) <= 3)
    check_all(
        gs.frozensets(gs.integers(0, 5), min_size=1), lambda s: isinstance(s, frozenset) and s
    )
    assert repr(gs.sets(gs.just(1))) == "sets(just(1), min_size=0, max_size=None)"
    assert repr(gs.frozensets(gs.just(1))) == "frozensets(just(1), min_size=0, max_size=None)"
    assert find_example(gs.sets(gs.integers(), min_size=2)) == {0, 1}
    with pytest.raises(hegel.InvalidArgument):
        gs.sets(gs.integers(), min_size=2, max_size=1)
    with pytest.raises(hegel.InvalidArgument):
        gs.frozensets(gs.integers(), min_size=2, max_size=1)


def test_sets_print_structurally():
    def body(tc):
        s = tc.draw(gs.sets(gs.integers(0, 3), min_size=2))
        f = tc.draw(gs.frozensets(gs.integers(0, 3), max_size=0))
        raise ValueError((s, f))

    text = notes(failing(body, ValueError))[0]
    assert "s = {0, 1}" in text
    assert "f = frozenset({})" in text or "f = frozenset()" in text


def test_iterables_are_one_shot_iterators():
    def body(tc):
        it = tc.draw(gs.iterables(gs.integers(0, 3), min_size=1, unique=True))
        assert isinstance(it, Iterator)
        assert iter(it) is it
        values = list(it)
        assert values
        assert list(it) == []
        assert repr(it) == f"iter({values!r})"

    run_test(body, test_cases=10)


def test_collections_of_an_empty_generator_are_empty():
    check_all(gs.lists(gs.nothing()), lambda xs: xs == [])
    check_all(gs.lists(gs.nothing(), unique=True), lambda xs: xs == [])
    check_all(gs.sets(gs.nothing()), lambda s: s == set())
    check_all(gs.frozensets(gs.nothing()), lambda s: s == frozenset())
    check_all(gs.dicts(gs.nothing(), gs.none()), lambda d: d == {})
    check_all(gs.dicts(gs.none(), gs.nothing()), lambda d: d == {})

    def body(tc):
        xs = tc.draw(gs.lists(gs.nothing()))
        s = tc.draw(gs.sets(gs.nothing()))
        f = tc.draw(gs.frozensets(gs.nothing()))
        d = tc.draw(gs.dicts(gs.nothing(), gs.none()))
        raise ValueError((xs, s, f, d))

    text = notes(failing(body, ValueError))[0]
    assert "xs = []" in text
    assert "s = {}" in text or "s = set()" in text
    assert "f = frozenset({})" in text or "f = frozenset()" in text
    assert "d = {}" in text


@pytest.mark.parametrize(
    "build",
    [
        lambda: gs.lists(gs.nothing(), min_size=1),
        lambda: gs.lists(gs.nothing(), min_size=1, unique=True),
        lambda: gs.sets(gs.nothing(), min_size=1),
        lambda: gs.frozensets(gs.nothing(), min_size=1),
        lambda: gs.dicts(gs.nothing(), gs.none(), min_size=1),
        lambda: gs.dicts(gs.none(), gs.nothing(), min_size=1),
    ],
)
def test_non_empty_collections_of_an_empty_generator_are_rejected(build):
    with pytest.raises(hegel.InvalidArgument, match="it has no values"):
        build()

    def body(tc):
        tc.draw(gs.integers().flat_map(lambda _: build()))

    failing(body, hegel.InvalidArgument)


def test_a_definition_whose_elements_turn_out_empty_is_rejected_when_drawn():
    only_itself = gs.deferred(lambda: gs.lists(only_itself, min_size=1))
    assert only_itself.is_empty

    def body(tc):
        tc.draw(only_itself)

    assert "it has no values" in str(failing(body, hegel.InvalidArgument))


@pytest.mark.parametrize(
    ("build", "count"),
    [
        (lambda: gs.lists(gs.sampled_from([1]), min_size=2, unique=True), 1),
        (lambda: gs.lists(gs.sampled_from([1, 1, 2]), min_size=3, unique=True), 2),
        (lambda: gs.lists(gs.sampled_from([[1], [1]]), min_size=3, unique_by=len), 2),
        (lambda: gs.lists(gs.booleans(), min_size=3, unique=True), 2),
        (lambda: gs.lists(gs.weighted_booleans(1.0), min_size=2, unique=True), 1),
        (lambda: gs.lists(gs.just(1), min_size=2, unique_by=id), 1),
        (lambda: gs.lists(gs.integers(0, 1), min_size=3, unique=True), 2),
        (lambda: gs.sets(gs.booleans(), min_size=3), 2),
        (lambda: gs.frozensets(gs.booleans(), min_size=3), 2),
        (lambda: gs.dicts(gs.booleans(), gs.integers(), min_size=3), 2),
    ],
)
def test_unique_collections_need_enough_distinct_values(build, count):
    plural = "" if count == 1 else "s"
    with pytest.raises(hegel.InvalidArgument, match=f"only {count} distinct value{plural}$"):
        build()


def test_unique_collections_with_enough_distinct_values():
    check_all(gs.lists(gs.sampled_from([1, 1, 2]), min_size=2, unique=True), lambda xs: xs)
    check_all(gs.lists(gs.sampled_from([[1], [1]]), min_size=2, unique_by=id), lambda xs: xs)
    check_all(gs.lists(gs.integers(0, 2), min_size=3, unique=True), lambda xs: len(xs) == 3)
    check_all(gs.lists(gs.integers(min_value=0), min_size=3, unique=True), lambda xs: xs)
    check_all(gs.lists(gs.booleans(), min_size=2, unique=True), lambda xs: set(xs) == {0, 1})
    check_all(gs.sets(gs.just(1), min_size=1), lambda s: s == {1})
    check_all(gs.dicts(gs.booleans(), gs.none(), min_size=2), lambda d: len(d) == 2)


def test_dicts():
    check_all(gs.dicts(gs.integers(0, 3), gs.text()), lambda d: isinstance(d, dict))
    check_all(
        gs.dicts(gs.integers(0, 3), gs.booleans(), min_size=2, max_size=3),
        lambda d: 2 <= len(d) <= 3,
    )
    assert (
        repr(gs.dicts(gs.just(1), gs.just(2)))
        == "dicts(just(1), just(2), min_size=0, max_size=None)"
    )
    assert find_example(gs.dicts(gs.integers(), gs.integers(), min_size=2)) == {0: 0, 1: 0}
    with pytest.raises(hegel.InvalidArgument):
        gs.dicts(gs.integers(), gs.integers(), min_size=2, max_size=1)

    def body(tc):
        d = tc.draw(gs.dicts(gs.integers(0, 1), gs.just("v"), min_size=2))
        raise ValueError(d)

    assert "d = {0: 'v', 1: 'v'}" in notes(failing(body, ValueError))[0]


def test_fixed_dicts():
    check_all(
        gs.fixed_dicts({"a": gs.integers(), "b": gs.text()}),
        lambda d: set(d) == {"a", "b"},
    )
    check_all(
        gs.fixed_dicts({"a": gs.integers()}, optional={"b": gs.booleans()}),
        lambda d: set(d) in ({"a"}, {"a", "b"}),
    )
    check_all(gs.fixed_dicts({}, optional={"b": gs.booleans()}), lambda d: set(d) <= {"b"})
    assert find_example(gs.fixed_dicts({1: gs.integers()}, optional={2: gs.integers()})) == {1: 0}
    assert find_example(
        gs.fixed_dicts({1: gs.integers()}, optional={2: gs.integers()}), lambda d: 2 in d
    ) == {1: 0, 2: 0}
    with pytest.raises(hegel.InvalidArgument):
        gs.fixed_dicts({"a": gs.integers()}, optional={"a": gs.integers()})
    with pytest.raises(hegel.InvalidArgument, match="mapping='fish' must be a dict"):
        gs.fixed_dicts("fish")
    with pytest.raises(hegel.InvalidArgument, match="optional='fish' must be a dict"):
        gs.fixed_dicts({}, optional="fish")
    with pytest.raises(hegel.InvalidArgument):
        gs.fixed_dicts({"a": "fish"})

    def body(tc):
        d = tc.draw(gs.fixed_dicts({"a": gs.just(1)}, optional={"b": gs.just(2)}))
        assert "b" not in d

    assert "d = {'a': 1, 'b': 2}" in notes(failing(body))[0]


def test_tuples():
    check_all(gs.tuples(gs.integers(), gs.text()), lambda t: len(t) == 2 and isinstance(t[1], str))
    check_all(gs.tuples(), lambda t: t == ())
    assert repr(gs.tuples(gs.just(1), gs.just(2))) == "tuples(just(1), just(2))"
    assert find_example(gs.tuples(gs.integers(), gs.booleans())) == (0, False)

    def body(tc):
        single = tc.draw(gs.tuples(gs.just(1)))
        pair = tc.draw(gs.tuples(gs.just(1), gs.just("x")))
        raise ValueError((single, pair))

    text = notes(failing(body, ValueError))[0]
    assert "single = (1,)" in text
    assert "pair = (1, 'x')" in text


def test_subsequences():
    check_all(
        gs.subsequences([1, 2, 3, 4]), lambda xs: xs == sorted(xs) and set(xs) <= {1, 2, 3, 4}
    )
    check_all(gs.subsequences(range(5), min_size=2, max_size=3), lambda xs: 2 <= len(xs) <= 3)
    check_all(gs.subsequences([1, 2], max_size=10), lambda xs: len(xs) <= 2)
    check_all(gs.subsequences([]), lambda xs: xs == [])
    assert find_example(gs.subsequences([3, 1, 2]), lambda xs: len(xs) == 2) == [3, 1]
    assert repr(gs.subsequences(range(2))) == "subsequences([0, 1], min_size=0, max_size=None)"
    with pytest.raises(hegel.InvalidArgument):
        gs.subsequences([1], min_size=2, max_size=1)

    def body(tc):
        tc.draw(gs.subsequences([1], min_size=2))

    failing(body, hegel.InvalidArgument)


def test_permutations():
    check_all(gs.permutations([1, 2, 3]), lambda xs: sorted(xs) == [1, 2, 3])
    check_all(gs.permutations([]), lambda xs: xs == [])
    assert find_example(gs.permutations([3, 1, 2])) == [3, 1, 2]
    assert find_example(gs.permutations([1, 2, 3]), lambda xs: xs[0] != 1) == [2, 1, 3]
    assert repr(gs.permutations("abc")) == "permutations(['a', 'b', 'c'])"


def test_samples():
    check_all(gs.samples([1, 2, 3]), lambda xs: set(xs) <= {1, 2, 3})
    check_all(gs.samples([1, 2, 3], min_size=5, max_size=6), lambda xs: 5 <= len(xs) <= 6)
    check_all(gs.samples([], max_size=5), lambda xs: xs == [])
    check_all(gs.samples([1, 2, 3], replacement=False), lambda xs: len(set(xs)) == len(xs))
    check_all(
        gs.samples([1, 2, 3], replacement=False, min_size=1, max_size=10),
        lambda xs: 1 <= len(xs) <= 3,
    )
    assert find_example(gs.samples([2, 1], min_size=2)) == [2, 2]
    assert (
        repr(gs.samples([2, 1], max_size=3, replacement=False))
        == "samples([2, 1], min_size=0, max_size=3, replacement=False)"
    )
    with pytest.raises(hegel.InvalidArgument):
        gs.samples([1], min_size=2, max_size=1)

    def empty(tc):
        tc.draw(gs.samples([], min_size=1))

    failing(empty, hegel.InvalidArgument)

    def too_many(tc):
        tc.draw(gs.samples([1], min_size=2, replacement=False))

    failing(too_many, hegel.InvalidArgument)


def test_large_collections_wrap_in_output():
    def body(tc):
        xs = tc.draw(gs.lists(gs.integers(10**6, 10**7), min_size=30, max_size=30))
        raise ValueError(xs)

    text = notes(failing(body, ValueError))[0]
    lines = text.splitlines()
    assert len(lines) > 3
    assert all(len(line) <= 79 for line in lines)
    assert lines[1].startswith("xs = [")
