# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_minimal_fractions_1,
# test_minimal_fractions_2, test_minimal_fractions_3 (fractions());
# test_dictionary's OrderedDict case (dict_class).
#
# minimal() is derandomized, so the seeds the original parametrized some
# tests over are passed to the search as its seed, and the @given-drawn
# parameters of others (which cannot start a nested run) are parametrized
# over fixed values instead.

from collections import Counter, namedtuple
from functools import reduce

import pytest

import hegel
from hegel import generators as gs
from hegel.generators import (
    booleans,
    builds,
    dicts,
    fixed_dicts,
    frozensets,
    integers,
    just,
    lists,
    none,
    sampled_from,
    sets,
    text,
    tuples,
)
from tests.hypothesis_port.common.debug import minimal


def test_integers_from_minimizes_leftwards():
    assert minimal(integers(min_value=101)) == 101


def test_minimize_bounded_integers_to_zero():
    assert minimal(integers(-10, 10)) == 0


def test_minimize_bounded_integers_to_positive():
    zero = 0

    def not_zero(x):
        return x != zero

    assert minimal(integers(-10, 10).filter(not_zero)) == 1


def test_minimize_string_to_empty():
    assert minimal(text()) == ""


def test_minimize_one_of():
    for _ in range(100):
        assert minimal(integers() | text() | booleans()) in (0, "", False)


def test_minimize_mixed_list():
    mixed = minimal(lists(integers() | text()), lambda x: len(x) >= 10)
    assert set(mixed).issubset({0, ""})


def test_minimize_longer_string():
    assert minimal(text(), lambda x: len(x) >= 10) == "0" * 10


def test_minimize_longer_list_of_strings():
    assert minimal(lists(text()), lambda x: len(x) >= 10) == [""] * 10


def test_minimize_3_set():
    assert minimal(sets(integers()), lambda x: len(x) >= 3) in ({0, 1, 2}, {-1, 0, 1})


def test_minimize_3_set_of_tuples():
    assert minimal(sets(tuples(integers())), lambda x: len(x) >= 2) == {(0,), (1,)}


def test_minimize_sets_of_sets():
    elements = integers(1, 100)
    size = 8
    set_of_sets = minimal(sets(frozensets(elements), min_size=size))
    assert frozenset() in set_of_sets
    assert len(set_of_sets) == size
    for s in set_of_sets:
        if len(s) > 1:
            assert any(s != t and t.issubset(s) for t in set_of_sets)


def test_minimize_sets_sampled_from():
    assert minimal(gs.sets(gs.sampled_from(range(10)), min_size=3)) == {0, 1, 2}


def test_can_simplify_flatmap_with_bounded_left_hand_size():
    assert (
        minimal(booleans().flat_map(lambda x: lists(just(x))), lambda x: len(x) >= 10)
        == [False] * 10
    )


def test_can_simplify_across_flatmap_of_just():
    assert minimal(integers().flat_map(just)) == 0


def test_can_simplify_on_right_hand_strategy_of_flatmap():
    assert minimal(integers().flat_map(lambda x: lists(just(x)))) == []


def test_can_ignore_left_hand_side_of_flatmap():
    assert (
        minimal(integers().flat_map(lambda x: lists(integers())), lambda x: len(x) >= 10)
        == [0] * 10
    )


def test_can_simplify_on_both_sides_of_flatmap():
    assert (
        minimal(integers().flat_map(lambda x: lists(just(x))), lambda x: len(x) >= 10) == [0] * 10
    )


def test_flatmap_rectangles():
    lengths = integers(min_value=0, max_value=10)

    def lists_of_length(n):
        return lists(sampled_from("ab"), min_size=n, max_size=n)

    xs = minimal(
        lengths.flat_map(lambda w: lists(lists_of_length(w))),
        lambda x: ["a", "b"] in x,
        test_cases=2000,
    )
    assert xs == [["a", "b"]]


def test_dictionary():
    assert minimal(dicts(keys=integers(), values=text())) == {}

    x = minimal(dicts(keys=integers(), values=text()), lambda t: len(t) >= 3)
    assert isinstance(x, dict)
    assert set(x.values()) == {""}
    for k in x:
        if k < 0:
            assert k + 1 in x
        if k > 0:
            assert k - 1 in x


def test_minimize_single_element_in_silly_large_int_range():
    assert minimal(integers(-(2**256), 2**256), lambda x: x >= -(2**255)) == 0


def test_minimize_multiple_elements_in_silly_large_int_range():
    actual = minimal(
        lists(integers(-(2**256), 2**256)),
        lambda x: len(x) >= 20,
        test_cases=10_000,
    )
    assert actual == [0] * 20


def test_minimize_multiple_elements_in_silly_large_int_range_min_is_not_dupe():
    target = list(range(20))

    actual = minimal(
        lists(integers(0, 2**256)),
        lambda x: len(x) >= 20 and all(x[i] >= target[i] for i in target),
    )
    assert actual == target


def test_find_large_union_list():
    size = 10

    def large_mostly_non_overlapping(xs):
        union = reduce(set.union, xs)
        return len(union) >= size

    result = minimal(
        lists(sets(integers(), min_size=1), min_size=1),
        large_mostly_non_overlapping,
    )
    assert len(result) == 1
    union = reduce(set.union, result)
    assert len(union) == size
    assert max(union) == min(union) + len(union) - 1


@pytest.mark.parametrize("n", [0, 1, 10, 100, 1000])
@pytest.mark.parametrize("seed", [13878544811291720918, 15832355027548327468, 12901656430307478246])
def test_containment(n, seed):
    iv = minimal(
        tuples(lists(integers()), integers()),
        lambda x: x[1] in x[0] and x[1] >= n,
        seed=seed,
    )
    assert iv == ([n], n)


def test_duplicate_containment():
    ls, i = minimal(
        tuples(lists(integers()), integers()),
        lambda s: s[0].count(s[1]) > 1,
    )
    assert ls == [0, 0]
    assert i == 0


@pytest.mark.parametrize("seed", [11, 28, 37])
def test_reordering_bytes(seed):
    ls = minimal(lists(integers()), lambda x: sum(x) >= 10 and len(x) >= 3, seed=seed)
    assert ls == sorted(ls)


def test_minimize_long_list():
    assert minimal(lists(booleans(), min_size=50), lambda x: len(x) >= 70) == [False] * 70


def test_minimize_list_of_longish_lists():
    size = 5
    xs = minimal(
        lists(lists(booleans())),
        lambda x: len([t for t in x if any(t) and len(t) >= 2]) >= size,
    )
    assert len(xs) == size
    for x in xs:
        assert x == [False, True]


def test_minimize_list_of_fairly_non_unique_ints():
    xs = minimal(lists(integers()), lambda x: len(set(x)) < len(x))
    assert len(xs) == 2


def test_list_with_complex_sorting_structure():
    xs = minimal(
        lists(lists(booleans())),
        lambda x: [list(reversed(t)) for t in x] > x and len(x) > 3,
    )
    assert len(xs) == 4


def test_list_with_wide_gap():
    xs = minimal(lists(integers()), lambda x: x and (max(x) > min(x) + 10 > 0))
    assert len(xs) == 2
    xs.sort()
    assert xs[1] == 11 + xs[0]


def test_minimize_namedtuple():
    T = namedtuple("T", ("a", "b"))
    tab = minimal(builds(T, integers(), integers()), lambda x: x.a < x.b)
    assert tab.b == tab.a + 1


def test_minimize_dict():
    tab = minimal(
        fixed_dicts({"a": booleans(), "b": booleans()}),
        lambda x: x["a"] or x["b"],
    )
    assert not (tab["a"] and tab["b"])


def test_minimize_list_of_sets():
    assert minimal(lists(sets(booleans())), lambda x: len(list(filter(None, x))) >= 3) == (
        [{False}] * 3
    )


def test_minimize_list_of_lists():
    assert minimal(lists(lists(integers())), lambda x: len(list(filter(None, x))) >= 3) == (
        [[0]] * 3
    )


def test_minimize_list_of_tuples():
    xs = minimal(lists(tuples(integers(), integers())), lambda x: len(x) >= 2)
    assert xs == [(0, 0), (0, 0)]


def test_minimize_multi_key_dicts():
    assert minimal(dicts(keys=booleans(), values=booleans()), bool) == {False: False}


def test_multiple_empty_lists_are_independent():
    x = minimal(lists(lists(none(), max_size=0)), lambda t: len(t) >= 2)
    u, v = x
    assert u is not v


def test_can_find_sets_unique_by_incomplete_data():
    size = 5
    ls = minimal(lists(tuples(integers(), integers()), unique_by=max), lambda x: len(x) >= size)
    assert len(ls) == size
    values = sorted(map(max, ls))
    assert values[-1] - values[0] == size - 1
    for u, _ in ls:
        assert u <= 0


@pytest.mark.parametrize("n", range(10))
def test_lists_forced_near_top(n):
    assert minimal(lists(integers(), min_size=n, max_size=n + 2), lambda t: len(t) == n + 2) == [
        0
    ] * (n + 2)


def test_sum_of_pair_int():
    assert minimal(tuples(integers(0, 1000), integers(0, 1000)), lambda x: sum(x) > 1000) == (
        1,
        1000,
    )


def test_sum_of_pair_float():
    assert minimal(tuples(gs.floats(0, 1000), gs.floats(0, 1000)), lambda x: sum(x) > 1000) == (
        1.0,
        1000.0,
    )


def test_sum_of_pair_mixed():
    # check both orderings
    assert minimal(tuples(gs.floats(0, 1000), gs.integers(0, 1000)), lambda x: sum(x) > 1000) == (
        1.0,
        1000,
    )
    assert minimal(tuples(gs.integers(0, 1000), gs.floats(0, 1000)), lambda x: sum(x) > 1000) == (
        1,
        1000.0,
    )


def test_sum_of_pair_separated_int():
    @hegel.composite
    def separated_sum(tc):
        n1 = tc.draw(gs.integers(0, 1000))
        tc.draw(gs.text())
        tc.draw(gs.booleans())
        tc.draw(gs.integers())
        n2 = tc.draw(gs.integers(0, 1000))
        return (n1, n2)

    assert minimal(separated_sum(), lambda x: sum(x) > 1000) == (1, 1000)


def test_sum_of_pair_separated_float():
    @hegel.composite
    def separated_sum(tc):
        f1 = tc.draw(gs.floats(0, 1000))
        tc.draw(gs.text())
        tc.draw(gs.booleans())
        tc.draw(gs.integers())
        f2 = tc.draw(gs.floats(0, 1000))
        return (f1, f2)

    assert minimal(separated_sum(), lambda x: sum(x) > 1000) == (1, 1000)


def test_calculator_benchmark():
    """This test comes from
    https://github.com/jlink/shrinking-challenge/blob/main/challenges/calculator.md,
    which is originally from Pike, Lee. "SmartCheck: automatic and efficient
    counterexample reduction and generalization."
    Proceedings of the 2014 ACM SIGPLAN symposium on Haskell. 2014.
    """

    expression = gs.deferred(
        lambda: gs.one_of(
            gs.integers(),
            gs.tuples(gs.just("+"), expression, expression),
            gs.tuples(gs.just("/"), expression, expression),
        )
    )

    def div_subterms(e):
        if isinstance(e, int):
            return True
        if e[0] == "/" and e[-1] == 0:
            return False
        return div_subterms(e[1]) and div_subterms(e[2])

    def evaluate(e):
        if isinstance(e, int):
            return e
        elif e[0] == "+":
            return evaluate(e[1]) + evaluate(e[2])
        else:
            assert e[0] == "/"
            return evaluate(e[1]) // evaluate(e[2])

    def is_failing(e):
        if not div_subterms(e):
            return False
        try:
            evaluate(e)
            return False
        except ZeroDivisionError:
            return True

    x = minimal(expression, is_failing)

    assert x == ("/", 0, ("+", 0, 0))


def test_one_of_slip():
    assert minimal(gs.integers(101, 200) | gs.integers(0, 100)) == 101


@pytest.mark.parametrize("n", [0, 1, -1, 2**31 + 12345, -(2**32)])
def test_perfectly_shrinks_integers(n):
    if n >= 0:
        assert minimal(gs.integers(), lambda x: x >= n) == n
    else:
        assert minimal(gs.integers(), lambda x: x <= n) == n


@pytest.mark.parametrize("gap", [0, 1, 7, 20])
def test_lowering_together_positive(gap):
    s = gs.tuples(gs.integers(0, 20), gs.integers(0, 20))
    assert minimal(s, lambda x: x[0] + gap == x[1], test_cases=2000) == (0, gap)


@pytest.mark.parametrize("gap", [0, -1, -7, -20])
def test_lowering_together_negative(gap):
    s = gs.tuples(gs.integers(-20, 0), gs.integers(-20, 0))
    assert minimal(s, lambda x: x[0] + gap == x[1], test_cases=2000) == (0, gap)


@pytest.mark.parametrize("gap", [0, 1, -1, 7, -10])
def test_lowering_together_mixed(gap):
    s = gs.tuples(gs.integers(-10, 10), gs.integers(-10, 10))
    assert minimal(s, lambda x: x[0] + gap == x[1], test_cases=2000) == (0, gap)


@pytest.mark.parametrize("gap", [0, 1, -1, 7, -10])
def test_lowering_together_with_gap(gap):
    s = gs.tuples(gs.integers(-10, 10), gs.text(), gs.floats(), gs.integers(-10, 10))
    assert minimal(s, lambda x: x[0] + gap == x[3], test_cases=2000) == (0, "", 0.0, gap)


def test_run_length_encoding():
    # extracted from https://github.com/HypothesisWorks/hypothesis/issues/4286,
    # as well as our docs

    def decode(table: list[tuple[int, str]]) -> str:
        out = ""
        for count, char in table:
            out += count * char
        return out

    def encode(s: str) -> list[tuple[int, str]]:
        count = 1
        prev = ""
        out = []

        if not s:
            return []

        for char in s:
            if char != prev:
                if prev:
                    entry = (count, prev)
                    out.append(entry)
                # BUG:
                # count = 1
                prev = char
            else:
                count += 1

        entry = (count, char)
        out.append(entry)
        return out

    assert minimal(gs.text(), lambda s: decode(encode(s)) != s) == "001"


def test_minimize_duplicated_characters_within_a_choice():
    # look for strings which have at least 3 of the same character, and also at
    # least two different characters (to avoid the trivial shrink of replacing
    # everything with "0" from working).

    # we should test this for binary too, but it's difficult to get it
    # to satisfy this precondition in the first place (probably worth improving
    # our generation here to duplicate binary elements in generate_mutations_from)
    assert (
        minimal(
            gs.text(min_size=1),
            lambda v: Counter(v).most_common()[0][1] > 2 and len(set(v)) > 1,
        )
        == "0001"
    )


def test_nasty_string_shrinks():
    # failures found via NASTY_STRINGS should shrink like normal
    assert minimal(gs.text(), lambda s: "𝕿𝖍𝖊" in s, test_cases=10_000) == "𝕿𝖍𝖊"


def test_shrink_text_differs_from_lower_to_ascii():
    # Regression test for https://github.com/HypothesisWorks/hypothesis/issues/4725
    # Text shrinking would previously sometimes get stuck on a high-codepoint
    # accented letter such as 'À' instead of converging to 'A'.
    assert minimal(gs.text(min_size=1), lambda s: s != s.lower()) == "A"


def test_shrink_text_differs_from_upper_to_ascii():
    assert minimal(gs.text(min_size=1), lambda s: s != s.upper()) == "a"


def test_shrink_strips_accent_to_ascii_letter():
    # Removing the accent from any accented latin letter should shrink that
    # letter to its base ascii form.
    assert minimal(gs.text(min_size=1), lambda s: "e" in s.lower() or "E" in s) == "E"


def test_shrink_decomposes_compatibility_form_to_ascii():
    # A character that NFKD-decomposes to an ascii letter (eg a Mathematical
    # Bold Capital T, or a ligature) should reduce to that base letter when
    # the predicate also matches the base letter.
    assert minimal(gs.text(min_size=1), lambda s: any(c.lower() == "t" for c in s)) == "T"


def test_shrink_ligature_to_base_character():
    # The ligature ﬁ NFKD-decomposes to "fi"; when the predicate accepts any
    # 'f' character, we should reduce to plain "F".
    assert minimal(gs.text(min_size=1), lambda s: any(c.lower() == "f" for c in s)) == "F"


def test_bound5():
    # redistribute_numeric_pairs should work for negative integers too
    bounded_ints = gs.lists(gs.integers(-100, 0), max_size=1)

    s = gs.tuples(
        bounded_ints,
        bounded_ints,
        bounded_ints,
        bounded_ints,
        bounded_ints,
    )
    assert minimal(s, lambda v: sum(sum(v, []), 0) < -150) == (
        [],
        [],
        [],
        [-51],
        [-100],
    )
