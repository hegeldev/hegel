# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_given_can_infer_from_manual_annotations,
# test_infer_all (@given argument binding and infer);
# test_does_not_add_param_empty_to_type_hints (hypothesis.internal.compat.get_type_hints);
# test_can_generate_from_all_registered_types for the bare abstract origins
# Collection, Iterable, Iterator, Mapping, MutableSet and Set (from_type never instantiates
# them); test_generic_origin_empty (from_type builds an unregistered generic's origin rather
# than failing); test_generic_origin_with_type_args (register_type_generator does not reject
# parametrised generics as keys).

import abc
import enum
import inspect
import typing
from collections.abc import Callable, Sequence
from typing import Generic, TypeVar
from typing import List as _List

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument, ResolutionFailed
from hegel.generators.from_type import _registry
from tests.hypothesis_port.common import types
from tests.hypothesis_port.common.debug import (
    assert_all_examples,
    assert_simple_property,
    check_can_generate_examples,
    find_any,
)
from tests.hypothesis_port.common.types import _all_generators, _global_type_lookup
from tests.hypothesis_port.common.utils import temp_registered

# we'll continue testing the typing variants until their removal from the stdlib
# ruff: noqa: UP006, UP035, UP007

# typing.Any is a class on 3.11+, but nothing is an instance of it.
types_with_core_strat = {typ for typ in types.core_types if typ is not typing.Any}


@pytest.mark.parametrize("typ", sorted(types_with_core_strat, key=str))
def test_resolve_core_strategies(typ):
    @hegel.test
    def inner(tc):
        ex = tc.draw(gs.from_type(typ))
        assert isinstance(ex, typ)

    inner()


def test_generic_sequence_of_integers_may_be_lists_or_bytes():
    strat = gs.from_type(Sequence[int])
    find_any(strat, lambda x: isinstance(x, bytes))
    find_any(strat, lambda x: isinstance(x, list))


def test_lookup_knows_about_all_core_strategies():
    # Build a set of all types output by core generators
    blocklist = {
        "builds",
        "deferred",
        "from_regex",
        "from_type",
        "ip_addresses",
        "iterables",
        "just",
        "nothing",
        "one_of",
        "permutations",
        "recursive",
        "sampled_from",
        "timezones",
    }
    assert set(_all_generators).issuperset(blocklist), blocklist.difference(_all_generators)
    found = set()
    for thing in (
        getattr(gs, name)
        for name in sorted(_all_generators)
        if name in dir(gs) and name not in blocklist
    ):
        for n in range(3):
            try:
                ex = find_any(thing(*([gs.nothing()] * n)))
                found.add(type(ex))
                break
            except Exception:
                continue

    cannot_lookup = found - set(types._global_type_lookup)
    assert not cannot_lookup


def test_lookup_keys_are_types():
    try:
        with pytest.raises(InvalidArgument):
            gs.register_type_generator("int", gs.integers())
        assert "int" not in types._global_type_lookup
    finally:
        _registry.pop("int", None)


@pytest.mark.parametrize(
    "typ, not_a_strategy",
    [
        (int, 42),  # Values must be generators
        # Can't register NotImplemented directly, even though generator functions
        # can return it.
        (int, NotImplemented),
    ],
)
def test_lookup_values_are_strategies(typ, not_a_strategy):
    with pytest.raises(InvalidArgument):
        gs.register_type_generator(typ, not_a_strategy)
    assert not_a_strategy not in types._global_type_lookup.values()


@pytest.mark.parametrize("typ", sorted(types_with_core_strat, key=str))
def test_lookup_overrides_defaults(typ):
    sentinel = object()
    with temp_registered(typ, gs.just(sentinel)):
        assert_simple_property(gs.from_type(typ), lambda v: v is sentinel)
    assert_simple_property(gs.from_type(typ), lambda v: v is not sentinel)


class ParentUnknownType:
    pass


class UnknownType(ParentUnknownType):
    def __init__(self, arg):
        pass


def test_custom_type_resolution_fails_without_registering():
    with pytest.raises(ResolutionFailed):
        check_can_generate_examples(gs.from_type(UnknownType))


def test_custom_type_resolution():
    sentinel = object()
    with temp_registered(UnknownType, gs.just(sentinel)):
        assert_simple_property(gs.from_type(UnknownType), lambda v: v is sentinel)
        # Also covered by registration of child class
        assert_simple_property(gs.from_type(ParentUnknownType), lambda v: v is sentinel)


def test_custom_type_resolution_with_function():
    sentinel = object()
    with temp_registered(UnknownType, lambda _: gs.just(sentinel)):
        assert_simple_property(gs.from_type(UnknownType), lambda v: v is sentinel)
        assert_simple_property(gs.from_type(ParentUnknownType), lambda v: v is sentinel)


def test_custom_type_resolution_with_function_non_strategy():
    with temp_registered(UnknownType, lambda _: None):
        with pytest.raises(ResolutionFailed):
            check_can_generate_examples(gs.from_type(UnknownType))
        with pytest.raises(ResolutionFailed):
            check_can_generate_examples(gs.from_type(ParentUnknownType))


@pytest.mark.parametrize("strategy_returned", [True, False])
def test_conditional_type_resolution_with_function(strategy_returned):
    sentinel = object()

    def resolve_strategy(thing):
        assert thing == UnknownType
        if strategy_returned:
            return gs.just(sentinel)
        return NotImplemented

    with temp_registered(UnknownType, resolve_strategy):
        if strategy_returned:
            assert_simple_property(gs.from_type(UnknownType), lambda v: v is sentinel)
        else:
            with pytest.raises(ResolutionFailed):
                check_can_generate_examples(gs.from_type(UnknownType))


def test_errors_if_generic_resolves_empty():
    with temp_registered(UnknownType, lambda _: gs.nothing()):
        with pytest.raises(ResolutionFailed):
            check_can_generate_examples(gs.from_type(UnknownType))
        with pytest.raises(ResolutionFailed):
            check_can_generate_examples(gs.from_type(ParentUnknownType))


def test_cannot_register_empty():
    # Cannot register and did not register
    try:
        with pytest.raises(InvalidArgument):
            gs.register_type_generator(UnknownType, gs.nothing())
        with pytest.raises(ResolutionFailed):
            check_can_generate_examples(gs.from_type(UnknownType))
        assert UnknownType not in types._global_type_lookup
    finally:
        _registry.pop(UnknownType, None)


def test_pulic_interface_works():
    check_can_generate_examples(gs.from_type(int))
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.from_type("not a type or annotated function"))


class EmptyEnum(enum.Enum):
    pass


def test_error_if_enum_is_empty():
    with pytest.raises(InvalidArgument):
        gs.from_type(EmptyEnum)


class BrokenClass:
    __init__ = "Hello!"


def test_uninspectable_builds():
    with pytest.raises(TypeError, match="object is not callable"):
        check_can_generate_examples(gs.builds(BrokenClass))


def test_uninspectable_from_type():
    # Hypothesis falls through to builds() and reports its TypeError; from_type
    # reports the unusable signature itself.
    with pytest.raises(ResolutionFailed, match="no usable signature"):
        check_can_generate_examples(gs.from_type(BrokenClass))


def _check_instances(t):
    # See https://github.com/samuelcolvin/pydantic/discussions/2508
    return (
        t.__module__ != "typing"
        and t.__name__ != "ByteString"
        and not t.__module__.startswith("pydantic")
        and t.__module__ != "typing_extensions"
    )


@pytest.mark.parametrize(
    "typ",
    [
        x
        for x in sorted((x for x in _global_type_lookup if _check_instances(x)), key=str)
        if not inspect.isabstract(x)
    ],
)
def test_can_generate_from_all_registered_types(typ):
    @hegel.test
    def inner(tc):
        value = tc.draw(gs.from_type(typ), name="value")
        assert isinstance(value, typ)

    inner()


T = TypeVar("T")


class MyGeneric(Generic[T]):
    def __init__(self, arg: T) -> None:
        self.arg = arg


class Lines(Sequence[str]):
    """Represent a sequence of text lines.

    It turns out that resolving a class which inherits from a parametrised generic
    type is... tricky.  See https://github.com/HypothesisWorks/hypothesis/issues/2951
    """


class SpecificDict(dict[int, int]):
    pass


def using_generic(instance: MyGeneric[T]) -> T:
    return instance.arg


def using_concrete_generic(instance: MyGeneric[int]) -> int:
    return instance.arg


def test_issue_2951_regression():
    lines_strat = gs.builds(Lines, lines=gs.lists(gs.text()))
    prev_seq_int_repr = repr(gs.from_type(Sequence[int]))
    with temp_registered(Lines, lines_strat):
        assert gs.from_type(Lines) == lines_strat
        # Now let's test that the generator for ``Sequence[int]`` did not
        # change just because we registered a generator for ``Lines``:
        assert repr(gs.from_type(Sequence[int])) == prev_seq_int_repr


def test_issue_2951_regression_two_params():
    map_strat = gs.builds(SpecificDict, gs.dicts(gs.integers(), gs.integers()))
    expected = repr(gs.from_type(dict[int, int]))
    with temp_registered(SpecificDict, map_strat):
        assert gs.from_type(SpecificDict) == map_strat
        assert expected == repr(gs.from_type(dict[int, int]))


@pytest.mark.parametrize(
    "generic",
    (
        Callable,
        list,
        Sequence,
        # you can register types with all generic parameters
        _List[T],
        getattr(typing, "Sequence", None)[T],  # pyupgrade workaround
        list[T],
        Sequence[T],
        # User-defined generics should also work
        MyGeneric,
        MyGeneric[T],
    ),
)
def test_generic_origin_without_type_args(generic):
    with temp_registered(generic, gs.just("example")):
        pass


@pytest.mark.parametrize(
    "strat, type_",
    [
        (gs.from_type, MyGeneric[T]),
        (gs.from_type, MyGeneric[int]),
        (gs.from_type, MyGeneric),
        (gs.builds, using_generic),
        (gs.builds, using_concrete_generic),
    ],
    ids=repr,
)
def test_generic_origin_from_type(strat, type_):
    with temp_registered(MyGeneric, gs.builds(MyGeneric)):
        check_can_generate_examples(strat(type_))


def test_generic_origin_concrete_builds():
    with temp_registered(MyGeneric, gs.builds(MyGeneric, gs.integers())):
        assert_all_examples(gs.builds(using_generic), lambda example: isinstance(example, int))


class AbstractFoo(abc.ABC):
    def __init__(self, x):  # noqa: B027
        pass

    @abc.abstractmethod
    def qux(self):
        pass


class ConcreteFoo1(AbstractFoo):
    # Can't resolve this one due to unannotated `x` param
    def qux(self):
        pass


class ConcreteFoo2(AbstractFoo):
    def __init__(self, x: int):
        pass

    def qux(self):
        pass


@hegel.test
def test_gen_abstract(tc):
    # This requires that we correctly checked which of the subclasses
    # could be resolved, rather than unconditionally using all of them.
    foo = tc.draw(gs.from_type(AbstractFoo))
    assert isinstance(foo, ConcreteFoo2)


class AbstractBar(abc.ABC):
    def __init__(self, x):  # noqa: B027
        pass

    @abc.abstractmethod
    def qux(self):
        pass


class ConcreteBar(AbstractBar):
    def qux(self):
        pass


def test_abstract_resolver_fallback():
    # from_type resolves eagerly, so trying to resolve the abstract type fails,
    # UNLESS the concrete type is currently resolvable
    with pytest.raises(ResolutionFailed):
        gs.from_type(AbstractBar)
    with temp_registered(ConcreteBar, gs.builds(ConcreteBar, x=gs.none())):
        # which in turn means we resolve to the concrete subtype.
        assert_simple_property(gs.from_type(AbstractBar), lambda gen: isinstance(gen, ConcreteBar))
    with pytest.raises(ResolutionFailed):
        gs.from_type(AbstractBar)
