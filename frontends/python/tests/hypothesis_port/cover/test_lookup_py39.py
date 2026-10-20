# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_annotated_positive_int,
# test_annotated_more_than_ten, test_annotated_with_two_strategies,
# test_annotated_extra_metadata, test_grouped_protocol_strategy (generators in
# Annotated metadata); test_can_register_builtin_list, test_lookup_registered_tuple
# (registering generic aliases); test_string_forward_ref_resolved (recursive
# types); test_collections_abc_callable_none (from_type does not resolve Callable).

import dataclasses
import typing

import pytest

import hegel
from hegel import generators as gs
from hegel.generators import from_type
from tests.hypothesis_port.common.debug import assert_all_examples, find_any
from tests.hypothesis_port.common.utils import temp_registered

# Union[A, B] is not equivalent to A | B until 3.14. We'll continue to test both
# until then.
# ruff: noqa: UP007


@pytest.mark.parametrize(
    "annotated_type,plain_type",
    [
        (typing.Annotated[int, "foo"], int),
        (typing.Annotated[list[float], "foo"], list[float]),
        (typing.Annotated[typing.Annotated[str, "foo"], "bar"], str),
        (
            typing.Annotated[typing.Annotated[list[dict[str, bool]], "foo"], "bar"],
            list[dict[str, bool]],
        ),
    ],
)
def test_typing_Annotated(annotated_type, plain_type):
    assert repr(from_type(annotated_type)) == repr(from_type(plain_type))


@pytest.mark.parametrize("typ", (typing.Union[list[int], int], list[int] | int))
def test_issue_3080(typ):
    # Check for https://github.com/HypothesisWorks/hypothesis/issues/3080
    s = from_type(typ)
    find_any(s, lambda x: isinstance(x, int))
    find_any(s, lambda x: isinstance(x, list))


@dataclasses.dataclass
class TypingTuple:
    a: dict[tuple[int, int], str]


@dataclasses.dataclass
class BuiltinTuple:
    a: dict[tuple[int, int], str]


TestDataClass = typing.Union[TypingTuple, BuiltinTuple]


@pytest.mark.parametrize("data_class", [TypingTuple, BuiltinTuple])
@hegel.test
def test_from_type_with_tuple_works(tc, data_class: TestDataClass):
    value: TestDataClass = tc.draw(from_type(data_class))
    assert len(value.a) >= 0


T = typing.TypeVar("T")


@typing.runtime_checkable
class Fooable(typing.Protocol[T]):
    def foo(self): ...


class FooableConcrete(tuple):
    def foo(self):
        pass


def test_only_tuple_subclasses_in_typing_type():
    # A generic typing type (such as Fooable) whose only concrete
    # instantiations are tuples should still generate tuples. This is in
    # contrast to test_tuple_subclasses_not_generic_sequences, which discards
    # tuples if there are any alternatives.
    with temp_registered(FooableConcrete, gs.builds(FooableConcrete)):
        s = from_type(Fooable[int])
        assert_all_examples(s, lambda x: type(x) is FooableConcrete)
