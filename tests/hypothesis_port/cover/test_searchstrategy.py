# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_just_strategy_does_not_draw,
# test_none_strategy_does_not_draw, test_deferred_strategy_draw (ConjectureData
# internals); test_jsonable, test_jsonable_defaultdict, test_jsonable_namedtuple,
# test_jsonable_small_ints_are_ints, test_jsonable_large_ints_are_floats,
# test_jsonable_very_large_ints, test_jsonable_override,
# test_jsonable_to_json_nested, test_to_jsonable_handles_reference_cycles
# (to_jsonable, observability); test_use_of_global_random_is_deprecated_in_given,
# test_use_of_global_random_is_deprecated_in_interactive_draws (deprecation
# warnings).

import functools

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument, Unsatisfiable
from tests.hypothesis_port.common.debug import assert_simple_property, check_can_generate_examples


def test_or_errors_when_given_non_generator():
    bools = gs.tuples(gs.booleans())
    with pytest.raises(InvalidArgument):
        bools | "foo"


def test_just_strategy_uses_repr():
    class WeirdRepr:
        def __repr__(self):
            return "ABCDEFG"

    assert repr(gs.just(WeirdRepr())) == f"just({WeirdRepr()!r})"


def test_can_map():
    s = gs.integers().map(lambda t: "foo")
    assert_simple_property(s, lambda v: v == "foo")


@pytest.mark.xfail(
    strict=True,
    reason="with health checks suppressed a generator that rejects every draw passes silently; "
    "the engine only raises Unsatisfiable when no data is drawn (docs/engine-issues.md)",
)
def test_example_raises_unsatisfiable_when_too_filtered():
    with pytest.raises(Unsatisfiable):
        check_can_generate_examples(gs.integers().filter(lambda x: False))


def nameless_const(x):
    def f(u, v):
        return u

    return functools.partial(f, x)


def test_can_map_nameless():
    f = nameless_const(2)
    assert repr(f) in repr(gs.integers().map(f))


def test_can_flat_map_nameless():
    f = nameless_const(gs.just(3))
    assert repr(f) in repr(gs.integers().flat_map(f))


def test_flat_map_with_invalid_expand():
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.just(100).flat_map(lambda n: "a"))


def test_one_of_is_not_empty_when_an_operand_has_values():
    assert not (gs.nothing() | gs.integers()).is_empty
    assert (gs.nothing() | gs.nothing()).is_empty
    assert gs.nothing().is_empty
    assert not gs.integers().is_empty


@hegel.test
def test_or_draws_from_either_operand(tc):
    x = tc.draw(gs.just("a") | gs.just("b"))
    assert x in ("a", "b")
