# Dropped from the Hypothesis original: test_sampled_repr_leaves_range_as_range
# (sampled_from of a huge range); test_as_created_reprs_fallback_for_distinct_calls_same_obj
# (Hypothesis's identity tracking of values returned by distinct calls).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import hashlib

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import notes


def _get_output(test_fn):
    with pytest.raises(AssertionError) as err:
        test_fn()
    return "\n".join(notes(err.value)).strip()


def test_includes_non_default_args_in_repr():
    assert repr(gs.integers()) == "integers()"
    assert repr(gs.integers(min_value=1)) == "integers(min_value=1)"


def hi(there, stuff):
    return there


def test_supports_positional_and_keyword_args_in_builds():
    # Hegel quotes the target's name.
    assert (
        repr(gs.builds(hi, gs.integers(), there=gs.booleans()))
        == "builds('hi', integers(), there=booleans())"
    )


@pytest.mark.xfail(
    strict=True, reason="sampled_from copies its elements into a list, so its repr shows a list"
)
def test_preserves_sequence_type_of_argument():
    assert repr(gs.sampled_from([0, 1])) == "sampled_from([0, 1])"
    assert repr(gs.sampled_from((0, 1))) == "sampled_from((0, 1))"


class IHaveABadRepr:
    def __repr__(self):
        raise ValueError("Oh no!")


def test_errors_are_deferred_until_repr_is_calculated():
    s = (
        gs.builds(
            lambda x, y: 1,
            gs.just(IHaveABadRepr()),
            y=gs.one_of(gs.sampled_from((IHaveABadRepr(),)), gs.just(IHaveABadRepr())),
        )
        .map(lambda t: t)
        .filter(lambda t: True)
        .flat_map(lambda t: gs.just(IHaveABadRepr()))
    )

    with pytest.raises(ValueError):
        repr(s)


@hegel.test
def test_iterables_repr_is_useful(tc):
    it = tc.draw(gs.iterables(gs.integers()))
    # fairly hard-coded but useful; also ensures the values are inexhaustible
    assert repr(it) == f"iter({list(it)!r})"


class Foo:
    def __init__(self, x: int) -> None:
        self.x = x


class Bar(Foo):
    pass


def test_reprs_as_created():
    @hegel.test(print_blob=False, test_cases=10_000, derandomize=True)
    def inner(tc):
        foo = tc.draw(gs.builds(Foo))
        bar = tc.draw(gs.from_type(Bar))
        baz = tc.draw(gs.none().map(Foo))
        assert baz.x is None
        assert foo.x <= 0 or bar.x >= 0

    output = _get_output(inner)
    assert output.startswith("Falsifying example:\nfoo = Foo(x=1)\nbar = Bar(x=-1)\nbaz = <")


def test_reprs_as_created_interactive():
    @hegel.test(print_blob=False, test_cases=10_000)
    def inner(tc):
        bar = tc.draw(gs.builds(Bar, gs.just(10)))
        assert not bar.x

    assert _get_output(inner) == "Falsifying example:\nbar = Bar(10)"


CONSTANT_FOO = Foo(None)


def some_foo(*_):
    return CONSTANT_FOO


def test_reprs_as_created_consistent_calls_despite_indentation():
    aas = "a" * 60
    strat = gs.builds(some_foo, gs.just(aas))

    # If we have multiple calls which return the same object, we can print their
    # nice repr even if varying indentation means that they'll come out different!
    @hegel.test(print_blob=False, test_cases=10_000)
    def inner(tc):
        a = tc.draw(strat)
        b = tc.draw(gs.builds(Bar, strat))
        assert a == b

    assert _get_output(inner) == (
        f"Falsifying example:\na = some_foo({aas!r})\nb = Bar(some_foo({aas!r}))"
    )


_no_arg_repr = pytest.mark.xfail(
    strict=True, reason="Hegel's characters() generator does not repr its arguments"
)


@pytest.mark.parametrize(
    "strategy, expected_repr",
    [
        (gs.characters(), "characters()"),
        pytest.param(gs.characters(codec="utf-8"), "characters(codec='utf-8')", marks=_no_arg_repr),
        pytest.param(
            gs.characters(min_codepoint=65), "characters(min_codepoint=65)", marks=_no_arg_repr
        ),
        pytest.param(
            gs.characters(max_codepoint=127), "characters(max_codepoint=127)", marks=_no_arg_repr
        ),
        pytest.param(
            gs.characters(categories=["Lu", "Ll"]),
            "characters(categories=('Lu', 'Ll'))",
            marks=_no_arg_repr,
        ),
        pytest.param(
            gs.characters(exclude_characters="abc"),
            "characters(exclude_characters='abc')",
            marks=_no_arg_repr,
        ),
        pytest.param(
            gs.characters(min_codepoint=65, max_codepoint=90),
            "characters(min_codepoint=65, max_codepoint=90)",
            marks=_no_arg_repr,
        ),
        pytest.param(
            gs.characters(codec="ascii", min_codepoint=32, max_codepoint=126),
            "characters(min_codepoint=32, max_codepoint=126)",
            marks=_no_arg_repr,
        ),
        pytest.param(
            gs.characters(categories=["Lu"], exclude_characters="AZ"),
            "characters(categories=('Lu',), exclude_characters='AZ')",
            marks=_no_arg_repr,
        ),
    ],
)
def test_characters_repr(strategy, expected_repr):
    assert repr(strategy) == expected_repr


def test_map_to_str_prints_as_repr():
    @hegel.test(phases=[hegel.Phase.generate, hegel.Phase.shrink], print_blob=False)
    def inner(tc):
        s = tc.draw(gs.integers().map(str))  # noqa: F841 (names the draw)
        raise AssertionError

    assert _get_output(inner) == "Falsifying example:\ns = '0'"


def test_map_to_bytes_prints_as_repr():
    @hegel.test(phases=[hegel.Phase.generate, hegel.Phase.shrink], print_blob=False)
    def inner(tc):
        b = tc.draw(gs.binary().map(lambda b: hashlib.sha256(b).digest()))  # noqa: F841
        raise AssertionError

    digest = hashlib.sha256(b"").digest()
    assert _get_output(inner) == f"Falsifying example:\nb = {digest!r}"


def test_invalid_call_syntax_falls_back_to_repr():
    # When the call-style repr (e.g. "a b()") is invalid Python syntax but
    # repr(obj) is valid, Hypothesis falls back to using repr(obj); Hegel
    # prints the call regardless.
    _BadName = type(
        "a b",
        (),
        {"__repr__": lambda self: "'hello world'"},
    )

    @hegel.test(phases=[hegel.Phase.generate, hegel.Phase.shrink], print_blob=False)
    def inner(tc):
        x = tc.draw(gs.builds(_BadName))  # noqa: F841 (names the draw)
        raise AssertionError

    assert _get_output(inner) == "Falsifying example:\nx = a b()"
