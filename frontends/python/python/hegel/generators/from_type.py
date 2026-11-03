"""Generators inferred from type annotations."""

from __future__ import annotations

import collections
import collections.abc
import dataclasses
import datetime as dt
import decimal
import enum
import fractions
import functools
import inspect
import ipaddress
import sys
import types
import typing
import uuid
from collections.abc import Callable
from typing import Any

from hegel.errors import InvalidArgument, ResolutionFailed
from hegel.generators.base import Generator
from hegel.generators.collections import (
    dicts,
    fixed_dicts,
    frozensets,
    iterables,
    lists,
    sets,
    tuples,
)
from hegel.generators.combinators import just, none, one_of, sampled_from
from hegel.generators.numeric import booleans, floats, integers
from hegel.generators.recursive import Deferred
from hegel.generators.strings import binary, ip_addresses, text, uuids
from hegel.generators.time import dates, datetimes, timedeltas, times

__all__ = ["from_type", "register_type_generator"]

_registry: dict[Any, Any] = {}
_TypeAliasType: Any = getattr(typing, "TypeAliasType", None)  # Python 3.12+


def register_type_generator(
    type_: Any, generator: Generator[Any] | Callable[[Any], Generator[Any]]
) -> None:
    """Make :func:`from_type` of ``type_`` (and so :func:`builds` inference
    of a parameter annotated with it) use ``generator``.

    ``generator`` is a generator, or a callable that takes the type asked
    for (a parameterisation such as ``MyBox[int]``, for a generic) and
    returns one. ``from_type`` of a class with no registration of its own
    but with registered subclasses draws from those subclasses' generators.
    Registering again replaces the earlier registration. A string in
    place of a type is an :class:`~hegel.InvalidArgument`.

    ::

        gs.register_type_generator(Money, gs.integers(0, 10**6).map(Money))
    """
    if isinstance(type_, str):
        raise InvalidArgument(
            f"register_type_generator takes a type, not the string {type_!r}: "
            "register the type the annotation names"
        )
    if not isinstance(generator, Generator) and not callable(generator):
        raise InvalidArgument(
            "register_type_generator takes a generator or a callable returning one"
        )
    _registry[type_] = generator


def _known(mapping: dict[Any, Any], key: Any) -> bool:
    """``key in mapping``, treating an unhashable key (a class whose metaclass
    sets ``__hash__ = None``) as unknown."""
    try:
        return key in mapping
    except TypeError:
        return False


def _decimals() -> Generator[decimal.Decimal]:
    return one_of(
        integers().map(decimal.Decimal),
        floats(allow_nan=False, allow_infinity=False).map(decimal.Decimal),
    )


def _fractions() -> Generator[fractions.Fraction]:
    return tuples(integers(), integers(min_value=1)).map(lambda p: fractions.Fraction(p[0], p[1]))


def _complex() -> Generator[complex]:
    return tuples(floats(), floats()).map(lambda p: complex(p[0], p[1]))


def _networks(version: int) -> Generator[Any]:
    bits = 32 if version == 4 else 128
    network = ipaddress.IPv4Network if version == 4 else ipaddress.IPv6Network
    return tuples(ip_addresses(v=version), integers(0, bits)).map(
        lambda p: network((p[0], p[1]), strict=False)
    )


def _interfaces(version: int) -> Generator[Any]:
    bits = 32 if version == 4 else 128
    interface = ipaddress.IPv4Interface if version == 4 else ipaddress.IPv6Interface
    return tuples(ip_addresses(v=version), integers(0, bits)).map(lambda p: interface(p))


def _zip() -> Generator[Any]:
    # Before Python 3.13 ``zip`` has no signature, so unlike the other
    # builtins whose constructor takes no required argument it cannot be
    # inferred from one.
    from hegel.generators.composite import builds

    return builds(zip)


_BUILTINS: dict[Any, Callable[[], Generator[Any]]] = {
    type(None): none,
    bool: booleans,
    int: integers,
    float: floats,
    complex: _complex,
    str: text,
    bytes: binary,
    bytearray: lambda: binary().map(bytearray),
    decimal.Decimal: _decimals,
    fractions.Fraction: _fractions,
    dt.date: dates,
    dt.time: times,
    dt.datetime: datetimes,
    dt.timedelta: timedeltas,
    uuid.UUID: uuids,
    ipaddress.IPv4Address: lambda: ip_addresses(v=4),
    ipaddress.IPv6Address: lambda: ip_addresses(v=6),
    ipaddress.IPv4Network: lambda: _networks(4),
    ipaddress.IPv6Network: lambda: _networks(6),
    ipaddress.IPv4Interface: lambda: _interfaces(4),
    ipaddress.IPv6Interface: lambda: _interfaces(6),
    object: lambda: one_of(none(), booleans(), integers(), floats(), text(), binary()),
    typing.Any: lambda: one_of(none(), booleans(), integers(), floats(), text(), binary()),
    zip: _zip,
}


def _generic(origin: Any, args: tuple[Any, ...]) -> Generator[Any] | None:
    if origin in (
        list,
        collections.abc.MutableSequence,
        collections.abc.Sequence,
        collections.deque,
    ):
        element = args[0] if args else object
        result = lists(from_type(element))
        if origin is collections.deque:
            return result.map(collections.deque).print_as_call("deque")
        if origin is collections.abc.Sequence and _bytes_satisfies(element):
            return one_of(result, binary())
        return result
    if origin in (set, collections.abc.MutableSet):
        return sets(from_type(args[0]) if args else integers())
    if origin in (frozenset, collections.abc.Set):
        return frozensets(from_type(args[0]) if args else integers())
    if origin in (
        dict,
        collections.abc.MutableMapping,
        collections.abc.Mapping,
        collections.OrderedDict,
    ):
        keys = from_type(args[0]) if args else integers()
        values = from_type(args[1]) if len(args) > 1 else integers()
        mapping = dicts(keys, values)
        return (
            mapping.map(collections.OrderedDict).print_as_call("OrderedDict")
            if origin is collections.OrderedDict
            else mapping
        )
    if origin in (collections.abc.Iterable, collections.abc.Iterator, collections.abc.Collection):
        element = args[0] if args else int
        iterable = iterables(from_type(element))
        if origin is not collections.abc.Iterator and _bytes_satisfies(element):
            return one_of(iterable, binary())
        return iterable
    if origin is tuple:
        if not args:
            return lists(integers()).map(tuple).print_as_call("tuple")
        if len(args) == 2 and args[1] is Ellipsis:
            return lists(from_type(args[0])).map(tuple).print_as_call("tuple")
        return tuples(*(from_type(a) for a in args))
    if origin in (typing.Union, types.UnionType):
        return one_of(*(from_type(a) for a in args))
    if origin is typing.Literal:
        return sampled_from(list(args))
    if origin is type:
        return just(args[0]) if args else just(object)
    if origin is typing.Annotated:
        return from_type(args[0])
    return None


def _bytes_satisfies(element: Any) -> bool:
    """Whether a ``bytes`` object is a sequence of ``element``: it is one of
    ``int``s, so it is also one of any supertype of ``int`` (``object``,
    ``numbers.Real``, ...) and of any union with such a member."""
    if typing.get_origin(element) in (typing.Union, types.UnionType):
        return any(_bytes_satisfies(member) for member in typing.get_args(element))
    return isinstance(element, type) and _safe_issubclass(int, element)


def from_type(type_: Any) -> Generator[Any]:
    """A generator of instances of ``type_``, inferred from the type.

    ``type_`` may be:

    - a type registered with :func:`register_type_generator`;
    - a builtin or standard-library value type (``int``, ``str``,
      ``bytes``, ``float``, ``datetime.date``, ``uuid.UUID``,
      ``decimal.Decimal``, ``ipaddress`` types, ...);
    - a parameterised collection or ``typing`` construct
      (``list[int]``, ``dict[str, float]``, ``tuple[int, ...]``,
      ``int | None``, ``Literal[...]``, ``Optional``, ``Annotated``,
      ``NewType``, a ``TypeVar`` by its bound or constraints, and type
      aliases);
    - an :class:`~enum.Enum`, sampled with :func:`sampled_from`;
    - a dataclass or ``TypedDict``, built from its field annotations;
    - an abstract class or protocol, by its resolvable subclasses;
    - any other class whose constructor's required parameters are all
      annotated, built with :func:`builds` (annotated optional parameters
      are either left to their defaults or generated).

    A type that cannot be resolved (a required constructor parameter
    without an annotation, an abstract class or protocol with no
    resolvable subclass, an unresolved string annotation) is a
    :class:`~hegel.ResolutionFailed`, raised when the generator is
    built.

    ::

        pairs = tc.draw(gs.from_type(list[tuple[int, str]]))
    """
    if _known(_registry, type_):
        return _registered(type_, type_)
    if isinstance(type_, str):
        raise ResolutionFailed(f"from_type({type_!r}): string annotations must be resolved first")
    if isinstance(type_, typing.ForwardRef):
        return from_type(_resolve_forward_ref(type_, None))
    if type_ is None:
        return none()
    if _TypeAliasType is not None and isinstance(type_, _TypeAliasType):  # unreachable on py3.11
        return from_type(type_.__value__)
    if _known(_BUILTINS, type_):
        return _BUILTINS[type_]()
    if type_ is type:
        return just(object)
    if type_ is typing.Annotated:
        # A class before Python 3.13 (a special form after), so not rejected
        # as a non-type below.
        raise ResolutionFailed("from_type(typing.Annotated): Annotated needs a type argument")
    origin = typing.get_origin(type_)
    if origin is tuple and hasattr(type_, "__args__") and not type_.__args__:
        return tuples()  # ``tuple[()]``, unlike a bare ``typing.Tuple``, is the empty tuple
    if _TypeAliasType is not None and isinstance(origin, _TypeAliasType):  # unreachable on py3.11
        if origin in _registry:
            return _registered(origin, type_)
        try:
            return from_type(origin.__value__[typing.get_args(type_)])
        except TypeError:
            return from_type(origin.__value__)
    if origin is not None:
        generic = _generic(origin, typing.get_args(type_))
        if generic is not None:
            return generic
        if origin in _registry:
            return _registered(origin, type_)
        return from_type(origin)
    if isinstance(type_, typing.TypeVar):
        bound = getattr(type_, "__bound__", None)
        constraints = getattr(type_, "__constraints__", ())
        if isinstance(bound, typing.ForwardRef):
            bound = _resolve_forward_ref(bound, type_)
        if bound is not None:
            return from_type(bound)
        if constraints:
            return one_of(*(from_type(c) for c in constraints))
        return from_type(object)
    if isinstance(type_, typing.NewType):
        return from_type(type_.__supertype__)
    if not isinstance(type_, type):
        raise ResolutionFailed(f"from_type({type_!r}): not a type")
    subclasses = _registered_subclasses(type_)
    if subclasses:
        return one_of(*(_registered(sub, sub) for sub in subclasses))
    if issubclass(type_, enum.Enum):
        return sampled_from(type_)
    if dataclasses.is_dataclass(type_):
        from hegel.generators.composite import builds

        return builds(type_)
    if inspect.isabstract(type_) or getattr(type_, "_is_protocol", False):
        # Neither an abstract class nor a Protocol can be instantiated.
        return _abstract(type_)
    if typing.is_typeddict(type_):
        try:
            hints = typing.get_type_hints(type_)
        except Exception as error:
            raise ResolutionFailed(
                f"Cannot infer a generator for {type_!r}: its annotations could not "
                f"be resolved ({error})"
            ) from None
        required = getattr(type_, "__required_keys__", set(hints))
        mapping = {k: from_type(v) for k, v in hints.items() if k in required}
        extras = {k: from_type(v) for k, v in hints.items() if k not in required}
        return fixed_dicts(mapping, optional=extras).map(type_).print_as_call(type_.__name__)
    generic = _generic(type_, ())
    if generic is not None:
        return generic  # a bare collection class, such as ``dict`` or ``deque``
    try:
        signature = _signature(type_)
    except (TypeError, ValueError):
        raise ResolutionFailed(
            f"Cannot infer a generator for {type_!r}: no usable signature"
        ) from None
    from hegel.generators.composite import builds

    hints = _type_hints(type_)
    optional: dict[str, Generator[Any]] = {}
    for parameter in signature.parameters.values():
        if parameter.kind not in (
            inspect.Parameter.POSITIONAL_ONLY,
            inspect.Parameter.POSITIONAL_OR_KEYWORD,
            inspect.Parameter.KEYWORD_ONLY,
        ):
            continue
        annotation = hints.get(parameter.name, parameter.annotation)
        if parameter.default is inspect.Parameter.empty:
            if annotation is inspect.Parameter.empty:
                raise ResolutionFailed(
                    f"Cannot infer a generator for {type_!r}: required argument "
                    f"{parameter.name!r} has no type annotation"
                )
        elif (
            annotation is not inspect.Parameter.empty
            and parameter.kind is not inspect.Parameter.POSITIONAL_ONLY
        ):
            # Unlike builds(), from_type() asks for arbitrary instances, so
            # annotated optional arguments are either defaulted or generated.
            # The annotation may name this very type, so resolve it lazily.
            optional[parameter.name] = one_of(
                just(parameter.default), Deferred(functools.partial(from_type, annotation))
            )
    return builds(type_, **optional)


def _signature(target: Any) -> inspect.Signature:
    """``inspect.signature``, minus a spurious ``self`` from a class's explicit
    ``__signature__`` (as pydantic-style code generation sets it)."""
    signature = inspect.signature(target)
    if isinstance(target, type) and isinstance(
        getattr(target, "__signature__", None), inspect.Signature
    ):
        parameters = list(signature.parameters.values())
        if (
            parameters
            and parameters[0].name == "self"
            and parameters[0].default is inspect.Parameter.empty
            and parameters[0].kind.name.startswith("POSITIONAL_")
        ):
            signature = signature.replace(parameters=parameters[1:])
    return signature


def _type_hints(target: Any) -> dict[str, Any]:
    """Resolved annotations of ``target``: for a class, its own annotations
    overlaid with those of ``__init__``; an explicit ``__signature__``
    overrides both, as it is what ``target`` says it accepts."""
    hints: dict[str, Any] = {}
    sources = [target, target.__init__] if isinstance(target, type) else [target]  # type: ignore[misc]
    for source in sources:
        try:
            hints.update(typing.get_type_hints(source))
        except Exception:
            pass
    hints.pop("return", None)
    signature = getattr(target, "__signature__", None)
    if isinstance(signature, inspect.Signature):
        for parameter in signature.parameters.values():
            annotation = parameter.annotation
            if isinstance(annotation, str):
                annotation = typing.ForwardRef(annotation)
            if isinstance(annotation, typing.ForwardRef):
                try:
                    annotation = _resolve_forward_ref(annotation, target)
                except ResolutionFailed:
                    continue  # keep whatever the annotations say
            if annotation is not inspect.Parameter.empty:
                hints[parameter.name] = annotation
    return hints


def _looks_like_a_type(annotation: Any) -> bool:
    return (
        isinstance(annotation, (type, typing.TypeVar, typing.NewType, typing.ForwardRef))
        or typing.get_origin(annotation) is not None
        or annotation is typing.Any
        or isinstance(annotation, typing._SpecialForm)  # noqa: SLF001
    )


def _registered(key: Any, type_: Any) -> Generator[Any]:
    registered = _registry[key]
    result = registered if isinstance(registered, Generator) else registered(type_)
    if not isinstance(result, Generator):
        raise ResolutionFailed(
            f"The generator registered for {key!r} returned {result!r}, not a generator"
        )
    return result


def _registered_subclasses(type_: Any) -> list[Any]:
    """Registered types that are strict subclasses of ``type_`` and of no
    other known type, in Hypothesis's order of preference."""
    known = [
        k for k in list(_registry) + list(_BUILTINS) if isinstance(k, type) and k is not object
    ]
    return [
        k
        for k in _registry
        if isinstance(k, type)
        and k is not type_
        and _safe_issubclass(k, type_)
        and sum(_safe_issubclass(k, other) for other in known) == 1
    ]


def _safe_issubclass(cls: Any, base: Any) -> bool:
    try:
        return issubclass(cls, base)
    except TypeError:  # TypedDicts, non-runtime protocols and the like
        return False


def _abstract(type_: Any) -> Generator[Any]:
    kind = "a Protocol" if getattr(type_, "_is_protocol", False) else "abstract"
    subclasses = type_.__subclasses__()
    if not subclasses:
        raise ResolutionFailed(
            f"Could not resolve {type_!r} to a generator: it is {kind} and has no subclasses"
        )
    options = []
    for subclass in subclasses:
        try:
            options.append(from_type(subclass))
        except InvalidArgument:
            continue
    if not options:
        raise ResolutionFailed(
            f"Could not resolve {type_!r} to a generator: it is {kind} and no subclass "
            "can be resolved"
        )
    return one_of(*options)


def _resolve_forward_ref(ref: typing.ForwardRef, owner: Any) -> Any:
    """The type a forward reference names: a registered ``ForwardRef``,
    or a name evaluated in the module that defined ``owner``."""
    if ref in _registry:
        return ref
    module = getattr(owner, "__module__", None)
    namespace = dict(vars(sys.modules[module])) if module in sys.modules else {}
    try:
        return eval(ref.__forward_arg__, namespace)  # noqa: S307
    except Exception:
        raise ResolutionFailed(
            f"Could not resolve forward reference {ref.__forward_arg__!r}: "
            "register a generator for it with register_type_generator"
        ) from None


def infer_missing_arguments(
    target: Callable[..., Any],
    args: tuple[Generator[Any], ...],
    kwargs: dict[str, Generator[Any]],
) -> tuple[tuple[Generator[Any], ...], dict[str, Generator[Any]]]:
    """Fill in generators for required parameters of ``target`` that have
    type annotations and were not supplied: positional-only parameters are
    appended to ``args`` (while every earlier one is filled), the others
    added to ``kwargs``."""
    try:
        signature = _signature(target)
    except (TypeError, ValueError):
        return args, kwargs
    hints = _type_hints(target)
    positional = list(args)
    result = dict(kwargs)
    parameters = list(signature.parameters.values())
    for index, parameter in enumerate(parameters):
        if parameter.name in result or parameter.default is not inspect.Parameter.empty:
            continue
        if parameter.kind in (inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD):
            continue
        if parameter.kind is not inspect.Parameter.KEYWORD_ONLY and index < len(args):
            continue
        if parameter.kind is inspect.Parameter.POSITIONAL_ONLY and index != len(positional):
            # An earlier positional-only parameter could not be inferred, so
            # this one cannot be passed either.
            continue
        annotation = hints.get(parameter.name, parameter.annotation)
        if annotation is inspect.Parameter.empty:
            continue
        if annotation is Ellipsis:
            continue
        if isinstance(annotation, str):
            raise InvalidArgument(
                f"Cannot infer a generator for parameter {parameter.name!r} of "
                f"{target!r}: its annotation {annotation!r} could not be resolved"
            )
        if not _looks_like_a_type(annotation):
            # Not a type (e.g. ``x: 1``); leave it for the caller to supply.
            continue
        if parameter.kind is inspect.Parameter.POSITIONAL_ONLY:
            positional.append(from_type(annotation))
        else:
            result[parameter.name] = from_type(annotation)
    return tuple(positional), result
