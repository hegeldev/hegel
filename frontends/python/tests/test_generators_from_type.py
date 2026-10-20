from __future__ import annotations

import abc
import collections
import collections.abc
import dataclasses
import datetime as dt
import decimal
import enum
import fractions
import inspect
import ipaddress
import numbers
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
    (collections.abc.Sequence[int], lambda v: isinstance(v, (list, bytes))),
    (collections.abc.Sequence[str], lambda v: isinstance(v, list)),
    (collections.abc.MutableSequence[int], lambda v: isinstance(v, list)),
    (collections.deque[int], lambda v: isinstance(v, collections.deque)),
    (collections.deque, lambda v: isinstance(v, collections.deque)),
    (set[int], lambda v: isinstance(v, set)),
    (collections.abc.MutableSet[int], lambda v: isinstance(v, set)),
    (frozenset[int], lambda v: isinstance(v, frozenset)),
    (collections.abc.Set[int], lambda v: isinstance(v, frozenset)),
    (
        dict[str, int],
        lambda v: all(isinstance(k, str) and isinstance(x, int) for k, x in v.items()),
    ),
    (dict[int, int], lambda v: isinstance(v, dict)),
    (dict, lambda v: isinstance(v, dict)),
    (collections.abc.Mapping[int, int], lambda v: isinstance(v, dict)),
    (collections.abc.MutableMapping[int, int], lambda v: isinstance(v, dict)),
    (collections.OrderedDict[int, int], lambda v: isinstance(v, collections.OrderedDict)),
    (collections.OrderedDict, lambda v: isinstance(v, collections.OrderedDict)),
    (collections.abc.Iterable[int], lambda v: all(isinstance(x, int) for x in v)),
    (collections.abc.Iterator[int], lambda v: not isinstance(v, bytes) and iter(v) is not None),
    (collections.abc.Collection[int], lambda v: iter(v) is not None),
    (tuple, lambda v: isinstance(v, tuple)),
    (tuple[()], lambda v: v == ()),
    (typing.Tuple[()], lambda v: v == ()),  # noqa: UP006
    (typing.Tuple, lambda v: isinstance(v, tuple)),  # noqa: UP006
    (tuple[int, ...], lambda v: isinstance(v, tuple) and all(isinstance(x, int) for x in v)),
    (tuple[int, str], lambda v: len(v) == 2 and isinstance(v[1], str)),
    (tuple[int, str], lambda v: len(v) == 2),
    (typing.Union[int, str], lambda v: isinstance(v, (int, str))),
    (int | None, lambda v: v is None or isinstance(v, int)),
    (typing.Optional[str], lambda v: v is None or isinstance(v, str)),
    (typing.Literal["a", 2], lambda v: v in ("a", 2)),
    (type, lambda v: v is object),
    (zip, lambda v: isinstance(v, zip)),
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
    # A class before Python 3.13, so not rejected along with the other special forms.
    with pytest.raises(hegel.InvalidArgument, match="Annotated needs a type argument"):
        gs.from_type(typing.Annotated)


def test_from_type_tolerates_annotations_that_do_not_evaluate():
    # Python 3.14 evaluates annotations lazily, so merely reading a class's
    # ``__annotations__`` can raise: the standard library's ``_colorize.ThemeSection``
    # annotates a field with a ``ClassVar`` it imports only for type checking,
    # and resolving ``Container`` or ``Sized`` through their subclasses meets it.
    class Lazy(abc.ABCMeta):
        @property
        def __annotations__(cls):
            raise NameError("name 'ClassVar' is not defined")

    class Empty(collections.abc.Sized, metaclass=Lazy):
        def __len__(self):
            return 0

    check_all(gs.from_type(Empty), lambda v: isinstance(v, Empty))


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

    supplied = (gs.just(1),)
    args, kwargs = infer_missing_arguments(positional_only, supplied, {})
    assert args == supplied
    assert set(kwargs) == {"b", "c"}
    assert infer_missing_arguments(len, (), {}) == ((), {})

    def no_annotation(a, b: ...):  # type: ignore[valid-type]
        return a

    assert infer_missing_arguments(no_annotation, (), {}) == ((), {})

    class BadHints:
        def __init__(self, a: NoSuchType):  # noqa: F821
            pass

    with pytest.raises(hegel.InvalidArgument, match="could not be resolved"):
        infer_missing_arguments(BadHints, (), {})


def test_builds_infers_positional_only_arguments():
    def positional_only(a: int, b: str, /, c: bytes = b"", *, d: float):
        return (a, b, c, d)

    check_all(
        gs.builds(positional_only),
        lambda v: isinstance(v[0], int) and isinstance(v[1], str) and v[2] == b"",
    )
    check_all(gs.builds(positional_only, gs.just(1)), lambda v: v[0] == 1 and isinstance(v[1], str))

    def unannotated_first(a, b: str, /):
        return (a, b)

    # ``b`` cannot be passed without ``a``, so neither is inferred.
    assert infer_missing_arguments(unannotated_first, (), {}) == ((), {})

    class PositionalOnly:
        def __init__(self, a: int, /, b: str = "b"):
            self.a = a
            self.b = b

    check_all(gs.from_type(PositionalOnly), lambda v: isinstance(v.a, int) and isinstance(v.b, str))


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
    assert (
        find_example(gs.from_type(Node), lambda n: n.child is not None, seed=0).child.child is None
    )


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
    with pytest.raises(hegel.InvalidArgument, match="not the string 'Shape'"):
        gs.register_type_generator("Shape", gs.just(Square(4)))
    assert Shape not in gs.from_type.__globals__["_registry"]
    assert "Shape" not in gs.from_type.__globals__["_registry"]


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


class UnhashableMeta(type):
    __hash__ = None  # type: ignore[assignment]


class Unhashable(metaclass=UnhashableMeta):
    pass


def test_from_type_resolves_unhashable_types():
    check_all(gs.from_type(Unhashable), lambda v: isinstance(v, Unhashable))


def takes_str(value: str, extra=None) -> None: ...


class SignedConstructor:
    value: int

    __signature__ = inspect.signature(takes_str)

    def __init__(self, value: int):
        self.value = value


def takes_int(value: int) -> None:
    assert isinstance(value, int)


takes_int.__signature__ = inspect.signature(takes_str)  # type: ignore[attr-defined]


def test_explicit_signature_outranks_annotations():
    check_all(gs.from_type(SignedConstructor), lambda v: isinstance(v.value, str))
    check_all(gs.builds(SignedConstructor), lambda v: isinstance(v.value, str))
    with pytest.raises(AssertionError):
        check_all(gs.builds(takes_int), lambda v: True)

    def forward(value: NoSuchName) -> None: ...  # noqa: F821

    def resolved(value: int) -> None:
        return value

    resolved.__signature__ = inspect.signature(forward)  # type: ignore[attr-defined]
    # An unresolvable annotation in the signature defers to the resolved one.
    check_all(gs.builds(resolved), lambda v: isinstance(v, int))


class Greets(typing.Protocol):
    def greet(self) -> str: ...


class Greeter(Greets):
    def __init__(self, name: str):
        self.name = name

    def greet(self) -> str:
        return self.name


class Sized(typing.Protocol):
    def size(self) -> int: ...


class Measured(abc.ABC):
    @abc.abstractmethod
    def method(self): ...


class Ruler(Measured):
    def __init__(self, sized: Sized):
        self.sized = sized

    def method(self):
        return self.sized


class Yardstick(Measured):
    def method(self):
        return 3


def test_from_type_resolves_protocols_through_their_subclasses():
    check_all(gs.from_type(Greets), lambda v: isinstance(v, Greeter))
    with pytest.raises(hegel.errors.ResolutionFailed, match="no subclasses"):
        gs.from_type(Sized)
    # A subclass needing an unresolvable protocol is skipped, not instantiated.
    check_all(gs.from_type(Measured), lambda v: isinstance(v, Yardstick))


def test_typed_dict_with_unresolvable_annotations():
    Alias = int

    class Local(typing.TypedDict):
        value: Alias

    with pytest.raises(hegel.errors.ResolutionFailed, match="annotations could not be resolved"):
        gs.from_type(Local)


@pytest.mark.parametrize(
    "type_",
    [
        collections.abc.Sequence[int],
        collections.abc.Sequence[object],
        collections.abc.Sequence[numbers.Real],
        collections.abc.Sequence[int | str],
        collections.abc.Sequence[typing.Union[str, numbers.Real]],  # noqa: UP007
        collections.abc.Iterable[int],
        collections.abc.Collection[int],
        typing.Sequence,  # noqa: UP006
    ],
    ids=repr,
)
def test_bytes_are_sequences_of_integers(type_):
    assert isinstance(find_example(gs.from_type(type_), lambda v: isinstance(v, bytes)), bytes)
    assert not isinstance(
        find_example(gs.from_type(type_), lambda v: not isinstance(v, bytes)), bytes
    )


@pytest.mark.parametrize(
    "type_",
    [
        collections.abc.Sequence[str],
        collections.abc.Sequence[bool],
        collections.abc.Sequence[str | bytes],
        collections.abc.Sequence[typing.Any],
        collections.abc.MutableSequence[int],
        collections.abc.Iterator[int],
        list[int],
    ],
    ids=repr,
)
def test_bytes_are_not_sequences_of_other_things(type_):
    check_all(gs.from_type(type_), lambda v: not isinstance(v, bytes))


def test_from_type_honours_an_explicit_signature_without_self():
    class Model:
        __signature__ = inspect.Signature(
            [
                inspect.Parameter("self", inspect.Parameter.POSITIONAL_OR_KEYWORD),
                inspect.Parameter("n", inspect.Parameter.KEYWORD_ONLY, annotation=int),
            ]
        )

        def __init__(self, **kwargs: int) -> None:
            self.n = kwargs["n"]

    value = find_example(gs.from_type(Model))
    assert isinstance(value.n, int)


def test_from_type_ignores_variadic_parameters():
    class Variadic:
        def __init__(self, x: int, *args: str, **kwargs: str) -> None:
            self.x = x

    assert isinstance(find_example(gs.from_type(Variadic)).x, int)


def test_from_type_keeps_an_explicit_signature_without_self():
    class Model:
        __signature__ = inspect.Signature(
            [inspect.Parameter("n", inspect.Parameter.KEYWORD_ONLY, annotation=int)]
        )

        def __init__(self, **kwargs: int) -> None:
            self.n = kwargs["n"]

    assert isinstance(find_example(gs.from_type(Model)).n, int)


def test_from_type_requires_annotations_for_required_arguments():
    class Needs:
        def __init__(self, x, *args, **kwargs):
            self.x = x

    with pytest.raises(hegel.InvalidArgument, match="x"):
        gs.from_type(Needs)
