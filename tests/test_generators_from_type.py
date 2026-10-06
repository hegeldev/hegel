from __future__ import annotations

import abc
import collections
import collections.abc
import dataclasses
import datetime as dt
import decimal
import enum
import fractions
import ipaddress
import typing
import uuid

import pytest

import hegel
from hegel import generators as gs
from hegel.generators.from_type import infer_missing_arguments
from tests.helpers import find_example, run_test


def check_all(generator, predicate, *, test_cases=30):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


class Colour(enum.Enum):
    red = 1


@dataclasses.dataclass
class Point:
    x: int
    y: float


class Movie(typing.TypedDict):
    title: str
    year: int


class PartialMovie(typing.TypedDict, total=False):
    title: str


class Annotated:
    def __init__(self, name: str, count: int = 0):
        self.name = name
        self.count = count


class Unannotated:
    def __init__(self, name):
        self.name = name


class Custom:
    pass


class Generic(typing.Generic[typing.TypeVar("T")]):
    pass


T = typing.TypeVar("T")
Bound = typing.TypeVar("Bound", bound=int)
Constrained = typing.TypeVar("Constrained", int, str)
UserId = typing.NewType("UserId", int)


CASES = [
    (type(None), lambda v: v is None),
    (None, lambda v: v is None),
    (bool, lambda v: isinstance(v, bool)),
    (int, lambda v: isinstance(v, int)),
    (float, lambda v: isinstance(v, float)),
    (complex, lambda v: isinstance(v, complex)),
    (str, lambda v: isinstance(v, str)),
    (bytes, lambda v: isinstance(v, bytes)),
    (bytearray, lambda v: isinstance(v, bytearray)),
    (decimal.Decimal, lambda v: isinstance(v, decimal.Decimal)),
    (fractions.Fraction, lambda v: isinstance(v, fractions.Fraction)),
    (dt.date, lambda v: type(v) is dt.date),
    (dt.time, lambda v: isinstance(v, dt.time)),
    (dt.datetime, lambda v: isinstance(v, dt.datetime)),
    (dt.timedelta, lambda v: isinstance(v, dt.timedelta)),
    (uuid.UUID, lambda v: isinstance(v, uuid.UUID)),
    (ipaddress.IPv4Address, lambda v: isinstance(v, ipaddress.IPv4Address)),
    (ipaddress.IPv6Address, lambda v: isinstance(v, ipaddress.IPv6Address)),
    (object, lambda v: True),
    (typing.Any, lambda v: True),
    (list, lambda v: isinstance(v, list)),
    (list[int], lambda v: isinstance(v, list) and all(isinstance(x, int) for x in v)),
    (list[str], lambda v: all(isinstance(x, str) for x in v)),
    (collections.abc.Sequence[int], lambda v: isinstance(v, list)),
    (collections.abc.MutableSequence[int], lambda v: isinstance(v, list)),
    (collections.deque[int], lambda v: isinstance(v, collections.deque)),
    (set[int], lambda v: isinstance(v, set)),
    (collections.abc.MutableSet[int], lambda v: isinstance(v, set)),
    (frozenset[int], lambda v: isinstance(v, frozenset)),
    (collections.abc.Set[int], lambda v: isinstance(v, frozenset)),
    (
        dict[str, int],
        lambda v: all(isinstance(k, str) and isinstance(x, int) for k, x in v.items()),
    ),
    (dict[int, int], lambda v: isinstance(v, dict)),
    (collections.abc.Mapping[int, int], lambda v: isinstance(v, dict)),
    (collections.abc.MutableMapping[int, int], lambda v: isinstance(v, dict)),
    (collections.OrderedDict[int, int], lambda v: isinstance(v, collections.OrderedDict)),
    (collections.abc.Iterable[int], lambda v: all(isinstance(x, int) for x in v)),
    (collections.abc.Iterator[int], lambda v: iter(v) is not None),
    (collections.abc.Collection[int], lambda v: iter(v) is not None),
    (tuple, lambda v: isinstance(v, tuple)),
    (typing.Tuple, lambda v: isinstance(v, tuple)),  # noqa: UP006
    (tuple[int, ...], lambda v: isinstance(v, tuple) and all(isinstance(x, int) for x in v)),
    (tuple[int, str], lambda v: len(v) == 2 and isinstance(v[1], str)),
    (tuple[int, str], lambda v: len(v) == 2),
    (typing.Union[int, str], lambda v: isinstance(v, (int, str))),
    (int | None, lambda v: v is None or isinstance(v, int)),
    (typing.Optional[str], lambda v: v is None or isinstance(v, str)),
    (typing.Literal["a", 2], lambda v: v in ("a", 2)),
    (type, lambda v: v is object),
    (type[int], lambda v: v is int),
    (typing.Annotated[int, "meta"], lambda v: isinstance(v, int)),
    (Colour, lambda v: v is Colour.red),
    (Point, lambda v: isinstance(v, Point) and isinstance(v.y, float)),
    (Movie, lambda v: set(v) == {"title", "year"}),
    (PartialMovie, lambda v: set(v) <= {"title"}),
    (Annotated, lambda v: isinstance(v, Annotated) and isinstance(v.count, int)),
    (T, lambda v: True),
    (Bound, lambda v: isinstance(v, int)),
    (Constrained, lambda v: isinstance(v, (int, str))),
    (UserId, lambda v: isinstance(v, int)),
    (Generic[int], lambda v: isinstance(v, Generic)),
]


@pytest.mark.parametrize("type_, check", CASES, ids=[repr(t) for t, _ in CASES])
def test_from_type(type_, check):
    check_all(gs.from_type(type_), check)


def test_from_type_errors():
    with pytest.raises(hegel.InvalidArgument):
        gs.from_type("int")
    with pytest.raises(hegel.InvalidArgument):
        gs.from_type(3)
    with pytest.raises(hegel.InvalidArgument):
        gs.from_type(Unannotated)

    class NoSignature:
        __signature__ = property(lambda self: 1 / 0)

    with pytest.raises(hegel.InvalidArgument, match="no usable signature"):
        gs.from_type(NoSignature)


def test_register_type_generator():
    gs.register_type_generator(Custom, gs.just("custom"))
    check_all(gs.from_type(Custom), lambda v: v == "custom")
    gs.register_type_generator(Custom, lambda t: gs.just(t))
    check_all(gs.from_type(Custom), lambda v: v is Custom)
    gs.register_type_generator(Generic, lambda t: gs.just(typing.get_args(t)))
    check_all(gs.from_type(Generic[str]), lambda v: v == (str,))
    with pytest.raises(hegel.InvalidArgument):
        gs.register_type_generator(Custom, 5)


def test_builds_infers_annotated_arguments():
    check_all(gs.builds(Annotated), lambda v: isinstance(v.name, str))
    check_all(gs.builds(Point, gs.just(1)), lambda v: v.x == 1 and isinstance(v.y, float))

    def positional_only(a: int, /, b: str, *args, c: bytes, **kwargs):
        return (a, b, c)

    inferred = infer_missing_arguments(positional_only, 1, {})
    assert set(inferred) == {"b", "c"}
    assert infer_missing_arguments(len, 0, {}) == {}

    def no_annotation(a, b: ...):  # type: ignore[valid-type]
        return a

    assert infer_missing_arguments(no_annotation, 0, {}) == {}

    class BadHints:
        def __init__(self, a: NoSuchType):  # noqa: F821
            pass

    with pytest.raises(hegel.InvalidArgument, match="could not be resolved"):
        infer_missing_arguments(BadHints, 0, {})


def test_from_type_networks_and_interfaces():
    check_all(gs.from_type(ipaddress.IPv4Network), lambda v: isinstance(v, ipaddress.IPv4Network))
    check_all(gs.from_type(ipaddress.IPv6Network), lambda v: isinstance(v, ipaddress.IPv6Network))
    check_all(
        gs.from_type(ipaddress.IPv4Interface), lambda v: isinstance(v, ipaddress.IPv4Interface)
    )
    check_all(
        gs.from_type(ipaddress.IPv6Interface), lambda v: isinstance(v, ipaddress.IPv6Interface)
    )


class Node:
    def __init__(self, child: Node | None = None):
        self.child = child


def test_from_type_resolves_self_referential_optional_arguments():
    check_all(gs.from_type(Node), lambda v: isinstance(v, Node))
    assert find_example(gs.from_type(Node), lambda n: True).child is None
    assert find_example(gs.from_type(Node), lambda n: n.child is not None).child.child is None


BoundToNode = typing.TypeVar("BoundToNode", bound="Node")


def test_from_type_resolves_forward_references():
    check_all(gs.from_type(typing.ForwardRef("int")), lambda v: isinstance(v, int))
    check_all(gs.from_type(BoundToNode), lambda v: isinstance(v, Node))
    with pytest.raises(hegel.errors.ResolutionFailed, match="Could not resolve"):
        gs.from_type(typing.ForwardRef("NoSuchThing"))
    with pytest.raises(hegel.errors.ResolutionFailed, match="string annotations"):
        gs.from_type("int")
    with pytest.raises(hegel.errors.ResolutionFailed, match="not a type"):
        gs.from_type(42)
    gs.register_type_generator(typing.ForwardRef("Registered"), gs.just("registered"))
    try:
        check_all(gs.from_type(typing.ForwardRef("Registered")), lambda v: v == "registered")
    finally:
        del gs.from_type.__globals__["_registry"][typing.ForwardRef("Registered")]


@pytest.mark.skipif(not hasattr(typing, "TypeAliasType"), reason="PEP 695 needs Python 3.12")
def test_from_type_resolves_type_aliases():
    alias = typing.TypeAliasType("alias", int)  # type: ignore[attr-defined]
    check_all(gs.from_type(alias), lambda v: isinstance(v, int))
    T = typing.TypeVar("T")
    generic = typing.TypeAliasType("generic", list[T], type_params=(T,))  # type: ignore[attr-defined]
    check_all(gs.from_type(generic[int]), lambda v: all(isinstance(x, int) for x in v))
    gs.register_type_generator(generic, lambda t: gs.just(typing.get_args(t)))
    try:
        check_all(gs.from_type(generic[str]), lambda v: v == (str,))
    finally:
        del gs.from_type.__globals__["_registry"][generic]
    unparametrised = typing.TypeAliasType("plain", list[int], type_params=(T,))  # type: ignore[attr-defined]
    check_all(gs.from_type(unparametrised[str]), lambda v: isinstance(v, list))


class Shape:
    def __init__(self, sides: int):
        self.sides = sides


class Square(Shape):
    pass


class Abstract(abc.ABC):
    @abc.abstractmethod
    def method(self): ...


class Concrete(Abstract):
    def __init__(self, x: int):
        self.x = x

    def method(self):
        return self.x


class Unbuildable(Abstract):
    def __init__(self, x):
        self.x = x

    def method(self):
        return self.x


class Lonely(abc.ABC):
    @abc.abstractmethod
    def method(self): ...


def test_from_type_prefers_registered_subclasses_and_resolves_abstract_classes():
    gs.register_type_generator(Square, gs.just(Square(4)))
    try:
        check_all(gs.from_type(Shape), lambda v: isinstance(v, Square))
    finally:
        del gs.from_type.__globals__["_registry"][Square]
    check_all(gs.from_type(Abstract), lambda v: isinstance(v, Concrete))
    with pytest.raises(hegel.errors.ResolutionFailed, match="no subclasses"):
        gs.from_type(Lonely)

    class OnlyUnbuildable(abc.ABC):
        @abc.abstractmethod
        def method(self): ...

    class Sub(OnlyUnbuildable):
        def __init__(self, x):
            self.x = x

        def method(self):
            return self.x

    with pytest.raises(hegel.errors.ResolutionFailed, match="no subclass can be resolved"):
        gs.from_type(OnlyUnbuildable)


def test_registered_generators_are_checked():
    gs.register_type_generator(Shape, lambda t: "not a generator")
    try:
        with pytest.raises(hegel.errors.ResolutionFailed, match="not a generator"):
            gs.from_type(Shape)
    finally:
        del gs.from_type.__globals__["_registry"][Shape]
    gs.register_type_generator(Shape, gs.nothing())
    try:
        with pytest.raises(hegel.errors.ResolutionFailed, match="is empty"):
            gs.from_type(Shape)
    finally:
        del gs.from_type.__globals__["_registry"][Shape]


class HasMethod(typing.Protocol):
    def method(self) -> int: ...


def test_registered_protocols_do_not_break_subclass_checks():
    # issubclass() against a non-runtime protocol raises TypeError.
    gs.register_type_generator(HasMethod, gs.just(Concrete(1)))
    gs.register_type_generator(Square, gs.just(Square(4)))
    try:
        check_all(gs.from_type(Shape), lambda v: isinstance(v, Square))
    finally:
        del gs.from_type.__globals__["_registry"][HasMethod]
        del gs.from_type.__globals__["_registry"][Square]


BoundToRegistered = typing.TypeVar("BoundToRegistered", bound="RegisteredName")  # noqa: F821


def test_type_variables_bound_to_registered_forward_references():
    gs.register_type_generator(typing.ForwardRef("RegisteredName"), gs.just("registered"))
    try:
        check_all(gs.from_type(BoundToRegistered), lambda v: v == "registered")
    finally:
        del gs.from_type.__globals__["_registry"][typing.ForwardRef("RegisteredName")]
