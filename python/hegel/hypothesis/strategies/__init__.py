"""Hypothesis's strategies, as thin layers over Hegel's generators."""

from __future__ import annotations

import ast
import codecs
import collections
import datetime as dt
import decimal
import fractions as _fractions
import functools
import inspect
import ipaddress
import math
import random as _random
import struct
import sys
import textwrap
import time
import typing
import warnings
from collections.abc import (
    AsyncGenerator,
    AsyncIterable,
    AsyncIterator,
    Callable,
    Hashable,
    Iterable,
    Iterator,
    Sequence,
)
from collections.abc import Generator as GeneratorABC
from types import EllipsisType
from typing import Any, Protocol, TypeVar

from hegel import generators as gs
from hegel._draw_names import DrawName
from hegel.generators.base import Generator
from hegel.generators.combinators import Just, Nothing, SampledFrom
from hegel.generators.recursive import Deferred
from hegel.generators.strings import Characters
from hegel.hypothesis.control import current_build_context, currently_in_test_context
from hegel.hypothesis.errors import HypothesisWarning, InvalidArgument, InvalidState
from hegel.hypothesis.strategies._base import (
    NOTHING,
    LazyStrategy,
    SearchStrategy,
    check_strategy,
    check_type,
    check_valid_interval,
    check_valid_sizes,
    defines_strategy,
    is_nothing,
    unwrap_strategies,
)
from hegel.hypothesis.utils.deprecation import note_deprecation
from hegel.test_case import TestCase

__all__ = [
    "DataObject",
    "DrawFn",
    "SearchStrategy",
    "binary",
    "booleans",
    "builds",
    "characters",
    "complex_numbers",
    "composite",
    "data",
    "dates",
    "datetimes",
    "decimals",
    "deferred",
    "dictionaries",
    "emails",
    "fixed_dictionaries",
    "floats",
    "fractions",
    "from_regex",
    "from_type",
    "frozensets",
    "functions",
    "integers",
    "ip_addresses",
    "iterables",
    "just",
    "lists",
    "none",
    "nothing",
    "one_of",
    "permutations",
    "random_module",
    "randoms",
    "recursive",
    "register_type_strategy",
    "runner",
    "sampled_from",
    "sets",
    "shared",
    "slices",
    "text",
    "timedeltas",
    "times",
    "timezone_keys",
    "timezones",
    "tuples",
    "uuids",
]

T = TypeVar("T")
Ex = TypeVar("Ex")


class _NotSet:
    def __repr__(self) -> str:
        return "not_set"


not_set: Any = _NotSet()


# -- simple pass-throughs ---------------------------------------------------


def _passthrough(name: str, function: Any) -> Any:
    def call(*args: Any, **kwargs: Any) -> Any:
        return function(*args, **kwargs)

    call.__name__ = name
    call.__qualname__ = name
    call.__doc__ = function.__doc__
    call.__wrapped__ = function  # type: ignore[attr-defined]
    return defines_strategy(call)


just = _passthrough("just", gs.just)
booleans = _passthrough("booleans", gs.booleans)
binary = _passthrough("binary", gs.binary)


def _builds(target: Callable[..., Any], /, *args: Any, **kwargs: Any) -> Any:
    if any(a is ... for a in args):
        raise InvalidArgument(
            "... was passed as a positional argument to builds(), but is only allowed as a "
            "keyword arg"
        )
    to_infer = sorted(k for k, v in kwargs.items() if v is ...)
    if to_infer:
        hints = _annotations_of(target)
        unannotated = [k for k in to_infer if k not in hints]
        if unannotated:
            raise InvalidArgument(
                f"passed ... for {', '.join(unannotated)}, but we cannot infer a strategy "
                "because these arguments have no type annotation"
            )
        kwargs = {**kwargs, **{k: from_type(hints[k]) for k in to_infer}}
    return gs.builds(
        target,
        *[unwrap_strategies(a) for a in args],
        **{k: unwrap_strategies(v) for k, v in kwargs.items()},
    )


def _annotations_of(target: Callable[..., Any]) -> dict[str, Any]:
    """The parameter annotations of ``target``, resolved where possible."""
    try:
        parameters = inspect.signature(target).parameters
    except (TypeError, ValueError):
        return {}
    hints: dict[str, Any] = {
        name: p.annotation for name, p in parameters.items() if p.annotation is not p.empty
    }
    try:
        hints.update(typing.get_type_hints(target))
    except Exception:  # noqa: BLE001 - unresolvable hints are reported when drawn
        pass
    return {k: v for k, v in hints.items() if k in parameters}


_builds.__doc__ = gs.builds.__doc__
builds = _passthrough("builds", _builds)
timezones = _passthrough("timezones", gs.timezones)
timedeltas = _passthrough("timedeltas", gs.timedeltas)
dates = _passthrough("dates", gs.dates)


def deferred(definition: Callable[[], SearchStrategy[Ex]]) -> SearchStrategy[Ex]:
    """A strategy defined by a function called on first use, so the
    definition can refer to itself (or to strategies defined later)."""
    strategy: Generator[Any]

    def resolve() -> Generator[Any]:
        if not callable(definition):
            raise InvalidArgument(
                f"Expected definition to be a function but got {definition!r} of type "
                f"{type(definition).__name__} instead."
            )
        result = definition()
        if not isinstance(result, SearchStrategy):
            raise InvalidArgument(
                f"Expected definition to return a SearchStrategy but got {result!r} of "
                f"type {type(result).__name__} instead."
            )
        seen = {id(strategy)}
        inner = unwrap_strategies(result)
        while isinstance(inner, Deferred):
            if id(inner) in seen:
                raise InvalidArgument("Definition of deferred strategy refers to itself")
            seen.add(id(inner))
            inner = inner.definition()
        return unwrap_strategies(result)

    strategy = gs.deferred(resolve)
    return SearchStrategy(strategy)


@defines_strategy
def uuids(*, version: int | None = None, allow_nil: bool = False) -> SearchStrategy[Any]:
    """UUIDs, of any RFC 4122 version or the given one."""
    if version not in (None, 1, 2, 3, 4, 5):
        raise InvalidArgument(
            f"version={version!r}, but version must be in (None, 1, 2, 3, 4, 5) "
            "to pass to the uuid.UUID constructor."
        )
    if not isinstance(allow_nil, bool):
        raise InvalidArgument(
            f"Expected bool but got allow_nil={allow_nil!r} (type={type(allow_nil).__name__})"
        )
    if allow_nil and version is not None:
        raise InvalidArgument("The nil UUID is not of any version")
    return SearchStrategy(gs.uuids(version=version, allow_nil=allow_nil))


@defines_strategy
def tuples(*args: SearchStrategy[Any]) -> SearchStrategy[tuple[Any, ...]]:
    """Tuples with one element drawn from each of the given strategies."""
    for arg in args:
        check_strategy(arg)
    return SearchStrategy(gs.tuples(*[unwrap_strategies(a) for a in args]))


@defines_strategy
def ip_addresses(*, v: int | None = None, network: Any = None) -> SearchStrategy[Any]:
    """IP addresses: either version (``v=4`` or ``v=6`` to pick), or those
    within ``network``."""
    if v is not None:
        check_type(int, v, "v")
        if v not in (4, 6):
            raise InvalidArgument(f"v={v!r}, but only v=4 or v=6 are valid")
    if network is None:
        return SearchStrategy(gs.ip_addresses(v=v))
    check_type((str, ipaddress.IPv4Network, ipaddress.IPv6Network), network, "network")
    if isinstance(network, str):
        try:
            network = ipaddress.ip_network(network)
        except ValueError as err:
            raise InvalidArgument(f"network={network!r} is not a valid network: {err}") from None
    if v is not None and v != network.version:
        raise InvalidArgument(f"v={v!r} is incompatible with network={network!r}")
    return SearchStrategy(gs.ip_addresses(network=network))


@defines_strategy
def permutations(values: Sequence[T]) -> SearchStrategy[list[T]]:
    """Permutations of ``values``, which must be a sequence."""
    if not isinstance(values, Sequence):
        raise InvalidArgument(
            f"Expected Sequence but got values={values!r} (type={type(values).__name__})"
        )
    return SearchStrategy(gs.permutations(values))


def none() -> SearchStrategy[None]:
    """Just ``None``."""
    return just(None)


def nothing() -> SearchStrategy[Any]:
    """A strategy with no values: every draw rejects the test case."""
    return NOTHING


@defines_strategy
def integers(min_value: int | None = None, max_value: int | None = None) -> SearchStrategy[int]:
    """Integers, optionally bounded (inclusive)."""
    for name, value in (("min_value", min_value), ("max_value", max_value)):
        if value is not None and (isinstance(value, bool) or not _is_integral(value)):
            raise InvalidArgument(f"{name}={value!r} must be an integer or None")
    if min_value is not None:
        min_value = _to_int(min_value, "min_value", math.ceil)
    if max_value is not None:
        max_value = _to_int(max_value, "max_value", math.floor)
    check_valid_interval(min_value, max_value, "min_value", "max_value")
    return SearchStrategy(gs.integers(min_value, max_value))


def _is_integral(value: Any) -> bool:
    if isinstance(value, int):
        return True
    if isinstance(value, (float, decimal.Decimal, _fractions.Fraction)):
        try:
            return value == int(value)
        except (OverflowError, ValueError):
            return False
    return False


def _to_int(value: Any, name: str, rounding: Callable[[Any], int]) -> int:
    if isinstance(value, int):
        return value
    return rounding(value)


_FLOAT_FORMATS = {16: ("<e", "<H"), 32: ("<f", "<I"), 64: ("<d", "<Q")}
_SMALLEST_NORMALS = {16: 2.0**-14, 32: 2.0**-126, 64: sys.float_info.min}
_F16_MAX = 65504.0


def _float_of(x: Any, width: int) -> float:
    """Round ``x`` to a float of the given width (raising OverflowError when
    it does not fit, like the struct module does)."""
    if width == 64:
        return float(x)
    fmt, _ = _FLOAT_FORMATS[width]
    return struct.unpack(fmt, struct.pack(fmt, float(x)))[0]


def _is_negative(value: float) -> bool:
    return math.copysign(1.0, value) < 0


def _next_up(value: float, width: int) -> float:
    if math.isnan(value) or (math.isinf(value) and value > 0):
        return value
    if value == 0.0 and _is_negative(value):
        return 0.0
    fmt, bits = _FLOAT_FORMATS[width]
    n = struct.unpack(bits, struct.pack(fmt, value))[0]
    n += 1 if value >= 0 else -1
    return struct.unpack(fmt, struct.pack(bits, n))[0]


def _next_down(value: float, width: int) -> float:
    return -_next_up(-value, width)


def _next_down_normal(value: float, width: int, *, allow_subnormal: bool) -> float:
    value = _next_down(value, width)
    if not allow_subnormal and 0 < abs(value) < _SMALLEST_NORMALS[width]:
        return 0.0 if value > 0 else -_SMALLEST_NORMALS[width]
    return value


def _next_up_normal(value: float, width: int, *, allow_subnormal: bool) -> float:
    return -_next_down_normal(-value, width, allow_subnormal=allow_subnormal)


def _check_float_bound(name: str, value: Any) -> None:
    if value is None:
        return
    if not isinstance(value, (int, float, decimal.Decimal, _fractions.Fraction)):
        raise InvalidArgument(f"{name}={value!r} must be a real number or None")
    if math.isnan(value):
        raise InvalidArgument(f"Invalid end point {name}={value!r}")


@defines_strategy
def floats(
    min_value: float | None = None,
    max_value: float | None = None,
    *,
    allow_nan: bool | None = None,
    allow_infinity: bool | None = None,
    allow_subnormal: bool | None = None,
    width: int = 64,
    exclude_min: bool = False,
    exclude_max: bool = False,
) -> SearchStrategy[float]:
    """Floats of the given width (16, 32 or 64 bits), validated and with
    exclusive bounds resolved the way Hypothesis does it."""
    for name, flag in (("exclude_min", exclude_min), ("exclude_max", exclude_max)):
        if not isinstance(flag, bool):
            raise InvalidArgument(
                f"Expected bool but got {name}={flag!r} (type={type(flag).__name__})"
            )
    if allow_nan is None:
        allow_nan = min_value is None and max_value is None
    elif allow_nan and (min_value is not None or max_value is not None):
        raise InvalidArgument(f"Cannot have allow_nan={allow_nan!r}, with min_value or max_value")
    if width not in (16, 32, 64):
        raise InvalidArgument(
            f"Got width={width!r}, but the only valid values are the integers 16, 32, and 64."
        )
    width = int(width)
    _check_float_bound("min_value", min_value)
    _check_float_bound("max_value", max_value)

    min_arg, max_arg = min_value, max_value
    if min_value is not None:
        min_value = _float_of(min_value, width)
        if min_value != min_arg:
            raise InvalidArgument(
                f"min_value={min_arg!r} cannot be exactly represented as a float "
                f"of width {width} - use min_value={min_value!r} instead."
            )
    if max_value is not None:
        max_value = _float_of(max_value, width)
        if max_value != max_arg:
            raise InvalidArgument(
                f"max_value={max_arg!r} cannot be exactly represented as a float "
                f"of width {width} - use max_value={max_value!r} instead."
            )

    if exclude_min and (min_value is None or min_value == math.inf):
        raise InvalidArgument(f"Cannot exclude min_value={min_value!r}")
    if exclude_max and (max_value is None or max_value == -math.inf):
        raise InvalidArgument(f"Cannot exclude max_value={max_value!r}")

    assumed_allow_subnormal = allow_subnormal is None or allow_subnormal
    if min_value is not None and exclude_min:
        min_value = _next_up_normal(min_value, width, allow_subnormal=assumed_allow_subnormal)
        if min_value == min_arg:  # -0.0 -> 0.0: step past both zeros
            min_value = _next_up_normal(min_value, width, allow_subnormal=assumed_allow_subnormal)
    if max_value is not None and exclude_max:
        max_value = _next_down_normal(max_value, width, allow_subnormal=assumed_allow_subnormal)
        if max_value == max_arg:
            max_value = _next_down_normal(max_value, width, allow_subnormal=assumed_allow_subnormal)

    if min_value == -math.inf:
        min_value = None
    if max_value == math.inf:
        max_value = None

    bad_zero_bounds = (
        min_value == max_value == 0
        and _is_negative(max_value)  # type: ignore[arg-type]
        and not _is_negative(min_value)  # type: ignore[arg-type]
    )
    if (
        min_value is not None
        and max_value is not None
        and (min_value > max_value or bad_zero_bounds)
    ):
        msg = (
            f"There are no {width}-bit floating-point values between "
            f"min_value={min_arg!r} and max_value={max_arg!r}"
        )
        if exclude_min or exclude_max:
            msg += f", exclude_min={exclude_min!r} and exclude_max={exclude_max!r}"
        raise InvalidArgument(msg)

    if allow_infinity is None:
        allow_infinity = min_value is None or max_value is None
    elif allow_infinity:
        if min_value is not None and max_value is not None:
            raise InvalidArgument(
                f"Cannot have allow_infinity={allow_infinity!r}, with both min_value and max_value"
            )
    elif min_value == math.inf:
        if min_arg == math.inf:
            raise InvalidArgument("allow_infinity=False excludes min_value=inf")
        raise InvalidArgument(
            f"exclude_min=True turns min_value={min_arg!r} into inf, but allow_infinity=False"
        )
    elif max_value == -math.inf:
        if max_arg == -math.inf:
            raise InvalidArgument("allow_infinity=False excludes max_value=-inf")
        raise InvalidArgument(
            f"exclude_max=True turns max_value={max_arg!r} into -inf, but allow_infinity=False"
        )

    smallest_normal = _SMALLEST_NORMALS[width]
    if allow_subnormal is None:
        if min_value is not None and max_value is not None:
            if min_value == max_value:
                allow_subnormal = -smallest_normal < min_value < smallest_normal
            else:
                allow_subnormal = min_value < smallest_normal and max_value > -smallest_normal
        elif min_value is not None:
            allow_subnormal = min_value < smallest_normal
        elif max_value is not None:
            allow_subnormal = max_value > -smallest_normal
        else:
            allow_subnormal = True
    if allow_subnormal:
        if min_value is not None and min_value >= smallest_normal:
            raise InvalidArgument(
                f"allow_subnormal=True, but minimum value {min_value} excludes values "
                f"below float{width}'s smallest positive normal {smallest_normal}"
            )
        if max_value is not None and max_value <= -smallest_normal:
            raise InvalidArgument(
                f"allow_subnormal=True, but maximum value {max_value} excludes values "
                f"above float{width}'s smallest negative normal {-smallest_normal}"
            )

    if width == 16:
        return SearchStrategy(
            _Float16(
                min_value,
                max_value,
                allow_nan=allow_nan,
                allow_infinity=allow_infinity,
                allow_subnormal=allow_subnormal,
            )
        )
    return SearchStrategy(
        gs.floats(
            min_value,
            max_value,
            allow_nan=allow_nan,
            allow_infinity=allow_infinity,
            allow_subnormal=allow_subnormal,
            width=width,
        )
    )


def _to_f16(value: float) -> float:
    return struct.unpack("<e", struct.pack("<e", value))[0]


class _Float16(Generator[float]):
    """16-bit floats, drawn as 32-bit floats and rounded to the nearest
    representable 16-bit value (re-drawing when that leaves the bounds).
    The bounds have already been validated and made inclusive."""

    def __init__(
        self,
        min_value: float | None,
        max_value: float | None,
        *,
        allow_nan: bool,
        allow_infinity: bool,
        allow_subnormal: bool,
    ) -> None:
        self.min_value = min_value
        self.max_value = max_value
        self.allow_nan = allow_nan
        self.allow_infinity = allow_infinity
        self.allow_subnormal = allow_subnormal
        self.lo = -_F16_MAX if min_value is None else max(min_value, -_F16_MAX)
        self.hi = _F16_MAX if max_value is None else min(max_value, _F16_MAX)
        self.inner: Generator[float]
        if self.lo > self.hi:  # an infinite bound: only that infinity remains
            self.inner = gs.just(math.inf if min_value == math.inf else -math.inf)
        else:
            # 16-bit subnormals are normal 32-bit floats, so let the inner
            # generator infer its own subnormal handling from the bounds.
            self.inner = gs.floats(
                self.lo, self.hi, allow_nan=False, allow_infinity=False, width=32
            )
        self.label = self.inner.label

    def do_draw(self, tc: TestCase) -> float:
        if self.allow_nan or self.allow_infinity:
            special = tc.draw_silent(gs.weighted_booleans(0.05))
            if special:
                choices: list[float] = []
                if self.allow_nan:
                    choices.append(math.nan)
                if self.allow_infinity:
                    if self.max_value is None:
                        choices.append(math.inf)
                    if self.min_value is None:
                        choices.append(-math.inf)
                return tc.draw_silent(gs.sampled_from(choices))
        if self.lo > self.hi:
            return tc.draw_silent(self.inner)
        for _ in range(3):
            value = _to_f16(tc.draw_silent(self.inner))
            if not self.allow_subnormal and 0 < abs(value) < 2.0**-14:
                continue
            if value < self.lo or value > self.hi:
                continue
            return value
        tc.reject()
        raise AssertionError("unreachable")  # nocov

    def __repr__(self) -> str:
        return f"floats(min_value={self.min_value!r}, max_value={self.max_value!r}, width=16)"


@defines_strategy(try_non_lazy=True)
def sampled_from(elements: Any) -> SearchStrategy[Any]:
    """One of the given values (a sequence, or an Enum class)."""
    if isinstance(elements, (set, frozenset)):
        raise InvalidArgument(
            "Cannot sample from a set, since sets have no defined order. "
            f"Use a sequence (e.g. list or tuple) instead: {elements!r}"
        )
    if isinstance(elements, dict) and not isinstance(elements, collections.OrderedDict):
        raise InvalidArgument("Cannot sample from a dict; use sampled_from(list(d)) or similar")
    if isinstance(elements, type) and issubclass(elements, enum_module.Flag):
        return _sampled_flags(elements)
    if isinstance(elements, type) and issubclass(elements, enum_module.Enum):
        return SearchStrategy(gs.sampled_from(elements))
    if isinstance(elements, (str, collections.OrderedDict)):
        elements = list(elements)
    if not isinstance(elements, Sequence):
        hint = ""
        if isinstance(elements, type) and getattr(elements, "__annotations__", None):
            hint = (
                " Did you mean to sample from its annotations? To generate instances "
                "of a dataclass or similar, use builds() or from_type() instead."
            )
        raise InvalidArgument(
            f"Cannot sample from {elements!r} because it is not an ordered collection.{hint}"
        )
    if not elements:
        raise InvalidArgument("Cannot sample from a length-zero sequence.")
    return SearchStrategy(gs.sampled_from(elements))


def _sampled_flags(flag_type: Any) -> SearchStrategy[Any]:
    """Members of an ``enum.Flag``, including the empty value and
    combinations of named flags, as Hypothesis samples them."""
    import operator

    flags = sorted(
        set(flag_type.__members__.values()), key=lambda v: (v.value.bit_count(), v.value)
    )
    flags_with_empty = flags
    if not flags or flags[0].value != 0:
        try:
            flags_with_empty = [*flags, flag_type(0)]
        except TypeError:  # nocov
            pass
    if not flags_with_empty:
        raise InvalidArgument("Cannot sample from a length-zero sequence.")
    inner = [gs.sampled_from(flags_with_empty)]
    if len(flags) > 1:
        inner.append(
            integers(1, len(flags))
            .flatmap(lambda r: sets(sampled_from(flags), min_size=r, max_size=r))
            .map(lambda s: flag_type(functools.reduce(operator.or_, (f.value for f in s))))
        )
    return SearchStrategy(gs.one_of(*[unwrap_strategies(i) for i in inner]))


import enum as enum_module  # noqa: E402


def one_of(*args: Any) -> SearchStrategy[Any]:
    """A value from any of the strategies (one iterable of strategies is
    also accepted)."""
    if len(args) == 1 and not isinstance(args[0], SearchStrategy):
        try:
            args = tuple(args[0])
        except TypeError:
            check_strategy(args[0])
    for arg in args:
        if not isinstance(arg, SearchStrategy):
            raise InvalidArgument(
                f"Expected a SearchStrategy but got {arg!r} (type={type(arg).__name__}). "
                f"Did you mean st.sampled_from({list(args)!r})?"
            )
    if not args:
        return NOTHING
    if len(args) == 1:
        return args[0]
    live = [arg for arg in args if not is_nothing(arg)]
    if not live:
        return NOTHING
    if len(live) == 1:
        return live[0]
    return OneOfStrategy(live)


class OneOfStrategy(SearchStrategy[Any]):
    """``one_of(...)`` of several strategies, built on first use so that
    ``a | b`` never validates its operands at definition time."""

    def __init__(self, strategies: list[SearchStrategy[Any]]) -> None:
        super().__init__()
        self.strategies = strategies

    def _build(self) -> Generator[Any]:
        return gs.one_of(*[unwrap_strategies(s) for s in self.strategies])

    def __repr__(self) -> str:
        return "one_of(" + ", ".join(repr(s) for s in self.strategies) + ")"


# -- collections -------------------------------------------------------------


@defines_strategy
def lists(
    elements: SearchStrategy[Ex],
    *,
    min_size: int = 0,
    max_size: int | None = None,
    unique_by: Callable[[Ex], Hashable] | tuple[Callable[[Ex], Hashable], ...] | None = None,
    unique: bool = False,
) -> SearchStrategy[list[Ex]]:
    check_strategy(elements, "elements")
    check_valid_sizes(min_size, max_size)
    if unique_by is not None and unique:
        raise InvalidArgument("cannot specify both unique and unique_by (you only need to use one)")
    if unique_by is not None:
        if not (
            callable(unique_by)
            or (isinstance(unique_by, tuple) and all(callable(f) for f in unique_by))
        ):
            raise InvalidArgument(
                f"unique_by={unique_by!r} is not a callable or tuple of callables"
            )
    min_size, max_size = int(min_size), None if max_size is None else int(max_size)
    if max_size == 0:
        return just([])
    if _check_nonempty_elements("lists", elements, min_size, max_size):
        return just([])
    if unique or unique_by is not None:
        inner = unwrap_strategies(elements)
        if isinstance(inner, SampledFrom) and min_size > len(inner.elements):
            raise InvalidArgument(
                f"Cannot create a collection of min_size={min_size!r} unique elements with "
                f"values drawn from only {len(inner.elements)} distinct elements"
            )
    return SearchStrategy(
        gs.lists(
            unwrap_strategies(elements),
            min_size=min_size,
            max_size=max_size,
            unique=unique,
            unique_by=unique_by,
        )
    )


def _check_nonempty_elements(
    name: str, elements: Any, min_size: int, max_size: int | None = None
) -> bool:
    """Whether ``elements`` is empty; an error when the collection must
    then be non-empty (or, as in Hypothesis, when a positive max_size was
    asked for)."""
    if not getattr(elements, "is_empty", False):
        return False
    if min_size > 0:
        raise InvalidArgument(
            f"Cannot create non-empty {name} with elements drawn from the strategy "
            f"{elements!r} because it has no values."
        )
    if max_size is not None and max_size > 0:
        raise InvalidArgument(
            f"Cannot create a collection of max_size={max_size!r}, because no elements "
            f"can be drawn from the element strategy {elements!r}"
        )
    return True


@defines_strategy
def sets(
    elements: SearchStrategy[Ex], *, min_size: int = 0, max_size: int | None = None
) -> SearchStrategy[set[Ex]]:
    check_strategy(elements, "elements")
    check_valid_sizes(min_size, max_size)
    if max_size == 0:
        return just(set())
    if _check_nonempty_elements("sets", elements, int(min_size), max_size):
        return just(set())
    return SearchStrategy(
        gs.sets(
            unwrap_strategies(elements),
            min_size=int(min_size),
            max_size=None if max_size is None else int(max_size),
        )
    )


@defines_strategy
def frozensets(
    elements: SearchStrategy[Ex], *, min_size: int = 0, max_size: int | None = None
) -> SearchStrategy[frozenset[Ex]]:
    check_strategy(elements, "elements")
    check_valid_sizes(min_size, max_size)
    if max_size == 0:
        return just(frozenset())
    if _check_nonempty_elements("frozensets", elements, int(min_size), max_size):
        return just(frozenset())
    return SearchStrategy(
        gs.frozensets(
            unwrap_strategies(elements),
            min_size=int(min_size),
            max_size=None if max_size is None else int(max_size),
        )
    )


@defines_strategy
def iterables(
    elements: SearchStrategy[Ex],
    *,
    min_size: int = 0,
    max_size: int | None = None,
    unique_by: Any = None,
    unique: bool = False,
) -> SearchStrategy[Iterable[Ex]]:
    return lists(
        elements, min_size=min_size, max_size=max_size, unique_by=unique_by, unique=unique
    ).map(_PrettyIter)


class _PrettyIter:
    def __init__(self, values: list[Any]) -> None:
        self._values = values
        self._iter = iter(values)

    def __iter__(self) -> Any:
        return self

    def __next__(self) -> Any:
        return next(self._iter)

    def __repr__(self) -> str:
        return f"iter({self._values!r})"


@defines_strategy
def fixed_dictionaries(
    mapping: dict[Any, SearchStrategy[Any]],
    *,
    optional: dict[Any, SearchStrategy[Any]] | None = None,
) -> SearchStrategy[dict[Any, Any]]:
    check_type(dict, mapping, "mapping")
    for k, v in mapping.items():
        check_strategy(v, f"mapping[{k!r}]")
    if optional is not None:
        check_type(dict, optional, "optional")
        for k, v in optional.items():
            check_strategy(v, f"optional[{k!r}]")
        if type(mapping) is not type(optional):
            raise InvalidArgument(
                f"Got arguments of different types: mapping={type(mapping).__name__}, "
                f"optional={type(optional).__name__}"
            )
        if set(mapping) & set(optional):
            raise InvalidArgument(
                "The following keys were in both mapping and optional, "
                f"which is invalid: {set(mapping) & set(optional)!r}"
            )
    return SearchStrategy(
        gs.fixed_dicts(
            {k: unwrap_strategies(v) for k, v in mapping.items()},
            optional=None
            if optional is None
            else {k: unwrap_strategies(v) for k, v in optional.items()},
        )
    )


@defines_strategy
def dictionaries(
    keys: SearchStrategy[Any],
    values: SearchStrategy[Any],
    *,
    dict_class: type = dict,
    min_size: int = 0,
    max_size: int | None = None,
) -> SearchStrategy[dict[Any, Any]]:
    check_strategy(keys, "keys")
    check_strategy(values, "values")
    check_valid_sizes(min_size, max_size)
    if max_size == 0:
        return just(dict_class())
    if _check_nonempty_elements(
        "dictionaries", keys, int(min_size), max_size
    ) or _check_nonempty_elements("dictionaries", values, int(min_size), max_size):
        return just(dict_class())
    strategy = gs.dicts(
        unwrap_strategies(keys),
        unwrap_strategies(values),
        min_size=int(min_size),
        max_size=None if max_size is None else int(max_size),
    )
    if dict_class is dict:
        return SearchStrategy(strategy)
    return SearchStrategy(strategy.map(dict_class))


# -- text -----------------------------------------------------------------------


@defines_strategy
def characters(
    *,
    codec: str | None = None,
    min_codepoint: int | None = None,
    max_codepoint: int | None = None,
    categories: Any = None,
    exclude_categories: Any = None,
    exclude_characters: Any = None,
    include_characters: Any = None,
    blacklist_categories: Any = None,
    whitelist_categories: Any = None,
    blacklist_characters: Any = None,
    whitelist_characters: Any = None,
) -> SearchStrategy[str]:
    """Single characters, selected by codec, codepoint range, Unicode
    category, and explicit inclusions and exclusions."""
    if blacklist_categories is not None:
        if exclude_categories is not None:
            raise InvalidArgument("Cannot pass both exclude_categories and blacklist_categories")
        exclude_categories = blacklist_categories
    if whitelist_categories is not None:
        if categories is not None:
            raise InvalidArgument("Cannot pass both categories and whitelist_categories")
        categories = whitelist_categories
    if blacklist_characters is not None:
        if exclude_characters is not None:
            raise InvalidArgument("Cannot pass both exclude_characters and blacklist_characters")
        exclude_characters = blacklist_characters
    if whitelist_characters is not None:
        if include_characters is not None:
            raise InvalidArgument("Cannot pass both include_characters and whitelist_characters")
        include_characters = whitelist_characters
    for name, value in (("min_codepoint", min_codepoint), ("max_codepoint", max_codepoint)):
        if value is not None and (
            isinstance(value, bool) or not isinstance(value, int) or value < 0
        ):
            raise InvalidArgument(f"{name}={value!r} must be a non-negative integer")
    check_valid_interval(min_codepoint, max_codepoint, "min_codepoint", "max_codepoint")
    if max_codepoint is not None and max_codepoint > sys.maxunicode:
        raise InvalidArgument(f"max_codepoint={max_codepoint!r} is above sys.maxunicode")
    if categories is not None and exclude_categories is not None:
        overlap = set(categories) & set(exclude_categories)
        if overlap:
            raise InvalidArgument(
                f"Categories {sorted(overlap)!r} are present in both categories "
                "and exclude_categories"
            )
    for name, value in (
        ("include_characters", include_characters),
        ("exclude_characters", exclude_characters),
    ):
        if value is None:
            continue
        if not_one_char := [c for c in value if not isinstance(c, str) or len(c) != 1]:
            raise InvalidArgument(
                f"Elements of {name} are required to be a single character, "
                f"but {not_one_char!r} are not."
            )
    if include_characters is not None and exclude_characters is not None:
        overlap = set(include_characters) & set(exclude_characters)
        if overlap:
            raise InvalidArgument(
                f"Characters {sorted(overlap)!r} are present in both "
                f"include_characters={include_characters!r}, and "
                f"exclude_characters={exclude_characters!r}"
            )
    if include_characters is not None and not isinstance(include_characters, str):
        include_characters = "".join(include_characters)
    if exclude_characters is not None and not isinstance(exclude_characters, str):
        exclude_characters = "".join(exclude_characters)
    if (
        min_codepoint is None
        and max_codepoint is None
        and categories is None
        and exclude_categories is None
        and include_characters is not None
        and codec is None
    ):
        raise InvalidArgument(
            "Nothing is excluded by other arguments, so passing only "
            f"include_characters={include_characters!r} would have no effect.  "
            f"Also pass categories=(), or use sampled_from({include_characters!r}) instead."
        )
    if codec is not None:
        check_type(str, codec, "codec")
        try:
            canonical = codecs.lookup(codec).name
        except LookupError:
            raise InvalidArgument(f"codec={codec!r} is not a valid codec name") from None
        codec = {"iso8859-1": "latin-1"}.get(canonical, canonical)
    return SearchStrategy(
        gs.characters(
            codec=codec,
            min_codepoint=min_codepoint,
            max_codepoint=max_codepoint,
            categories=None if categories is None else list(categories),
            exclude_categories=None if exclude_categories is None else list(exclude_categories),
            include_characters=include_characters,
            exclude_characters=exclude_characters,
        )
    )


def _resolve_alphabet(alphabet: Any) -> Generator[str] | None:
    """Resolve the ``alphabet=`` argument of text() and from_regex() to a
    characters() generator when it is statically known (a collection of
    characters, or a sampled_from/characters strategy), ``NOTHING`` when
    it has no characters, or None when only drawing from it can tell."""
    if not isinstance(alphabet, SearchStrategy):
        if isinstance(alphabet, (bytes, dict)):
            raise InvalidArgument(
                f"alphabet={alphabet!r} must be a collection of characters, or a "
                "strategy generating single characters"
            )
        try:
            chars = list(alphabet)
        except TypeError:
            raise InvalidArgument(
                f"alphabet={alphabet!r} must be a collection of characters, or a "
                "strategy generating single characters"
            ) from None
        bad = [c for c in chars if not isinstance(c, str) or len(c) != 1]
        if bad:
            raise InvalidArgument(
                "The following elements in alphabet are not of length one, which leads "
                f"to violation of size constraints: {bad!r}"
            )
        if not chars:
            return gs.nothing()
        return gs.characters(categories=[], include_characters="".join(dict.fromkeys(chars)))
    inner = unwrap_strategies(alphabet)
    if isinstance(inner, Characters):
        return inner
    if isinstance(inner, SampledFrom):
        return _resolve_alphabet(list(inner.elements))
    return None


@defines_strategy
def text(
    alphabet: Any = not_set,
    *,
    min_size: int = 0,
    max_size: int | None = None,
) -> SearchStrategy[str]:
    """Strings of characters from ``alphabet`` (a strategy for single
    characters, or a collection of them; default: UTF-8 encodable)."""
    check_valid_sizes(min_size, max_size)
    min_size = int(min_size)
    if max_size is not None:
        max_size = int(max_size)
    if max_size == 0:
        return just("")
    if alphabet is not_set:
        return SearchStrategy(gs.text(min_size=min_size, max_size=max_size, codec="utf-8"))
    resolved = _resolve_alphabet(alphabet)
    if resolved is None:
        return SearchStrategy(
            gs.text(unwrap_strategies(alphabet), min_size=min_size, max_size=max_size)
        )
    if isinstance(resolved, Nothing):
        if min_size == 0:
            return just("")
        raise InvalidArgument(
            "Cannot create non-empty lists with elements drawn from strategy "
            f"{alphabet!r} because it has no values."
        )
    return SearchStrategy(gs.text(resolved, min_size=min_size, max_size=max_size))


@defines_strategy
def from_regex(
    regex: Any, *, fullmatch: bool = False, alphabet: Any = None
) -> SearchStrategy[str | bytes]:
    check_type(bool, fullmatch, "fullmatch")
    if alphabet is not None:
        check_type((str, Sequence, set, frozenset, SearchStrategy), alphabet, "alphabet")
        if isinstance(getattr(regex, "pattern", regex), bytes):
            raise InvalidArgument("alphabet= is not supported for bytestrings")
        resolved = _resolve_alphabet(alphabet)
        if resolved is None:
            raise InvalidArgument(
                f"alphabet={alphabet!r} must be a collection of characters, or a "
                "strategy for which the set of characters is known in advance "
                "(characters() or sampled_from())"
            )
        if isinstance(resolved, Nothing):
            return NOTHING
        alphabet = resolved
    return gs.from_regex(regex, fullmatch=fullmatch, alphabet=alphabet)  # type: ignore[return-value]


@defines_strategy
def emails(*, domains: SearchStrategy[str] | None = None) -> SearchStrategy[str]:
    if domains is None:
        return SearchStrategy(gs.emails())
    check_strategy(domains, "domains")
    local = text(
        characters(
            codec="ascii",
            categories=("Lu", "Ll", "Nd"),
            include_characters="!#$%&'*+-/=^_`{|}~.",
        ),
        min_size=1,
        max_size=64,
    ).filter(lambda s: ".." not in s and not s.startswith(".") and not s.endswith("."))
    return tuples(local, domains).map(lambda p: "@".join(p))


# -- numbers ------------------------------------------------------------------


def _to_decimal(value: Any, name: str, places: int | None = None) -> decimal.Decimal | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise InvalidArgument(f"{name}={value!r} must be a number, not bool")
    if isinstance(value, _fractions.Fraction):
        converted = decimal.Context(prec=places).divide(value.numerator, value.denominator)
        if converted != value:
            raise InvalidArgument(
                f"{value!r} cannot be exactly represented as a decimal with places={places!r}"
            )
        return converted
    try:
        with decimal.localcontext(decimal.Context()):  # default traps enabled
            return decimal.Decimal(value)
    except (TypeError, ValueError, decimal.InvalidOperation):
        raise InvalidArgument(f"{name}={value!r} cannot be converted to a Decimal") from None


@defines_strategy
def decimals(
    min_value: Any = None,
    max_value: Any = None,
    *,
    allow_nan: bool | None = None,
    allow_infinity: bool | None = None,
    places: int | None = None,
) -> SearchStrategy[decimal.Decimal]:
    """Decimal numbers, optionally bounded and with a fixed number of
    decimal places. All arithmetic is done exactly, independent of the
    caller's decimal context."""
    if places is not None:
        if isinstance(places, bool) or not isinstance(places, int) or places < 0:
            raise InvalidArgument(f"places={places!r} must be a non-negative integer")
    lo = _to_decimal(min_value, "min_value", places)
    hi = _to_decimal(max_value, "max_value", places)
    if lo is not None and lo.is_nan():
        raise InvalidArgument(f"min_value={min_value!r} may not be NaN")
    if hi is not None and hi.is_nan():
        raise InvalidArgument(f"max_value={max_value!r} may not be NaN")
    if lo is not None and lo.is_infinite():
        if lo > 0:
            raise InvalidArgument(f"Invalid min_value={min_value!r}")
        if allow_infinity is False:
            raise InvalidArgument(f"allow_infinity=False, but min_value={min_value!r}")
        lo = None
    if hi is not None and hi.is_infinite():
        if hi < 0:
            raise InvalidArgument(f"Invalid max_value={max_value!r}")
        if allow_infinity is False:
            raise InvalidArgument(f"allow_infinity=False, but max_value={max_value!r}")
        hi = None
    if lo is not None and hi is not None and lo > hi:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    unbounded = lo is None or hi is None
    if allow_nan is None:
        allow_nan = lo is None and hi is None
    elif allow_nan and not (lo is None and hi is None):
        raise InvalidArgument(f"Cannot allow_nan={allow_nan!r} with min_value or max_value")
    if allow_infinity is None:
        allow_infinity = unbounded
    elif allow_infinity and not unbounded:
        raise InvalidArgument("Cannot allow infinity between finite bounds")

    if places is not None:
        scale = 10**places
        int_lo = None if lo is None else math.ceil(_fractions.Fraction(lo) * scale)
        int_hi = None if hi is None else math.floor(_fractions.Fraction(hi) * scale)
        if int_lo is not None and int_hi is not None and int_lo > int_hi:
            raise InvalidArgument(
                f"There are no decimals with {places} places between min_value={min_value!r} "
                f"and max_value={max_value!r}"
            )

        def int_to_decimal(n: int) -> decimal.Decimal:
            return decimal.Decimal(f"{n}E-{places}")  # exact in every context

        finite: SearchStrategy[decimal.Decimal] = integers(int_lo, int_hi).map(int_to_decimal)
    else:

        def fraction_to_decimal(f: _fractions.Fraction) -> decimal.Decimal:
            precision = math.ceil(math.log10(abs(f.numerator) or 1) + math.log10(f.denominator)) + 1
            return decimal.Context(prec=precision or 1).divide(
                decimal.Decimal(f.numerator), f.denominator
            )

        finite = fractions(lo, hi).map(fraction_to_decimal)
    specials: list[decimal.Decimal] = []
    if allow_infinity:
        if hi is None:
            specials.append(decimal.Decimal("Infinity"))
        if lo is None:
            specials.append(decimal.Decimal("-Infinity"))
    if allow_nan:
        specials += [
            decimal.Decimal("NaN"),
            decimal.Decimal("-NaN"),
            decimal.Decimal("sNaN"),
            decimal.Decimal("-sNaN"),
        ]
    if specials:
        return SearchStrategy(gs.one_of(unwrap_strategies(finite), gs.sampled_from(specials)))
    return finite


@defines_strategy
def fractions(
    min_value: Any = None, max_value: Any = None, *, max_denominator: int | None = None
) -> SearchStrategy[_fractions.Fraction]:
    """Rational numbers, optionally bounded and with bounded denominator."""
    lo = _to_fraction(min_value, "min_value")
    hi = _to_fraction(max_value, "max_value")
    if lo is not None and hi is not None and lo > hi:
        raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
    if max_denominator is not None:
        if (
            isinstance(max_denominator, bool)
            or not isinstance(max_denominator, int)
            or max_denominator < 1
        ):
            raise InvalidArgument(f"max_denominator={max_denominator!r} must be a positive integer")
        if lo is not None and lo.denominator > max_denominator:
            raise InvalidArgument(
                f"min_value={min_value!r} has a denominator above "
                f"max_denominator={max_denominator!r}"
            )
        if hi is not None and hi.denominator > max_denominator:
            raise InvalidArgument(
                f"max_value={max_value!r} has a denominator above "
                f"max_denominator={max_denominator!r}"
            )
    denominators = integers(1, max_denominator)

    def build(denominator: int) -> SearchStrategy[_fractions.Fraction]:
        num_lo = None if lo is None else math.ceil(lo * denominator)
        num_hi = None if hi is None else math.floor(hi * denominator)
        if num_lo is not None and num_hi is not None and num_lo > num_hi:
            return SearchStrategy(gs.nothing())
        return integers(num_lo, num_hi).map(lambda n, d=denominator: _fractions.Fraction(n, d))

    if lo is not None and hi is not None and lo == hi:
        return just(lo)
    return denominators.flatmap(build)


def _to_fraction(value: Any, name: str) -> _fractions.Fraction | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise InvalidArgument(f"{name}={value!r} must be a number, not bool")
    try:
        if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
            raise ValueError
        return _fractions.Fraction(value)
    except (TypeError, ValueError, ZeroDivisionError):
        raise InvalidArgument(f"{name}={value!r} cannot be converted to a Fraction") from None


def _cathetus(h: float, a: float) -> float:
    """The other leg of a right triangle with hypotenuse ``h`` and leg
    ``a``: sqrt(h**2 - a**2), computed without overflow or underflow."""
    h, a = math.fabs(h), math.fabs(a)
    if h < a:
        return math.nan
    if math.isinf(h):
        return math.inf
    if h > math.sqrt(sys.float_info.max):
        if h > sys.float_info.max / 2:
            return math.sqrt(h - a) * math.sqrt(h / 2 + a / 2) * math.sqrt(2)
        return math.sqrt(h - a) * math.sqrt(h + a)
    if h < math.sqrt(sys.float_info.min):
        return math.sqrt(h - a) * math.sqrt(h + a)
    return math.sqrt((h - a) * (h + a))


@defines_strategy
def complex_numbers(
    *,
    min_magnitude: float = 0,
    max_magnitude: float | None = None,
    allow_infinity: bool | None = None,
    allow_nan: bool | None = None,
    allow_subnormal: bool = True,
    width: int = 128,
) -> SearchStrategy[complex]:
    """Complex numbers with real and imaginary parts of ``width // 2`` bits,
    optionally bounded in magnitude; drawn as Hypothesis draws them, with
    the real part constrained by the imaginary part already drawn."""
    _check_float_bound("min_magnitude", min_magnitude)
    _check_float_bound("max_magnitude", max_magnitude)
    if min_magnitude is None:
        raise InvalidArgument("Use min_magnitude=0 or omit the argument entirely.")
    if min_magnitude < 0:
        raise InvalidArgument(f"min_magnitude={min_magnitude!r} must not be negative.")
    if max_magnitude is not None and max_magnitude < 0:
        raise InvalidArgument(f"max_magnitude={max_magnitude!r} must not be negative.")
    check_valid_interval(min_magnitude, max_magnitude, "min_magnitude", "max_magnitude")
    check_type(bool, allow_subnormal, "allow_subnormal")
    if max_magnitude == math.inf:
        max_magnitude = None
    if allow_infinity is None:
        allow_infinity = max_magnitude is None
    elif allow_infinity and max_magnitude is not None:
        raise InvalidArgument(
            f"Cannot have allow_infinity={allow_infinity!r} with max_magnitude={max_magnitude!r}"
        )
    if allow_nan is None:
        allow_nan = min_magnitude == 0 and max_magnitude is None
    elif allow_nan and (min_magnitude != 0 or max_magnitude is not None):
        raise InvalidArgument(
            f"Cannot have allow_nan={allow_nan!r}, min_magnitude={min_magnitude!r}, "
            f"max_magnitude={max_magnitude!r}"
        )
    if width not in (32, 64, 128):
        raise InvalidArgument(
            f"width={width!r}, but must be 32, 64 or 128 (other complex dtypes "
            "such as complex192 or complex256 are not supported)"
        )
    component_width = width // 2
    allow_kw: dict[str, Any] = {
        "allow_nan": allow_nan,
        "allow_infinity": allow_infinity,
        # A nonzero normal min_magnitude with a zero imaginary part would make
        # allow_subnormal=True an error for the real part, so True becomes None.
        "allow_subnormal": None if allow_subnormal else allow_subnormal,
        "width": component_width,
    }
    if min_magnitude == 0 and max_magnitude is None:
        return tuples(floats(**allow_kw), floats(**allow_kw)).map(lambda p: complex(p[0], p[1]))

    def constrained_complex(tc: TestCase) -> complex:
        draw = _draw_fn(tc)
        if max_magnitude is None:
            zi = draw(floats(**allow_kw))
            rmax = None
        else:
            bound = _float_of(max_magnitude, component_width)
            zi = draw(floats(-bound, bound, **allow_kw))
            rmax = _float_of(_cathetus(max_magnitude, zi), component_width)
        if min_magnitude == 0 or math.fabs(zi) >= min_magnitude:
            zr = draw(floats(None if rmax is None else -rmax, rmax, **allow_kw))
        else:
            rmin = _float_of(_cathetus(min_magnitude, zi), component_width)
            zr = draw(floats(rmin, rmax, **allow_kw))
        # Draw the sign bool for a given pair of magnitude arguments either
        # always or never (for stable shrinking), but only invert when needed.
        if min_magnitude > 0 and draw(booleans()) and math.fabs(zi) <= min_magnitude:
            zr = -zr
        return complex(zr, zi)

    return SearchStrategy(gs.composite(constrained_complex)())


# -- time -------------------------------------------------------------------------


@defines_strategy
def times(
    min_value: dt.time = dt.time.min, max_value: dt.time = dt.time.max, *, timezones: Any = None
) -> SearchStrategy[dt.time]:
    check_type(dt.time, min_value, "min_value")
    check_type(dt.time, max_value, "max_value")
    if timezones is None or isinstance(timezones, SearchStrategy) and _is_none_strategy(timezones):
        return SearchStrategy(gs.times(min_value, max_value))
    check_strategy(timezones, "timezones")
    return SearchStrategy(gs.times(min_value, max_value, timezones=unwrap_strategies(timezones)))


@defines_strategy
def datetimes(
    min_value: dt.datetime = dt.datetime.min,
    max_value: dt.datetime = dt.datetime.max,
    *,
    timezones: Any = None,
    allow_imaginary: bool = True,
) -> SearchStrategy[dt.datetime]:
    check_type(dt.datetime, min_value, "min_value")
    check_type(dt.datetime, max_value, "max_value")
    check_type(bool, allow_imaginary, "allow_imaginary")
    if min_value.tzinfo is not None:
        raise InvalidArgument(f"min_value={min_value!r} must not have tzinfo")
    if max_value.tzinfo is not None:
        raise InvalidArgument(f"max_value={max_value!r} must not have tzinfo")
    if timezones is None or isinstance(timezones, SearchStrategy) and _is_none_strategy(timezones):
        return SearchStrategy(gs.datetimes(min_value, max_value, allow_imaginary=allow_imaginary))
    check_strategy(timezones, "timezones")
    return SearchStrategy(
        gs.datetimes(
            min_value,
            max_value,
            timezones=unwrap_strategies(timezones),
            allow_imaginary=allow_imaginary,
        )
    )


def _is_none_strategy(strategy: Any) -> bool:
    try:
        strategy = unwrap_strategies(strategy)
    except InvalidArgument:
        return False
    return isinstance(strategy, Just) and strategy.value is None


@defines_strategy
def timezone_keys(*, allow_prefix: bool = True) -> SearchStrategy[str]:
    """IANA time zone keys available on this system."""
    import zoneinfo

    check_type(bool, allow_prefix, "allow_prefix")
    keys = sorted(zoneinfo.available_timezones())
    if not keys:  # nocov
        raise InvalidArgument("No timezone data is available on this system")
    base = sampled_from(keys)
    if not allow_prefix:
        return base
    return tuples(sampled_from(["", "posix/", "right/"]), base).map(lambda p: p[0] + p[1])


# -- structural -------------------------------------------------------------------


@defines_strategy
def recursive(
    base: SearchStrategy[Ex],
    extend: Callable[[SearchStrategy[Any]], SearchStrategy[T]],
    *,
    min_leaves: int | None = None,
    max_leaves: int = 100,
) -> SearchStrategy[Ex | T]:
    check_strategy(base, "base")
    if not callable(extend):
        raise InvalidArgument(f"extend={extend!r} must be callable")
    if isinstance(max_leaves, bool) or not isinstance(max_leaves, int) or max_leaves < 1:
        raise InvalidArgument(f"max_leaves={max_leaves!r} must be a positive integer")
    if min_leaves is not None:
        if isinstance(min_leaves, bool) or not isinstance(min_leaves, int) or min_leaves < 1:
            raise InvalidArgument(f"min_leaves={min_leaves!r} must be a positive integer")
        if min_leaves > max_leaves:
            raise InvalidArgument(f"min_leaves={min_leaves!r} > max_leaves={max_leaves!r}")
    probe = extend(base)
    if probe is base:
        import warnings

        from hegel.hypothesis.errors import HypothesisWarning

        warnings.warn(
            f"extend={_describe_function(extend)} is a no-op, so recursive() will only "
            "ever generate values from the base strategy",
            HypothesisWarning,
            stacklevel=2,
        )

    def extend_generator(subtrees: Generator[Any]) -> Generator[Any]:
        return unwrap_strategies(extend(SearchStrategy(subtrees)))

    return SearchStrategy(
        gs.recursive(unwrap_strategies(base), extend_generator, max_leaves=max_leaves)
    )


def _describe_function(f: Any) -> str:
    from hegel.hypothesis.strategies._base import _describe

    return _describe(f)


class DataObject:
    """``data()``'s value: draws strategies interactively inside the test."""

    def __init__(self, tc: TestCase) -> None:
        self._tc = tc
        self.count = 0

    def __repr__(self) -> str:
        return "data(...)"

    def draw(self, strategy: SearchStrategy[Ex], label: Any = None) -> Ex:
        check_strategy(strategy, "strategy")
        self.count += 1
        # Hypothesis numbers unlabelled draws across every data() object.
        counter = self._tc._per_case("hypothesis.data_draws", lambda: [0])
        counter[0] += 1
        if label is None:
            draw_name, separator = DrawName(f"Draw {counter[0]}", False), ": "
        else:
            draw_name, separator = DrawName(str(label), False), " = "
        from hegel.hypothesis.control import _current_build_context

        started = time.perf_counter()
        try:
            return self._tc._draw_as(unwrap_strategies(strategy), draw_name, separator)
        finally:
            context = _current_build_context.value
            if context is not None:
                context.draw_time += time.perf_counter() - started

    @property
    def conjecture_data(self) -> TestCase:
        return self._tc


class _DataStrategy(SearchStrategy[DataObject]):
    def __init__(self) -> None:
        super().__init__(_Data())

    def map(self, pack: Any) -> Any:
        raise InvalidArgument("Cannot map a data() strategy; draw a value and transform it instead")

    def filter(self, condition: Any) -> Any:
        raise InvalidArgument("Cannot filter a data() strategy; use assume() in the test instead")

    def flatmap(self, expand: Any) -> Any:
        raise InvalidArgument("Cannot flatmap a data() strategy; draw a value and use it instead")


class _Data(Generator[DataObject]):
    def do_draw(self, tc: TestCase) -> DataObject:
        return DataObject(_root(tc))

    def do_draw_and_print(self, tc: TestCase, printer: Any) -> DataObject:
        printer.text("data(...)")
        return DataObject(_root(tc))

    def __repr__(self) -> str:
        return "data()"


def _root(tc: TestCase) -> TestCase:
    """``data()`` is drawn as a test argument, inside a draw span; the
    DataObject draws on the test case itself so its draws are reported."""
    return tc


@defines_strategy(try_non_lazy=True)
def data() -> SearchStrategy[DataObject]:
    return _DataStrategy()


class DrawFn(Protocol):
    def __call__(self, strategy: SearchStrategy[Ex], label: Any = None) -> Ex: ...


def _draw_fn(tc: TestCase) -> DrawFn:
    def draw(strategy: SearchStrategy[Ex], label: Any = None) -> Ex:
        check_strategy(strategy, "strategy")
        return tc.draw_silent(unwrap_strategies(strategy))

    return draw


def composite(f: Callable[..., Ex]) -> Callable[..., SearchStrategy[Ex]]:
    """Define a strategy as a function whose first argument is ``draw``."""
    if isinstance(f, (classmethod, staticmethod)):
        special = type(f)
        inner = f.__func__
        return special(composite(inner))  # type: ignore[return-value]
    if not callable(f):
        raise InvalidArgument(f"composite() takes a function, not {f!r}")
    signature = inspect.signature(f)
    params = list(signature.parameters.values())
    if not params or "POSITIONAL" not in params[0].kind.name:
        raise InvalidArgument(
            "Functions wrapped with composite must take at least one "
            f"positional argument. {f!r} does not"
        )
    if params[0].default is not params[0].empty:
        raise InvalidArgument("A default value for initial argument will never be used")
    if not (
        f is getattr(typing, "_overload_dummy", None) or _first_param_referenced(f, params[0].name)
    ):
        note_deprecation(
            "There is no reason to use @st.composite on a function which "
            "does not call the provided draw() function internally.",
            since="2022-07-17",
            has_codemod=False,
        )
    if getattr(signature.return_annotation, "__origin__", None) is SearchStrategy:
        ret_repr = repr(signature.return_annotation).replace("hegel.hypothesis.strategies.", "st.")
        warnings.warn(
            f"Return-type annotation is `{ret_repr}`, but the decorated "
            "function should return a value (not a strategy)",
            HypothesisWarning,
            stacklevel=2,
        )

    @functools.wraps(f)
    def tc_function(tc: TestCase, *args: Any, **kwargs: Any) -> Ex:
        return f(_draw_fn(tc), *args, **kwargs)

    tc_function.__signature__ = signature  # type: ignore[attr-defined]
    build = gs.composite(tc_function)
    # A function taking only *args receives draw as its first element.
    outer_params = params if params[0].kind is params[0].VAR_POSITIONAL else params[1:]
    outer_signature = signature.replace(parameters=outer_params)

    @functools.wraps(f)
    def accept(*args: Any, **kwargs: Any) -> SearchStrategy[Ex]:
        # Fail now, as calling the function would, rather than when drawn.
        outer_signature.bind(*args, **kwargs)
        return LazyStrategy(build, args, kwargs, name=getattr(f, "__name__", "composite"))

    accept.__signature__ = outer_signature  # type: ignore[attr-defined]
    return accept


def _first_param_referenced(f: Any, name: str) -> bool:
    """Whether ``f``'s body reads the parameter ``name`` (the draw function)."""
    try:
        tree = ast.parse(textwrap.dedent(inspect.getsource(f)))
    except Exception:  # noqa: BLE001
        return True
    return any(
        isinstance(node, ast.Name) and node.id == name and isinstance(node.ctx, ast.Load)
        for node in ast.walk(tree)
    )


@defines_strategy
def shared(base: SearchStrategy[Ex], *, key: Hashable = None) -> SearchStrategy[Ex]:
    """One value per test case, drawn the first time it is needed and
    reused by every ``shared`` strategy with the same key (or the same
    object when no key is given)."""
    check_strategy(base, "base")
    return SearchStrategy(_Shared(unwrap_strategies(base), key))


class _Shared(Generator[Any]):
    def __init__(self, base: Generator[Any], key: Hashable) -> None:
        self.base = base
        self.key = key
        self.label = base.label

    def _cache_key(self) -> Any:
        return ("shared", id(self)) if self.key is None else ("shared", self.key)

    def do_draw(self, tc: TestCase) -> Any:
        store = _per_case_store(tc)
        key = self._cache_key()
        if key not in store:
            store[key] = (tc.draw_silent(self.base), self)
        else:
            _, other = store[key]
            if self.base.label != other.base.label:
                warnings.warn(
                    f"Different strategies are shared under key={self.key!r}. This"
                    " risks drawing values that are not valid for the strategy,"
                    " or that have a narrower range than expected."
                    f" Conflicting strategies: ({self!r}, {other!r}).",
                    HypothesisWarning,
                    stacklevel=1,
                )
        return store[key][0]

    def __repr__(self) -> str:
        return f"shared({self.base!r}, key={self.key!r})"


def _per_case_store(tc: TestCase) -> dict[Any, Any]:
    if currently_in_test_context():
        return current_build_context().shared
    return tc._per_case("hypothesis.shared", dict)


@defines_strategy
def slices(size: int) -> SearchStrategy[slice]:
    """Slices usable on a sequence of ``size`` elements, drawn the way
    Hypothesis draws them so that they shrink toward ``slice(None)``."""
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        raise InvalidArgument(f"size={size!r} must be a non-negative integer")

    def build(tc: TestCase) -> slice:
        draw = _draw_fn(tc)
        if size == 0:
            return slice(None, None, draw(none() | integers().filter(bool)))
        # For slices start is inclusive and stop is exclusive
        start = draw(integers(0, size - 1) | none())
        stop = draw(integers(0, size) | none())
        # Limit step size to be reasonable
        if start is None and stop is None:
            max_step = size
        elif start is None:
            max_step = stop
        elif stop is None:
            max_step = start
        else:
            max_step = abs(start - stop)
        step = draw(integers(1, max_step or 1))
        if (draw(booleans()) and start == stop) or (stop or 0) < (start or 0):
            step *= -1
        if draw(booleans()) and start is not None:
            start -= size
        if draw(booleans()) and stop is not None:
            stop -= size
        if (not draw(booleans())) and step == 1:
            step = None  # type: ignore[assignment]
        return slice(start, stop, step)

    return SearchStrategy(gs.composite(build)())


@defines_strategy
def randoms(
    *, note_method_calls: bool = False, use_true_random: bool = False
) -> SearchStrategy[_random.Random]:
    """``random.Random`` instances with a generated seed; with
    ``note_method_calls`` each call is reported with the failing example."""
    check_type(bool, note_method_calls, "note_method_calls")
    check_type(bool, use_true_random, "use_true_random")
    cls = TrueRandom if use_true_random else HypothesisRandom

    def draw_random(tc: TestCase) -> _random.Random:
        seed = tc.draw_silent(gs.integers(0, 2**64 - 1))
        return cls(seed, note_method_calls=note_method_calls, tc=tc)

    return SearchStrategy(gs.composite(draw_random)())


RANDOM_METHODS = [
    name
    for name in [
        "_randbelow",
        "betavariate",
        "binomialvariate",
        "choice",
        "choices",
        "expovariate",
        "gammavariate",
        "gauss",
        "getrandbits",
        "lognormvariate",
        "normalvariate",
        "paretovariate",
        "randint",
        "random",
        "randrange",
        "sample",
        "shuffle",
        "triangular",
        "uniform",
        "vonmisesvariate",
        "weibullvariate",
        "randbytes",
    ]
    if hasattr(_random.Random, name)
]


def _random_stub_getrandbits(self: Any, n: int) -> int:  # nocov
    raise NotImplementedError


def _random_stub_random(self: Any) -> float:  # nocov
    raise NotImplementedError


def _random_stub_randbelow(self: Any, n: int) -> int:  # nocov
    raise NotImplementedError


_RANDOM_STUBS: dict[str, Any] = {
    "getrandbits": _random_stub_getrandbits,
    "random": _random_stub_random,
    "_randbelow": _random_stub_randbelow,
}
_DUMMY_RANDOM = _random.Random(0)


@functools.cache
def _random_signature(name: str) -> inspect.Signature:
    return inspect.signature(_RANDOM_STUBS.get(name, getattr(_random.Random, name)))


def convert_kwargs(name: str, kwargs: dict[str, Any]) -> tuple[list[Any], dict[str, Any]]:
    """Split keyword arguments to ``Random.<name>`` into the positional and
    keyword parts of the shortest equivalent call."""
    kwargs = dict(kwargs)
    signature = _random_signature(name)
    params = signature.parameters
    bound = signature.bind(_DUMMY_RANDOM, **kwargs)
    bound.apply_defaults()
    for k in list(kwargs):
        if kwargs[k] is params[k].default or params[k].kind != inspect.Parameter.KEYWORD_ONLY:
            kwargs.pop(k)
    arg_names = list(params)[1:]
    args = []
    for a in arg_names:
        if params[a].kind == inspect.Parameter.KEYWORD_ONLY:
            break
        args.append(bound.arguments[a])
        kwargs.pop(a, None)
    while args:
        name = arg_names[len(args) - 1]
        if args[-1] is params[name].default:
            args.pop()
        else:
            break
    return (args, kwargs)


class HypothesisRandom(_random.Random):
    """A ``random.Random`` seeded from the test case, which can report
    its method calls alongside the failing example."""

    def __init__(
        self, seed: int = 0, *, note_method_calls: bool = False, tc: TestCase | None = None
    ) -> None:
        super().__init__(seed)
        self._hegel_seed = seed
        self._note_method_calls = note_method_calls
        self._tc = tc

    def __repr__(self) -> str:
        return f"HypothesisRandom({self._hegel_seed!r})"

    def __copy__(self) -> HypothesisRandom:
        result = type(self)(
            self._hegel_seed, note_method_calls=self._note_method_calls, tc=self._tc
        )
        result.setstate(self.getstate())
        return result

    def __deepcopy__(self, memo: Any) -> HypothesisRandom:
        return self.__copy__()

    def __reduce__(self) -> Any:  # nocov
        return (type(self), (self._hegel_seed,), self.getstate())

    def seed(self, a: Any = None, version: int = 2) -> None:
        super().seed(a, version)
        if isinstance(a, int):
            self._hegel_seed = a

    def _hegel_log_random(self, method: str, kwargs: dict[str, Any], result: Any) -> None:
        if not self._note_method_calls:
            return
        from hegel.hypothesis._settings import Verbosity, settings
        from hegel.hypothesis.reporting import report

        tc = self._tc
        final = tc is not None and tc.is_final
        if not (final or settings.default.verbosity >= Verbosity.verbose):
            return
        args, kwargs = convert_kwargs(method, kwargs)
        argstr = ", ".join([repr(a) for a in args] + [f"{k}={v!r}" for k, v in kwargs.items()])
        text = f"{self!r}.{method}({argstr}) -> {result!r}"
        if final:
            assert tc is not None
            tc.note(text)
        else:
            report(text)


class TrueRandom(HypothesisRandom):
    """A :class:`HypothesisRandom` whose seed is reported as ``Random(seed)``."""

    def __repr__(self) -> str:
        return f"Random({self._hegel_seed!r})"


def _define_noting_method(name: str) -> None:
    target = getattr(_random.Random, name)
    signature = _random_signature(name)

    def implementation(self: HypothesisRandom, *args: Any, **kwargs: Any) -> Any:
        result = target(self, *args, **kwargs)
        if self._note_method_calls:
            bound = signature.bind(self, *args, **kwargs)
            bound.apply_defaults()
            arguments = dict(bound.arguments)
            arguments.pop(next(iter(signature.parameters)))
            self._hegel_log_random(name, arguments, result)
        return result

    implementation.__name__ = name
    implementation.__qualname__ = f"HypothesisRandom.{name}"
    implementation.__doc__ = target.__doc__
    setattr(HypothesisRandom, name, implementation)


for _name in RANDOM_METHODS:
    _define_noting_method(_name)


class _RandomSeeder:
    def __init__(self, seed: int) -> None:
        self.seed = seed

    def __repr__(self) -> str:
        return f"RandomSeeder({self.seed!r})"


@defines_strategy
def random_module() -> SearchStrategy[_RandomSeeder]:
    """Seed the global ``random`` module, and every generator passed to
    ``register_random``, for the test case, restoring their states
    afterwards."""

    def seed_random(tc: TestCase) -> _RandomSeeder:
        from hegel.hypothesis._entropy import get_seeder_and_restorer

        seed = tc.draw_silent(gs.integers(0, 2**32 - 1))
        seed_all, restore_all = get_seeder_and_restorer(seed)
        seed_all()
        if currently_in_test_context():
            from hegel.hypothesis.control import cleanup

            cleanup(restore_all)
        return _RandomSeeder(seed)

    return SearchStrategy(gs.composite(seed_random)())


@defines_strategy
def runner(*, default: Any = not_set) -> SearchStrategy[Any]:
    """The test runner (``self`` of a test method), or ``default``."""

    def get_runner(tc: TestCase) -> Any:
        if currently_in_test_context():
            value = current_build_context().runner
            if value is not None:
                return value
        if default is not_set:
            raise InvalidArgument(
                "Cannot use runner() strategy with no associated runner or explicit default."
            )
        return default

    return SearchStrategy(gs.composite(get_runner)())


@defines_strategy
def functions(
    *,
    like: Callable[..., Any] = lambda: None,
    returns: SearchStrategy[Any] | EllipsisType = ...,
    pure: bool = False,
) -> SearchStrategy[Callable[..., Any]]:
    """Functions with the same signature (and kind: plain, async, generator
    or async generator) as ``like``, returning values drawn from
    ``returns`` (inferred from the return annotation when omitted)."""
    check_type(bool, pure, "pure")
    if not callable(like):
        raise InvalidArgument(
            "The first argument to functions() must be a callable to imitate, "
            f"but got non-callable like={like!r}"
        )
    is_async = inspect.iscoroutinefunction(like)
    is_async_gen = inspect.isasyncgenfunction(like)
    is_gen = inspect.isgeneratorfunction(like) or is_async_gen
    if pure and (is_async or is_gen):
        raise InvalidArgument(
            f"pure=True is invalid for like={getattr(like, '__name__', like)!r}, because "
            "async functions are for non-deterministic IO and generators are consumed "
            "by iteration, so returning a cached value makes no sense"
        )
    if returns is None or returns is ...:
        try:
            hints = typing.get_type_hints(like)
        except Exception:  # noqa: BLE001
            hints = {}
        if is_gen:
            allowed: tuple[Any, ...] = (
                (AsyncIterator, AsyncIterable, AsyncGenerator)
                if is_async_gen
                else (Iterator, Iterable, GeneratorABC)
            )
            ret = hints.get("return")
            kind = typing.get_origin(ret) or ret
            if ret is not None and kind not in allowed:
                options = ", ".join(t.__name__ for t in allowed)
                raise InvalidArgument(
                    f"Cannot infer the yield type of like={getattr(like, '__name__', like)!r} "
                    f"from its return annotation {ret!r}. Expected one of {options}. "
                    "Alternatively, pass returns= to specify the yield type explicitly."
                )
            args = typing.get_args(ret)
            returns = from_type(args[0]) if args else none()
        else:
            returns = from_type(hints.get("return", type(None)))
    check_strategy(returns, "returns")
    strategy = lists(returns) if is_gen else returns
    return SearchStrategy(_Functions(like, unwrap_strategies(strategy), pure))


class _CanVary:
    def __repr__(self) -> str:
        return "can_vary"


_can_vary: Any = _CanVary()


async def _checkpoint() -> None:  # nocov - depends on the installed frameworks
    # Trio-style checkpoint semantics, finding the right way to yield to
    # the user's async framework when it is not asyncio.
    if anyio := sys.modules.get("anyio"):
        await anyio.lowlevel.checkpoint()
        return
    if sniffio := sys.modules.get("sniffio"):
        if sniffio.current_async_library() == "trio":
            await sys.modules["trio"].lowlevel.checkpoint()
            return
    import asyncio

    await asyncio.sleep(0)


class _Functions(Generator[Callable[..., Any]]):
    def __init__(self, like: Callable[..., Any], returns: Generator[Any], pure: bool) -> None:
        self.like = like
        self.returns = returns
        self.pure = pure
        self.label = returns.label
        # A returns strategy with a single possible value - just(), none(),
        # or a one-element sampled_from() - makes the function a constant,
        # shown as a lambda instead of noting each call.
        unwrapped = unwrap_strategies(returns)
        if isinstance(unwrapped, Just):
            self._constant = unwrapped.value
        elif isinstance(unwrapped, SampledFrom) and len(unwrapped.elements) == 1:
            self._constant = unwrapped.elements[0]
        else:
            self._constant = _can_vary

    @property
    def is_empty(self) -> bool:
        return self.returns.is_empty

    def _constant_description(self) -> str:
        sig = inspect.signature(self.like)
        params = str(
            sig.replace(
                parameters=[
                    param.replace(annotation=inspect.Parameter.empty)
                    for param in sig.parameters.values()
                ],
                return_annotation=inspect.Signature.empty,
            )
        )[1:-1]
        return f"lambda {params}: {self._constant!r}" if params else f"lambda: {self._constant!r}"

    def description(self) -> str:
        if self._constant is _can_vary or inspect.iscoroutinefunction(self.like):
            return getattr(self.like, "__name__", repr(self.like))
        return self._constant_description()

    def do_draw(self, tc: TestCase) -> Callable[..., Any]:
        like, returns, pure = self.like, self.returns, self.pure
        varies = self._constant is _can_vary or inspect.iscoroutinefunction(like)
        cache: dict[Any, Any] = {}
        signature = inspect.signature(like)
        context = current_build_context() if currently_in_test_context() else None

        def should_note() -> bool:
            from hegel.hypothesis._settings import Verbosity, settings

            return tc.is_final or settings.default.verbosity >= Verbosity.verbose

        def draw_value(args: tuple[Any, ...], kwargs: dict[str, Any]) -> Any:
            if context is None or not any(c is context for c in _build_context_stack()):
                raise InvalidState(
                    f"This generated {getattr(like, '__name__', like)} function can only "
                    "be called within the scope of the @given that created it."
                )
            if pure:
                bound = signature.bind(*args, **kwargs)
                bound.apply_defaults()
                key = (tuple(bound.args), frozenset(bound.kwargs.items()))
                if key not in cache:
                    cache[key] = tc.draw_silent(returns)
                    if varies and should_note():
                        _note_call(tc, like, args, kwargs, cache[key])
                return cache[key]
            value = tc.draw_silent(returns)
            if varies and should_note():
                _note_call(tc, like, args, kwargs, value)
            return value

        async def async_body(*args: Any, **kwargs: Any) -> Any:
            value = draw_value(args, kwargs)
            await _checkpoint()
            return value

        async def async_gen_body(*args: Any, **kwargs: Any) -> Any:
            for value in draw_value(args, kwargs):
                await _checkpoint()
                yield value
            await _checkpoint()

        def gen_body(*args: Any, **kwargs: Any) -> Any:
            yield from draw_value(args, kwargs)

        def plain_body(*args: Any, **kwargs: Any) -> Any:
            return draw_value(args, kwargs)

        body: Callable[..., Any]
        if inspect.iscoroutinefunction(like):
            body, kind = async_body, "async"
        elif inspect.isasyncgenfunction(like):
            body, kind = async_gen_body, "async gen"
        elif inspect.isgeneratorfunction(like):
            body, kind = gen_body, "gen"
        else:
            body, kind = plain_body, "plain"
        inner = _with_signature(like, signature, body, kind)
        inner.__hegel_signature__ = signature  # type: ignore[attr-defined]
        if not varies:
            inner.__hegel_description__ = self._constant_description()  # type: ignore[attr-defined]
        return inner

    def do_draw_and_print(self, tc: TestCase, printer: Any) -> Callable[..., Any]:
        value = self.do_draw(tc)
        printer.text(self.description())
        return value

    def __repr__(self) -> str:
        return f"functions(like={self.like!r}, returns={self.returns!r}, pure={self.pure!r})"


def _with_signature(
    like: Callable[..., Any], sig: inspect.Signature, body: Callable[..., Any], kind: str
) -> Callable[..., Any]:
    """A function of the same kind (plain, async, generator or async
    generator) and signature as ``like`` that forwards its arguments to
    ``body``, so that calling it with the wrong arguments is a TypeError
    before any value is drawn."""
    namespace: dict[str, Any] = {"__body": body}
    decl: list[str] = []
    call: list[str] = []
    params = list(sig.parameters.values())
    positional_only = [p for p in params if p.kind is p.POSITIONAL_ONLY]
    has_var_positional = any(p.kind is p.VAR_POSITIONAL for p in params)
    star_added = False
    for index, param in enumerate(params):
        default = ""
        if param.default is not param.empty:
            namespace[f"__default{index}"] = param.default
            default = f"=__default{index}"
        if param.kind is param.VAR_POSITIONAL:
            decl.append(f"*{param.name}")
            call.append(f"*{param.name}")
            star_added = True
        elif param.kind is param.VAR_KEYWORD:
            decl.append(f"**{param.name}")
            call.append(f"**{param.name}")
        elif param.kind is param.KEYWORD_ONLY:
            if not has_var_positional and not star_added:
                decl.append("*")
                star_added = True
            decl.append(param.name + default)
            call.append(f"{param.name}={param.name}")
        else:
            decl.append(param.name + default)
            call.append(param.name)
            if positional_only and param is positional_only[-1]:
                decl.append("/")
    arguments = ", ".join(decl)
    forwarded = ", ".join(call)
    if kind == "async":
        source = f"async def inner({arguments}):\n    return await __body({forwarded})\n"
    elif kind == "async gen":
        source = (
            f"async def inner({arguments}):\n"
            f"    async for value in __body({forwarded}):\n        yield value\n"
        )
    elif kind == "gen":
        source = f"def inner({arguments}):\n    yield from __body({forwarded})\n"
    else:
        source = f"def inner({arguments}):\n    return __body({forwarded})\n"
    exec(source, namespace)  # noqa: S102
    inner = namespace["inner"]
    for attribute in ("__module__", "__name__", "__qualname__", "__doc__"):
        try:
            setattr(inner, attribute, getattr(like, attribute))
        except AttributeError:  # nocov
            pass
    inner.__dict__.update(getattr(like, "__dict__", {}))
    return inner


def _build_context_stack() -> list[Any]:
    from hegel.hypothesis.control import _stack

    return _stack()


def _note_call(
    tc: TestCase,
    like: Callable[..., Any],
    args: tuple[Any, ...],
    kwargs: dict[str, Any],
    value: Any,
) -> None:
    from hegel.hypothesis.reporting import report

    bits = [repr(a) for a in args] + [f"{k}={v!r}" for k, v in kwargs.items()]
    text = f"Called function: {getattr(like, '__name__', like)}({', '.join(bits)}) -> {value!r}"
    if tc.is_final:
        tc.note(text)
    else:
        report(text)


def register_type_strategy(
    custom_type: Any, strategy: SearchStrategy[Any] | Callable[[Any], SearchStrategy[Any]]
) -> None:
    """Make ``from_type(custom_type)`` use ``strategy`` (or the strategy
    the callable returns for the type)."""
    if not _is_a_type(custom_type):
        raise InvalidArgument(f"custom_type={custom_type!r} must be a type")
    if typing.get_args(custom_type):
        raise InvalidArgument(
            f"Cannot register generic type {custom_type!r}, because it has type "
            "arguments which would not be handled. Instead, register a function for "
            f"{typing.get_origin(custom_type)!r} which can inspect specific type "
            "objects and return a strategy"
        )
    if not isinstance(strategy, SearchStrategy) and not callable(strategy):
        raise InvalidArgument(
            f"strategy={strategy!r} must be a SearchStrategy, or a function that "
            "takes a type and returns one"
        )
    if isinstance(strategy, SearchStrategy):
        if strategy.is_empty:
            raise InvalidArgument(f"strategy={strategy!r} cannot be an empty strategy")
        gs.register_type_generator(custom_type, strategy.generator)
        return

    def resolve(type_: Any) -> Any:
        result = strategy(type_)
        return result.generator if isinstance(result, SearchStrategy) else result

    gs.register_type_generator(custom_type, resolve)


_TypeAliasType: Any = getattr(typing, "TypeAliasType", None)  # Python 3.12+


def _is_a_type(thing: Any) -> bool:
    return (
        isinstance(thing, (type, typing.TypeVar, typing.NewType, typing.ForwardRef))
        or (_TypeAliasType is not None and isinstance(thing, _TypeAliasType))
        or typing.get_origin(thing) is not None
        or thing in (typing.Any,)
    )


@defines_strategy
def from_type(thing: Any) -> SearchStrategy[Any]:
    """A strategy for instances of ``thing``, from its annotations or the
    registered strategy for it."""
    return SearchStrategy(gs.from_type(thing))
