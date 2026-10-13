# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_posonly_lambda_formatting (lambda
# source descriptions); test_does_not_convert_posonly_to_keyword
# (hypothesis.internal helpers); test_given_works_with_keyword_only_params,
# test_given_works_with_keyword_only_params_some_unbound,
# test_given_works_with_positional_only_params,
# test_cannot_pass_strategies_by_position_if_there_are_posonly_args,
# test_cannot_pass_strategies_for_posonly_args, test_example_argument_validation
# (@given and @example argument binding); test_typing_Final (from_type does not
# resolve Final); test_can_resolve_recursive_dataclass (recursive types).

import typing
from types import SimpleNamespace

import pytest

import hegel
from hegel import generators as gs
from hegel.generators import from_type
from tests.hypothesis_port.common.debug import (
    assert_simple_property,
    check_can_generate_examples,
    find_any,
)
from tests.hypothesis_port.common.utils import temp_registered


@pytest.mark.parametrize("value", ["dog", b"goldfish", 42, 63.4, -80.5, False])
def test_typing_Literal(value):
    assert_simple_property(from_type(typing.Literal[value]), lambda v: v == value)


@hegel.test
def test_typing_Literal_nested(tc):
    lit = typing.Literal
    values = [
        (lit["hamster", 0], ("hamster", 0)),
        (lit[26, False, "bunny", 130], (26, False, "bunny", 130)),
        (lit[lit[1]], {1}),
        (lit[lit[1], 2], {1, 2}),
        (lit[1, lit[2], 3], {1, 2, 3}),
        (lit[lit[lit[1], lit[2]], lit[lit[3], lit[4]]], {1, 2, 3, 4}),
    ]
    literal_type, flattened_literals = tc.draw(gs.sampled_from(values))
    assert tc.draw(from_type(literal_type)) in flattened_literals


class A(typing.TypedDict):
    a: int


@hegel.test
def test_simple_typeddict(tc):
    value = tc.draw(from_type(A))
    assert type(value) is dict
    assert set(value) == {"a"}
    assert isinstance(value["a"], int)


class B(A, total=False):
    # a is required, b is optional
    b: bool


@hegel.test
def test_typeddict_with_optional(tc):
    value = tc.draw(from_type(B))
    assert type(value) is dict
    assert set(value).issubset({"a", "b"})
    assert isinstance(value["a"], int)
    if "b" in value:
        assert isinstance(value["b"], bool)


def test_simple_optional_key_is_optional():
    # Optional keys are not currently supported, as PEP-589 leaves no traces
    # at runtime.  See https://github.com/python/cpython/pull/17214
    find_any(from_type(B), lambda d: "b" not in d)


class C(B):
    # a is required, b is optional, c is required again
    c: str


@hegel.test
def test_typeddict_with_optional_then_required_again(tc):
    value = tc.draw(from_type(C))
    assert type(value) is dict
    assert set(value).issubset({"a", "b", "c"})
    assert isinstance(value["a"], int)
    if "b" in value:
        assert isinstance(value["b"], bool)
    assert isinstance(value["c"], str)


class NestedDict(typing.TypedDict):
    inner: A


@hegel.test
def test_typeddict_with_nested_value(tc):
    value = tc.draw(from_type(NestedDict))
    assert type(value) is dict
    assert set(value) == {"inner"}
    assert isinstance(value["inner"]["a"], int)


def test_layered_optional_key_is_optional():
    # Optional keys are not currently supported, as PEP-589 leaves no traces
    # at runtime.  See https://github.com/python/cpython/pull/17214
    find_any(from_type(C), lambda d: "b" not in d)


def test_can_register_new_type_for_typeddicts():
    sentinel = object()
    with temp_registered(C, gs.just(sentinel)):
        assert_simple_property(from_type(C), lambda v: v is sentinel)


class FooProtocol(typing.Protocol):
    def frozzle(self, x):
        pass


class BarProtocol(typing.Protocol):
    def bazzle(self, y):
        pass


@hegel.test
def test_can_resolve_registered_protocol(tc):
    with temp_registered(
        FooProtocol,
        gs.builds(SimpleNamespace, frozzle=gs.just(lambda x: None)),
    ):
        obj = tc.draw(from_type(FooProtocol))
    assert obj.frozzle(x=1) is None


def test_cannot_resolve_un_registered_protocol():
    with pytest.raises(TypeError, match="[Pp]rotocol"):
        check_can_generate_examples(from_type(BarProtocol))
