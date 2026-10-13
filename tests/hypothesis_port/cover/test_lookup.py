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
# test_resolving_mutually_recursive_abstract_subclasses_is_efficient,
# test_can_get_type_hints (hypothesis.internal helpers);
# test_cannot_pass_infer_as_posarg, test_cannot_force_inference_for_unannotated_arg,
# test_can_use_type_hints, test_error_if_has_unresolvable_hints (@given
# argument inference and the `...` infer marker);
# test_can_resolve_forward_reference_to_class, test_can_resolve_type_forward_reference,
# test_forward_ref_resolved_from_local_scope, test_ambiguous_forward_ref_is_not_resolved,
# test_forward_ref_found_in_multiple_frames_is_resolved,
# test_forward_ref_to_non_type_in_caller_frame_fails,
# test_from_type_builtin_generic_with_string_forward_ref (forward references
# resolved from caller frames);
# test_builds_suggests_from_type, test_builds_suggests_from_type_on_construction,
# test_builds_mentions_no_type_check (builds() error hints);
# test_rare_types, test_ItemsView, test_regex_types, test_Generator,
# test_resolves_bare_callable_to_function, test_resolves_callable_with_arg_to_function,
# test_resolves_ellipses_callable_to_function, test_supportsop_types_support_protocol,
# test_supportscast_types_support_protocol_or_are_castable, test_timezone_lookup,
# test_hashable_type_unhashable_value, test_from_type_resolves_required_posonly_args,
# test_does_not_resolve_special_cases, the Supports* cases of test_resolves_weird_types,
# the DefaultDict, KeysView, ValuesView, Iterator, Container and Counter cases of
# test_specialised_collection_types, the ChainMap and DefaultDict cases of
# test_specialised_mapping_types, the Iterator and Collection cases of
# test_resolve_typing_module, and the builtins outside RESOLVABLE_BUILTINS in
# test_resolves_builtin_types and test_resolves_forwardrefs_to_builtin_types
# (from_type does not resolve these types);
# test_typing_Type_Union, test_type_with_unresolvable_forward_reference_fails,
# test_resolves_type_of_builtin_types, test_resolves_type_of_union_of_forwardrefs_to_builtins,
# the type["int"] cases of test_typing_Type_int (forward references inside type[...]);
# test_namedtuple_self_referential_forward_ref, test_resolving_recursive_type,
# test_resolving_recursive_typeddict, test_resolving_mutually_recursive_types,
# test_resolving_mutually_recursive_types_with_limited_stack,
# test_resolving_recursive_type_with_registered_constraint,
# test_resolving_recursive_type_with_registered_constraint_not_none,
# test_resolves_forward_references_outside_annotations (recursive types);
# test_same_typevars_same_type, test_typevars_can_be_redefined,
# test_typevars_can_be_redefine_with_factory, test_typevars_can_be_resolved_conditionally,
# the unbound, Foo-bound, union-bound and constrained cases of
# test_typevar_type_is_consistent (consistent TypeVar draws and TypeVar registry overrides);
# test_generic_aliases_can_be_conditionally_resolved_by_registered_function,
# test_custom_strategy_function_resolves_types_conditionally, the unhandled case of
# test_resolves_NewType_conditionally (factories declining with NotImplemented);
# test_register_generic_typing_strats (registering generic aliases).

import abc
import builtins
import collections
import enum
import inspect
import string
import typing
from inspect import signature
from numbers import Real
from typing import (
    Dict as _Dict,
)
from typing import (
    FrozenSet as _FrozenSet,
)
from typing import (
    List as _List,
)
from typing import (
    Set as _Set,
)
from typing import (
    Tuple as _Tuple,
)
from typing import (
    Type as _Type,
)

import pytest

import hegel
from hegel import generators as gs
from hegel.generators import from_type
from tests.hypothesis_port.common import types
from tests.hypothesis_port.common.debug import (
    assert_no_examples,
    assert_simple_property,
    check_can_generate_examples,
    find_any,
    minimal,
)
from tests.hypothesis_port.common.utils import temp_registered

# we'll continue testing the typing variants until their removal from the stdlib
# ruff: noqa: UP006, UP035, UP045, UP007

RUNTIME_SUBCLASSES = (
    "Hegel resolves an abstract collections.abc type by its runtime subclasses, which "
    "under pytest include an uninstantiable Protocol"
)


def xfail(reason):
    return pytest.mark.xfail(strict=True, reason=reason)


sentinel = object()
# The builtins from_type can resolve: those with their own generator, plus the
# few whose constructor takes no required arguments.
RESOLVABLE_BUILTINS = {
    bool,
    bytearray,
    bytes,
    complex,
    float,
    frozenset,
    int,
    list,
    object,
    property,
    set,
    str,
    tuple,
    type,
    zip,
}
BUILTIN_TYPES = tuple(
    v
    for v in vars(builtins).values()
    if isinstance(v, type) and v.__name__ != "BuiltinImporter" and v in RESOLVABLE_BUILTINS
)
# The typing module's aliases for the generic origins from_type handles.
typing_aliases = [
    t
    for t in vars(typing).values()
    if typing.get_origin(t) in types._global_type_lookup
    and typing.get_origin(t) not in (collections.abc.Iterator, collections.abc.Collection)
]


@pytest.mark.parametrize("typ", typing_aliases, ids=repr)
@hegel.test(suppress_health_check={hegel.HealthCheck.too_slow, hegel.HealthCheck.filter_too_much})
def test_resolve_typing_module(tc, typ):
    ex = tc.draw(from_type(typ))
    assert isinstance(ex, typ)


@pytest.mark.parametrize(
    "typ,instance_of",
    [
        (typing.Union[int, str], (int, str)),
        (int | str, (int, str)),
        (typing.Optional[int], (int, type(None))),
        (int | None, (int, type(None))),
    ],
)
@hegel.test
def test_specialised_scalar_types(tc, typ, instance_of):
    ex = tc.draw(from_type(typ))
    assert isinstance(ex, instance_of)


@pytest.mark.parametrize("t", [type[int], _Type[int]])
def test_typing_Type_int(t):
    assert_simple_property(from_type(t), lambda x: x is int)


class Elem:
    pass


@pytest.mark.parametrize(
    "typ,coll_type",
    [
        (_Set[Elem], set),
        (_FrozenSet[Elem], frozenset),
        (_Dict[Elem, None], dict),
        (set[Elem], set),
        (frozenset[Elem], frozenset),
        (dict[Elem, None], dict),
        (_List[Elem], list),
        (_Tuple[Elem], tuple),
        (_Tuple[Elem, ...], tuple),
        (list[Elem], list),
        (tuple[Elem], tuple),
        (tuple[Elem, ...], tuple),
        (typing.Sequence[Elem], typing.Sequence),
        (typing.Iterable[Elem], typing.Iterable),
        (typing.Mapping[Elem, None], typing.Mapping),
        (typing.NamedTuple("A_NamedTuple", (("elem", Elem),)), tuple),
        (typing.Deque[Elem], typing.Deque),
    ],
    ids=repr,
)
@hegel.test
def test_specialised_collection_types(tc, typ, coll_type):
    ex = tc.draw(from_type(typ))
    assert isinstance(ex, coll_type)
    instances = [isinstance(elem, Elem) for elem in ex]
    assert all(instances)
    tc.assume(instances)  # non-empty collections without calling len(iterator)


class ElemValue:
    pass


@pytest.mark.parametrize(
    "typ,coll_type",
    [(typing.OrderedDict[Elem, ElemValue], typing.OrderedDict)],
    ids=repr,
)
@hegel.test
def test_specialised_mapping_types(tc, typ, coll_type):
    ex = tc.draw(from_type(typ).filter(len))
    assert isinstance(ex, coll_type)
    instances = [isinstance(elem, Elem) for elem in ex]
    assert all(instances)
    assert all(isinstance(elem, ElemValue) for elem in ex.values())


def test_Optional_minimises_to_None():
    assert minimal(from_type(typing.Optional[int]), lambda ex: True) is None


@pytest.mark.parametrize("n", [0, 1, 5])
@pytest.mark.parametrize("t", [tuple, _Tuple])
def test_variable_length_tuples(t, n):
    type_ = t[int, ...]
    check_can_generate_examples(from_type(type_).filter(lambda ex: len(ex) == n))


def test_lookup_overrides_defaults():
    sentinel = object()
    with temp_registered(int, gs.just(sentinel)):

        @hegel.test
        def inner_1(tc):
            ex = tc.draw(from_type(list[int]))
            assert all(elem is sentinel for elem in ex)

        inner_1()

    @hegel.test
    def inner_2(tc):
        ex = tc.draw(from_type(list[int]))
        assert all(isinstance(elem, int) for elem in ex)

    inner_2()


@pytest.mark.parametrize(
    "typ",
    [
        typing.Sequence,
        pytest.param(
            typing.Container, marks=pytest.mark.xfail(strict=False, reason=RUNTIME_SUBCLASSES)
        ),
        typing.Mapping,
        typing.Reversible,
    ],
    ids=repr,
)
def test_resolves_weird_types(typ):
    check_can_generate_examples(from_type(typ))


class Foo:
    def __init__(self, x):
        pass


class Bar(Foo):
    pass


class Baz(Foo):
    pass


gs.register_type_generator(Bar, gs.builds(Bar, gs.integers()))
gs.register_type_generator(Baz, gs.builds(Baz, gs.integers()))


@pytest.mark.parametrize("var,expected", [(typing.TypeVar("V", bound=int), int)])
@hegel.test(suppress_health_check={hegel.HealthCheck.too_slow})
def test_typevar_type_is_consistent(tc, var, expected):
    strat = from_type(var)
    v1 = tc.draw(strat)
    v2 = tc.draw(strat)
    tc.assume(v1 != v2)  # Values may vary, just not types
    assert type(v1) is type(v2)
    assert isinstance(v1, expected)


def test_distinct_typevars_same_constraint():
    A = typing.TypeVar("A", int, str)
    B = typing.TypeVar("B", int, str)
    find_any(
        gs.tuples(from_type(A), from_type(B)),
        lambda ab: type(ab[0]) is not type(ab[1]),
    )


def test_distinct_typevars_distinct_type():
    """Ensures that two different type vars have at least one different type in their strategies."""
    A = typing.TypeVar("A")
    B = typing.TypeVar("B")
    find_any(
        gs.tuples(from_type(A), from_type(B)),
        lambda ab: type(ab[0]) is not type(ab[1]),
    )


def annotated_func(a: int, b: int = 2, *, c: int, d: int = 4):
    return a + b + c + d


def test_issue_946_regression():
    # Turned type hints into kwargs even if the required posarg was passed
    check_can_generate_examples(gs.builds(annotated_func, gs.integers()))


def test_force_builds_to_infer_strategies_for_default_args():
    # By default, leaves args with defaults and minimises to 2+4=6.  (Forcing
    # inference for them with `b=...` is a Hypothesis-only feature.)
    assert minimal(gs.builds(annotated_func), lambda ex: True) == 6


class UnknownType:
    def __init__(self, arg):
        pass


class UnknownAnnotatedType:
    def __init__(self, arg: int):
        pass


@hegel.test
def test_builds_for_unknown_annotated_type(tc):
    ex = tc.draw(from_type(UnknownAnnotatedType))
    assert isinstance(ex, UnknownAnnotatedType)


def unknown_annotated_func(a: UnknownType, b=2, *, c: UnknownType, d=4):
    pass


def test_raises_for_arg_with_unresolvable_annotation():
    with pytest.raises(hegel.ResolutionFailed):
        check_can_generate_examples(gs.builds(unknown_annotated_func))
    with pytest.raises(hegel.ResolutionFailed):
        check_can_generate_examples(gs.builds(unknown_annotated_func, a=gs.none()))


def test_resolves_NewType():
    typ = typing.NewType("T", int)
    nested = typing.NewType("NestedT", typ)
    uni = typing.NewType("UnionT", int | None)
    assert_simple_property(from_type(typ), lambda x: isinstance(x, int))
    assert_simple_property(from_type(nested), lambda x: isinstance(x, int))
    assert_simple_property(from_type(uni), lambda x: isinstance(x, (int, type(None))))
    find_any(from_type(uni), lambda x: isinstance(x, int))
    find_any(from_type(uni), lambda x: isinstance(x, type(None)))


def test_resolves_NewType_conditionally():
    sentinel = object()
    typ = typing.NewType("T", int)

    def resolve_custom_strategy(thing):
        assert thing is typ
        return gs.just(sentinel)

    with temp_registered(typ, resolve_custom_strategy):
        assert_simple_property(from_type(typ), lambda x: x is sentinel)


E = enum.Enum("E", "a b c")


@hegel.test
def test_resolves_enum(tc):
    ex = tc.draw(from_type(E))
    assert isinstance(ex, E)


@pytest.mark.parametrize("resolver", [from_type, gs.sampled_from])
def test_resolves_flag_enum(resolver):
    # Storing all combinations takes O(2^n) memory.  Using an enum of 52
    # members in this test ensures that we won't try!
    F = enum.Flag("F", " ".join(string.ascii_letters))

    # Checks for combination coverage are found in nocover/test_sampled_from
    @hegel.test
    def inner(tc):
        ex = tc.draw(resolver(F))
        assert isinstance(ex, F)

    inner()


class AnnotatedTarget:
    def __init__(self, a: int, b: int):
        pass

    def method(self, a: int, b: int):
        pass


@pytest.mark.parametrize("target", [AnnotatedTarget, AnnotatedTarget(1, 2).method])
@pytest.mark.parametrize(
    "args,kwargs",
    [
        ((), {}),
        ((1,), {}),
        ((1, 2), {}),
        ((), {"a": 1}),
        ((), {"b": 2}),
        ((), {"a": 1, "b": 2}),
    ],
)
def test_required_args(target, args, kwargs):
    # Mostly checking that `self` (and only self) is correctly excluded
    check_can_generate_examples(
        gs.builds(target, *map(gs.just, args), **{k: gs.just(v) for k, v in kwargs.items()})
    )


class AnnotatedNamedTuple(typing.NamedTuple):
    a: str


@hegel.test
def test_infers_args_for_namedtuple_builds(tc):
    thing = tc.draw(gs.builds(AnnotatedNamedTuple))
    assert isinstance(thing.a, str)


@hegel.test
def test_infers_args_for_namedtuple_from_type(tc):
    thing = tc.draw(from_type(AnnotatedNamedTuple))
    assert isinstance(thing.a, str)


@hegel.test
def test_override_args_for_namedtuple(tc):
    thing = tc.draw(gs.builds(AnnotatedNamedTuple, a=gs.none()))
    assert thing.a is None


def test_unresolved_forward_ref_fails_without_ambiguity_warning():
    # Hegel never searches caller frames, so there is no ambiguity to warn about.
    with pytest.raises(hegel.ResolutionFailed):
        check_can_generate_examples(from_type(typing.ForwardRef("DefinitelyNotDefinedAnywhere")))


class MyList:
    def __init__(self, nxt: typing.Optional["MyList"] = None):
        self.nxt = nxt

    def __repr__(self):
        return f"MyList({self.nxt})"

    def __eq__(self, other):
        return type(self) is type(other) and self.nxt == other.nxt


@hegel.test
def test_resolving_recursive_type_with_defaults(tc):
    lst = tc.draw(from_type(MyList))
    assert isinstance(lst, MyList)


def test_recursive_type_with_defaults_minimizes_to_defaults():
    assert minimal(from_type(MyList), lambda ex: True) == MyList()


class A_with_default:
    def __init__(self, nxt: typing.Optional["B_with_default"] = None):
        self.nxt = nxt

    def __repr__(self):
        return f"A_with_default({self.nxt})"


class B_with_default:
    def __init__(self, nxt: typing.Optional["A_with_default"] = None):
        self.nxt = nxt

    def __repr__(self):
        return f"B_with_default({self.nxt})"


@hegel.test
def test_resolving_mutually_recursive_types_with_defaults(tc):
    # This test is required to cover the raise/except part of the recursion
    # check in from_type, see
    # https://github.com/HypothesisWorks/hypothesis/issues/3655. If the
    # skip-nondefaulted-args check is removed, this test becomes redundant.
    nxt = tc.draw(from_type(A_with_default))
    i = 0
    while nxt:
        assert isinstance(nxt, [A_with_default, B_with_default][i % 2])
        nxt = nxt.nxt
        i += 1


@xfail("Hegel's from_type treats tuple[()] like a bare tuple, so it yields non-empty tuples")
@hegel.test
def test_resolves_empty_Tuple_issue_1583_regression(tc):
    # See e.g. https://github.com/python/mypy/commit/71332d58
    ex = tc.draw(from_type(tuple[()]) | from_type(_Tuple[()]))
    assert ex == ()


def test_can_register_NewType():
    Name = typing.NewType("Name", str)
    gs.register_type_generator(Name, gs.just("Eric Idle"))
    assert_simple_property(from_type(Name), lambda x: x == "Eric Idle")


class AbstractFoo(abc.ABC):
    @abc.abstractmethod
    def foo(self):
        pass


class ConcreteFoo(AbstractFoo):
    def foo(self):
        pass


@hegel.test
def test_can_resolve_abstract_class(tc):
    instance = tc.draw(from_type(AbstractFoo))
    assert isinstance(instance, ConcreteFoo)
    instance.foo()


class AbstractBar(abc.ABC):
    @abc.abstractmethod
    def bar(self):
        pass


def test_cannot_resolve_abstract_class_with_no_concrete_subclass():
    with pytest.raises(hegel.ResolutionFailed):
        check_can_generate_examples(from_type(AbstractBar))


@pytest.mark.parametrize(
    "typ",
    [
        typing.Hashable,  # resolved through the registered Bar and Baz
        pytest.param(
            typing.Sized, marks=pytest.mark.xfail(strict=False, reason=RUNTIME_SUBCLASSES)
        ),
    ],
)
@hegel.test
def test_inference_on_generic_collections_abc_aliases(tc, typ):
    # regression test for inference bug on types that are just aliases
    # types for simple interfaces in collections abc and take no args
    # the typing module such as Hashable and Sized
    # see https://github.com/HypothesisWorks/hypothesis/issues/2272
    value = tc.draw(from_type(typ))
    assert isinstance(value, typ)


@hegel.test
def test_bytestring_not_treated_as_generic_sequence(tc):
    # Check that we don't fall into the specific problem from
    # https://github.com/HypothesisWorks/hypothesis/issues/2257
    val = tc.draw(from_type(typing.Sequence[set]))
    assert not isinstance(val, bytes)
    # Check it hasn't happened again from some other non-generic sequence type.
    for x in val:
        assert isinstance(x, set)


@pytest.mark.parametrize(
    "type_", [int, Real, object, typing.Union[int, str], typing.Union[Real, str]]
)
@xfail("Hegel's from_type(Sequence[T]) only ever generates lists")
def test_bytestring_is_valid_sequence_of_int_and_parent_classes(type_):
    find_any(
        from_type(typing.Sequence[type_]),
        lambda val: isinstance(val, bytes),
    )


def test_can_cast():
    assert types.can_cast(int, "0")
    assert not types.can_cast(int, "abc")


@pytest.mark.parametrize(
    "typ",
    [
        _Set[typing.Hashable],
        _FrozenSet[typing.Hashable],
        _Dict[typing.Hashable, int],
        set[typing.Hashable],
        frozenset[typing.Hashable],
        dict[typing.Hashable, int],
    ],
)
@hegel.test
def test_generic_collections_only_use_hashable_elements(tc, typ):
    tc.draw(from_type(typ))


@hegel.test
def test_no_byteswarning(tc):
    tc.draw(gs.sets(gs.integers() | gs.binary(), min_size=2))


@xfail("Hegel's from_type looks the type up in a dict, so an unhashable type is a TypeError")
def test_unhashable_type():
    class UnhashableMeta(type):
        __hash__ = None

    class UnhashableType(metaclass=UnhashableMeta):
        pass

    assert_simple_property(from_type(UnhashableType), lambda x: isinstance(x, UnhashableType))


@pytest.mark.parametrize(
    "typ,resolved",
    [
        (int, gs.integers()),
        (list[str], gs.lists(gs.text())),
        (_List[str], gs.lists(gs.text())),
    ],
)
def test_repr_passthrough(typ, resolved):
    assert repr(from_type(typ)) == repr(resolved)


def constructor(a: str = None):  # noqa  # deprecated implicit optional, for testing
    pass


class WithOptionalInSignature:
    __signature__ = inspect.signature(constructor)
    __annotations__ = typing.get_type_hints(constructor)

    def __init__(self, **kwargs):
        assert set(kwargs) == {"a"}
        self.a = kwargs["a"]


def test_compat_get_type_hints_aware_of_None_default():
    # Regression test for https://github.com/HypothesisWorks/hypothesis/issues/2648
    # (from_type, unlike builds, generates annotated arguments with defaults)
    strategy = from_type(WithOptionalInSignature)
    find_any(strategy, lambda x: x.a is None)
    find_any(strategy, lambda x: x.a is not None)

    # https://docs.python.org/3.11/library/typing.html#typing.get_type_hints
    assert typing.get_type_hints(constructor)["a"] is str
    assert inspect.signature(constructor).parameters["a"].annotation is str


_ValueType = typing.TypeVar("_ValueType")


class Wrapper(typing.Generic[_ValueType]):
    _inner_value: _ValueType

    def __init__(self, inner_value: _ValueType) -> None:
        self._inner_value = inner_value


@hegel.test
def test_issue_2603_regression(tc):
    """It was impossible to build annotated classes with constructors."""
    built = tc.draw(gs.builds(Wrapper))
    assert isinstance(built, Wrapper)


class AnnotatedConstructor(typing.Generic[_ValueType]):
    value: _ValueType  # the same name we have in `__init__`

    def __init__(self, value: int) -> None:
        """By this example we show, that ``int`` is more important than ``_ValueType``."""
        assert isinstance(value, int)


@hegel.test
def test_constructor_is_more_important(tc):
    """Constructor types should take precedence over all other annotations."""
    tc.draw(gs.builds(AnnotatedConstructor))


def use_signature(self, value: str) -> None: ...


class AnnotatedConstructorWithSignature(typing.Generic[_ValueType]):
    value: _ValueType  # the same name we have in `__init__`

    __signature__ = signature(use_signature)

    def __init__(self, value: int) -> None:
        """By this example we show, that ``__signature__`` is the most important source."""
        assert isinstance(value, str)


def selfless_signature(value: str) -> None: ...


class AnnotatedConstructorWithSelflessSignature(AnnotatedConstructorWithSignature):
    __signature__ = signature(selfless_signature)


def really_takes_str(value: int) -> None:
    """By this example we show, that ``__signature__`` is the most important source."""
    assert isinstance(value, str)


really_takes_str.__signature__ = signature(selfless_signature)


@pytest.mark.parametrize(
    "thing",
    [
        AnnotatedConstructorWithSignature,
        AnnotatedConstructorWithSelflessSignature,
        really_takes_str,
    ],
)
@xfail("Hegel's builds prefers __annotations__ to the annotations of an explicit __signature__")
def test_signature_is_the_most_important_source(thing):
    """Signature types should take precedence over all other annotations."""
    check_can_generate_examples(gs.builds(thing))


class AnnotatedAndDefault:
    def __init__(self, foo: bool | None = None):
        self.foo = foo


def test_from_type_can_be_default_or_annotation():
    find_any(from_type(AnnotatedAndDefault), lambda x: x.foo is None)
    find_any(from_type(AnnotatedAndDefault), lambda x: isinstance(x.foo, bool))


@pytest.mark.parametrize("t", BUILTIN_TYPES, ids=lambda t: t.__name__)
def test_resolves_builtin_types(t):
    assert_simple_property(from_type(t), lambda v: isinstance(v, t))


@pytest.mark.parametrize("t", BUILTIN_TYPES, ids=lambda t: t.__name__)
@hegel.test(test_cases=20)
def test_resolves_forwardrefs_to_builtin_types(tc, t):
    s = from_type(typing.ForwardRef(t.__name__))
    v = tc.draw(s)
    assert isinstance(v, t)


class TupleSubtype(tuple):
    pass


def test_tuple_subclasses_not_generic_sequences():
    # see https://github.com/HypothesisWorks/hypothesis/issues/3767.
    with temp_registered(TupleSubtype, gs.builds(TupleSubtype)):
        s = from_type(typing.Sequence[int])
        assert_no_examples(s, lambda x: isinstance(x, tuple))


class MyProtocol(typing.Protocol):
    pass


def test_issue_4194_regression():
    # this was an edge case where we were calling issubclass on something
    # that was not a type, which errored. I don't have a more principled test
    # case or name for this.
    inner = typing.Union[typing.Sequence["A"], MyProtocol]
    A = typing.Union[typing.Sequence[inner], MyProtocol]

    with (
        temp_registered(MyProtocol, gs.just(b"")),
        temp_registered(typing.ForwardRef("A"), gs.integers()),
    ):
        find_any(from_type(A))
