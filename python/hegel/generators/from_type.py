"""Generators inferred from type annotations."""

from __future__ import annotations

import collections
import collections.abc
import dataclasses
import datetime as dt
import decimal
import enum
import fractions
import inspect
import ipaddress
import types
import typing
import uuid
from collections.abc import Callable
from typing import Any

from hegel.errors import InvalidArgument
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
from hegel.generators.strings import binary, ip_addresses, text, uuids
from hegel.generators.time import dates, datetimes, timedeltas, times

__all__ = ["from_type", "register_type_generator"]

_registry: dict[Any, Any] = {}


def register_type_generator(
    type_: Any, generator: Generator[Any] | Callable[[Any], Generator[Any]]
) -> None:
    """Make ``from_type(type_)`` (and ``builds`` inference) use
    ``generator``, or the generator the callable returns for the type."""
    if not isinstance(generator, Generator) and not callable(generator):
        raise InvalidArgument(
            "register_type_generator takes a generator or a callable returning one"
        )
    _registry[type_] = generator


def _decimals() -> Generator[decimal.Decimal]:
    return one_of(
        integers().map(decimal.Decimal),
        floats(allow_nan=False, allow_infinity=False).map(decimal.Decimal),
    )


def _fractions() -> Generator[fractions.Fraction]:
    return tuples(integers(), integers(min_value=1)).map(lambda p: fractions.Fraction(p[0], p[1]))


def _complex() -> Generator[complex]:
    return tuples(floats(), floats()).map(lambda p: complex(p[0], p[1]))


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
    object: lambda: one_of(none(), booleans(), integers(), floats(), text(), binary()),
    typing.Any: lambda: one_of(none(), booleans(), integers(), floats(), text(), binary()),
}


def _generic(origin: Any, args: tuple[Any, ...]) -> Generator[Any] | None:
    if origin in (
        list,
        collections.abc.MutableSequence,
        collections.abc.Sequence,
        collections.deque,
    ):
        inner = from_type(args[0]) if args else from_type(object)
        result = lists(inner)
        return result.map(collections.deque) if origin is collections.deque else result
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
        if origin is collections.OrderedDict:
            return mapping.map(collections.OrderedDict)
        return mapping
    if origin in (collections.abc.Iterable, collections.abc.Iterator, collections.abc.Collection):
        return iterables(from_type(args[0]) if args else integers())
    if origin is tuple:
        if not args:
            return lists(integers()).map(tuple)
        if len(args) == 2 and args[1] is Ellipsis:
            return lists(from_type(args[0])).map(tuple)
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


def from_type(type_: Any) -> Generator[Any]:
    """A generator of instances of ``type_``: builtins, standard-library
    value types, ``typing`` constructs, enums, dataclasses and classes with
    annotated constructors, or anything registered with
    :func:`register_type_generator`."""
    if type_ in _registry:
        registered = _registry[type_]
        return registered if isinstance(registered, Generator) else registered(type_)
    if isinstance(type_, str):
        raise InvalidArgument(f"from_type({type_!r}): string annotations must be resolved first")
    if type_ is None:
        return none()
    if type_ in _BUILTINS:
        return _BUILTINS[type_]()
    if type_ is type:
        return just(object)
    origin = typing.get_origin(type_)
    if origin is not None:
        generic = _generic(origin, typing.get_args(type_))
        if generic is not None:
            return generic
        if origin in _registry:
            registered = _registry[origin]
            return registered if isinstance(registered, Generator) else registered(type_)
        return from_type(origin)
    if isinstance(type_, typing.TypeVar):
        bound = getattr(type_, "__bound__", None)
        constraints = getattr(type_, "__constraints__", ())
        if bound is not None:
            return from_type(bound)
        if constraints:
            return one_of(*(from_type(c) for c in constraints))
        return from_type(object)
    if isinstance(type_, typing.NewType):
        return from_type(type_.__supertype__)
    if not isinstance(type_, type):
        raise InvalidArgument(f"from_type({type_!r}): not a type")
    if issubclass(type_, enum.Enum):
        return sampled_from(type_)
    if dataclasses.is_dataclass(type_):
        from hegel.generators.composite import builds

        return builds(type_)
    if hasattr(type_, "__annotations__") and typing.is_typeddict(type_):
        hints = typing.get_type_hints(type_)
        required = getattr(type_, "__required_keys__", set(hints))
        mapping = {k: from_type(v) for k, v in hints.items() if k in required}
        optional = {k: from_type(v) for k, v in hints.items() if k not in required}
        return fixed_dicts(mapping, optional=optional).map(type_)
    try:
        signature = _signature(type_)
    except (TypeError, ValueError):
        raise InvalidArgument(
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
                raise InvalidArgument(
                    f"Cannot infer a generator for {type_!r}: required argument "
                    f"{parameter.name!r} has no type annotation"
                )
        elif (
            annotation is not inspect.Parameter.empty
            and parameter.kind is not inspect.Parameter.POSITIONAL_ONLY
        ):
            # Unlike builds(), from_type() asks for arbitrary instances, so
            # annotated optional arguments are either defaulted or generated.
            optional[parameter.name] = one_of(just(parameter.default), from_type(annotation))
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
    overlaid with those of ``__init__``."""
    hints: dict[str, Any] = {}
    sources = [target, target.__init__] if isinstance(target, type) else [target]
    for source in sources:
        try:
            hints.update(typing.get_type_hints(source))
        except Exception:
            pass
    hints.pop("return", None)
    return hints


def _looks_like_a_type(annotation: Any) -> bool:
    return (
        isinstance(annotation, (type, typing.TypeVar, typing.NewType, typing.ForwardRef))
        or typing.get_origin(annotation) is not None
        or annotation is typing.Any
        or isinstance(annotation, typing._SpecialForm)  # noqa: SLF001
    )


def infer_missing_arguments(
    target: Callable[..., Any], positional_count: int, kwargs: dict[str, Generator[Any]]
) -> dict[str, Generator[Any]]:
    """Fill in generators for required parameters of ``target`` that have
    type annotations and were not supplied."""
    try:
        signature = _signature(target)
    except (TypeError, ValueError):
        return kwargs
    hints = _type_hints(target)
    result = dict(kwargs)
    parameters = list(signature.parameters.values())
    for index, parameter in enumerate(parameters):
        if parameter.name in result or parameter.default is not inspect.Parameter.empty:
            continue
        if parameter.kind in (inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD):
            continue
        if parameter.kind is inspect.Parameter.POSITIONAL_ONLY or (
            parameter.kind is inspect.Parameter.POSITIONAL_OR_KEYWORD and index < positional_count
        ):
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
        result[parameter.name] = from_type(annotation)
    return result
