# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import functools
import random
import sys
from collections import defaultdict, namedtuple
from dataclasses import dataclass
from typing import Any

# Dropped (ConjectureData and to_jsonable are Hypothesis internals): test_just_strategy_does_not_draw, test_none_strategy_does_not_draw, test_deferred_strategy_draw, test_jsonable, test_jsonable_defaultdict, test_jsonable_namedtuple, test_jsonable_small_ints_are_ints, test_jsonable_large_ints_are_floats, test_jsonable_very_large_ints, test_jsonable_override, test_jsonable_to_json_nested, test_to_jsonable_handles_reference_cycles.


import pytest

from hegel.hypothesis import given, strategies as st
from hegel.hypothesis.errors import InvalidArgument, Unsatisfiable
from tests.hypothesis_port.common.compat import UniqueIdentifier, get_pretty_function_description

from tests.hypothesis_port.common.debug import assert_simple_property, check_can_generate_examples
from tests.hypothesis_port.common.utils import checks_deprecated_behaviour


def test_or_errors_when_given_non_strategy():
    bools = st.tuples(st.booleans())
    with pytest.raises(ValueError):
        bools | "foo"


SomeNamedTuple = namedtuple("SomeNamedTuple", ("a", "b"))


def last(xs):
    t = None
    for x in xs:
        t = x
    return t


def test_just_strategy_uses_repr():
    class WeirdRepr:
        def __repr__(self):
            return "ABCDEFG"

    assert repr(st.just(WeirdRepr())) == f"just({WeirdRepr()!r})"


def test_can_map():
    s = st.integers().map(pack=lambda t: "foo")
    assert_simple_property(s, lambda v: v == "foo")


def test_example_raises_unsatisfiable_when_too_filtered():
    with pytest.raises(Unsatisfiable):
        check_can_generate_examples(st.integers().filter(lambda x: False))


def nameless_const(x):
    def f(u, v):
        return u

    return functools.partial(f, x)


def test_can_map_nameless():
    f = nameless_const(2)
    assert get_pretty_function_description(f) in repr(st.integers().map(f))


def test_can_flatmap_nameless():
    f = nameless_const(st.just(3))
    assert get_pretty_function_description(f) in repr(st.integers().flatmap(f))


def test_flatmap_with_invalid_expand():
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(st.just(100).flatmap(lambda n: "a"))


_bad_random_strategy = st.lists(st.integers(), min_size=1).map(random.choice)


@checks_deprecated_behaviour
def test_use_of_global_random_is_deprecated_in_given():
    check_can_generate_examples(_bad_random_strategy)


@checks_deprecated_behaviour
def test_use_of_global_random_is_deprecated_in_interactive_draws():
    @given(st.data())
    def inner(d):
        d.draw(_bad_random_strategy)

    inner()


@dataclass
class HasDefaultDict:
    x: defaultdict


@dataclass
class HasCustomJsonFormat:
    x: str

    def to_json(self):
        return "surprise!"


@dataclass
class Inner:
    value: int

    def to_json(self):
        return "custom"


@dataclass
class Outer:
    inner: Inner


recursive_list = []
recursive_list.append(recursive_list)

recursive_dict = {}
recursive_dict["a"] = recursive_dict

mutual1 = []
mutual2 = [mutual1]
mutual1.append(mutual2)

shared = UniqueIdentifier("shared")


@dataclass
class A:
    a: Any
    b: Any
