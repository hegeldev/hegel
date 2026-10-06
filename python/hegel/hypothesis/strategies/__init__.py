"""Hypothesis's strategies, as thin layers over Hegel's generators."""

from __future__ import annotations

import datetime as dt
import decimal
import fractions as _fractions
import functools
import inspect
import math
import random as _random
import struct
import sys
from collections.abc import Callable, Hashable, Iterable, Sequence
from typing import Any, Protocol, TypeVar

from hegel import generators as gs
from hegel.generators.base import Generator
from hegel.generators.combinators import Just, Nothing, OneOf
from hegel.hypothesis.control import current_build_context, currently_in_test_context
from hegel.hypothesis.errors import InvalidArgument
from hegel.hypothesis.strategies._base import (
    NOTHING,
    LazyStrategy,
    SearchStrategy,
    check_strategy,
    check_type,
    check_valid_interval,
    check_valid_sizes,
    defines_strategy,
    unwrap_strategies,
)
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
    call.__wrapped__ = function
    return defines_strategy(call)


just = _passthrough("just", gs.just)
booleans = _passthrough("booleans", gs.booleans)
binary = _passthrough("binary", gs.binary)
uuids = _passthrough("uuids", gs.uuids)
ip_addresses = _passthrough("ip_addresses", gs.ip_addresses)
tuples = _passthrough("tuples", gs.tuples)
permutations = _passthrough("permutations", gs.permutations)
builds = _passthrough("builds", gs.builds)
timezones = _passthrough("timezones", gs.timezones)
timedeltas = _passthrough("timedeltas", gs.timedeltas)
dates = _passthrough("dates", gs.dates)
deferred = gs.deferred


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
    return gs.integers(min_value, max_value)


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
    """Floats of the given width (16, 32 or 64 bits)."""
    if width not in (16, 32, 64):
        raise InvalidArgument(
            f"Got width={width!r}, but the only valid values are the integers 16, 32, and 64."
        )
    for name, value in (("min_value", min_value), ("max_value", max_value)):
        if value is not None and not isinstance(
            value, (int, float, decimal.Decimal, _fractions.Fraction)
        ):
            raise InvalidArgument(f"{name}={value!r} must be a real number or None")
    if min_value is not None:
        min_value = float(min_value)
    if max_value is not None:
        max_value = float(max_value)
    if width == 16:
        return _Float16(
            min_value,
            max_value,
            allow_nan=allow_nan,
            allow_infinity=allow_infinity,
            allow_subnormal=allow_subnormal,
            exclude_min=exclude_min,
            exclude_max=exclude_max,
        )
    return gs.floats(
        min_value,
        max_value,
        allow_nan=allow_nan,
        allow_infinity=allow_infinity,
        allow_subnormal=allow_subnormal,
        width=width,
        exclude_min=exclude_min,
        exclude_max=exclude_max,
    )


_F16_MAX = 65504.0
_F16_SMALLEST_SUBNORMAL = 2.0**-24


def _to_f16(value: float) -> float:
    return struct.unpack("<e", struct.pack("<e", value))[0]


class _Float16(Generator[float]):
    """16-bit floats, drawn as 32-bit floats and rounded to the nearest
    representable 16-bit value (re-drawing when that leaves the bounds)."""

    def __init__(
        self,
        min_value: float | None,
        max_value: float | None,
        *,
        allow_nan: bool | None,
        allow_infinity: bool | None,
        allow_subnormal: bool | None,
        exclude_min: bool,
        exclude_max: bool,
    ) -> None:
        for name, value in (("min_value", min_value), ("max_value", max_value)):
            if (
                value is not None
                and not math.isnan(value)
                and abs(value) > _F16_MAX
                and not math.isinf(value)
            ):
                raise InvalidArgument(f"{name}={value!r} is not representable as a 16-bit float")
        self.min_value = min_value
        self.max_value = max_value
        self.exclude_min = exclude_min
        self.exclude_max = exclude_max
        self.allow_subnormal = allow_subnormal
        lo = -_F16_MAX if min_value is None else min_value
        hi = _F16_MAX if max_value is None else max_value
        inner_min = None if min_value is None else max(min_value, -_F16_MAX)
        inner_max = None if max_value is None else min(max_value, _F16_MAX)
        if min_value is None and max_value is None:
            self.inner = gs.floats(
                -_F16_MAX,
                _F16_MAX,
                allow_nan=False,
                allow_infinity=False,
                allow_subnormal=allow_subnormal,
                width=32,
            )
        else:
            self.inner = gs.floats(
                inner_min,
                inner_max,
                allow_nan=False,
                allow_infinity=False,
                allow_subnormal=allow_subnormal,
                width=32,
                exclude_min=exclude_min,
                exclude_max=exclude_max,
            )
        unbounded = min_value is None or max_value is None
        self.allow_nan = unbounded if allow_nan is None else allow_nan
        if self.allow_nan and not unbounded:
            raise InvalidArgument(
                f"Cannot have allow_nan={allow_nan!r}, with min_value or max_value"
            )
        self.allow_infinity = unbounded if allow_infinity is None else allow_infinity
        if self.allow_infinity and not unbounded:
            raise InvalidArgument(
                "Cannot have allow_infinity=True, with both min_value and max_value"
            )
        self.lo, self.hi = lo, hi
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
        for _ in range(3):
            value = _to_f16(tc.draw_silent(self.inner))
            if self.allow_subnormal is False and 0 < abs(value) < 2.0**-14:
                continue
            if value < self.lo or value > self.hi:
                continue
            if self.exclude_min and value == self.lo:
                continue
            if self.exclude_max and value == self.hi:
                continue
            return value
        tc.reject()
        raise AssertionError("unreachable")  # nocov

    def __repr__(self) -> str:
        return f"floats(min_value={self.min_value!r}, max_value={self.max_value!r}, width=16)"


@defines_strategy
def sampled_from(elements: Any) -> SearchStrategy[Any]:
    """One of the given values (a sequence, or an Enum class)."""
    if isinstance(elements, (set, frozenset)):
        raise InvalidArgument(
            "Cannot sample from a set, since sets have no defined order. "
            f"Use a sequence (e.g. list or tuple) instead: {elements!r}"
        )
    if isinstance(elements, dict):
        raise InvalidArgument("Cannot sample from a dict; use sampled_from(list(d)) or similar")
    if isinstance(elements, type) and issubclass(elements, enum_module.Enum):
        return gs.sampled_from(elements)
    if isinstance(elements, str):
        elements = list(elements)
    if not isinstance(elements, Sequence):
        try:
            elements = list(elements)
        except TypeError:
            raise InvalidArgument(f"Cannot sample from {elements!r}, not an iterable") from None
    if not elements:
        return gs.nothing()
    return gs.sampled_from(elements)


import enum as enum_module  # noqa: E402


def one_of(*args: Any) -> SearchStrategy[Any]:
    """A value from any of the strategies (one iterable of strategies is
    also accepted)."""
    if len(args) == 1 and not isinstance(args[0], Generator):
        try:
            args = tuple(args[0])
        except TypeError:
            check_strategy(args[0])
    for arg in args:
        if not isinstance(arg, Generator):
            raise InvalidArgument(
                f"Expected a SearchStrategy but got {arg!r} (type={type(arg).__name__}). "
                f"Did you mean st.sampled_from({list(args)!r})?"
            )
    if not args:
        return NOTHING
    if len(args) == 1:
        return args[0]
    live = [arg for arg in args if not isinstance(arg, Nothing)]
    if not live:
        return NOTHING
    if len(live) == 1:
        return live[0]
    return OneOf(live)


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
    if _check_nonempty_elements("lists", elements, min_size):
        return just([])
    return gs.lists(
        elements, min_size=min_size, max_size=max_size, unique=unique, unique_by=unique_by
    )


def _check_nonempty_elements(name: str, elements: Any, min_size: int) -> bool:
    """Whether ``elements`` is empty; an error when the collection must
    then be non-empty."""
    if not getattr(elements, "is_empty", False):
        return False
    if min_size > 0:
        raise InvalidArgument(
            f"Cannot create non-empty {name} with elements drawn from the strategy "
            f"{elements!r} because it has no values."
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
    if _check_nonempty_elements("sets", elements, int(min_size)):
        return just(set())
    return gs.sets(
        elements, min_size=int(min_size), max_size=None if max_size is None else int(max_size)
    )


@defines_strategy
def frozensets(
    elements: SearchStrategy[Ex], *, min_size: int = 0, max_size: int | None = None
) -> SearchStrategy[frozenset[Ex]]:
    check_strategy(elements, "elements")
    check_valid_sizes(min_size, max_size)
    if max_size == 0:
        return just(frozenset())
    if _check_nonempty_elements("frozensets", elements, int(min_size)):
        return just(frozenset())
    return gs.frozensets(
        elements, min_size=int(min_size), max_size=None if max_size is None else int(max_size)
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
    return gs.fixed_dicts(mapping, optional=optional)


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
    if _check_nonempty_elements("dictionaries", keys, int(min_size)) or _check_nonempty_elements(
        "dictionaries", values, int(min_size)
    ):
        return just(dict_class())
    strategy = gs.dicts(
        keys, values, min_size=int(min_size), max_size=None if max_size is None else int(max_size)
    )
    if dict_class is dict:
        return strategy
    return strategy.map(dict_class)


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
    if include_characters is not None and exclude_characters is not None:
        overlap = set(include_characters) & set(exclude_characters)
        if overlap:
            raise InvalidArgument(
                f"Characters {sorted(overlap)!r} are present in both "
                "include_characters and exclude_characters"
            )
    return gs.characters(
        codec=codec,
        min_codepoint=min_codepoint,
        max_codepoint=max_codepoint,
        categories=None if categories is None else list(categories),
        exclude_categories=None if exclude_categories is None else list(exclude_categories),
        include_characters=include_characters,
        exclude_characters=exclude_characters,
    )


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
        return gs.text(min_size=min_size, max_size=max_size, codec="utf-8")
    if isinstance(alphabet, Generator):
        return gs.text(alphabet, min_size=min_size, max_size=max_size)
    if isinstance(alphabet, str):
        chars = list(alphabet)
    else:
        try:
            chars = list(alphabet)
        except TypeError:
            raise InvalidArgument(
                f"alphabet={alphabet!r} must be a strategy or a collection of characters"
            ) from None
    for c in chars:
        if not isinstance(c, str) or len(c) != 1:
            bad = [c for c in chars if not isinstance(c, str) or len(c) != 1]
            raise InvalidArgument(
                "The following elements in alphabet are not of length one, which leads to "
                f"violation of size constraints: {bad!r}"
            )
    if not chars:
        return just("") if min_size == 0 else gs.nothing()
    return gs.text("".join(dict.fromkeys(chars)), min_size=min_size, max_size=max_size)


@defines_strategy
def from_regex(
    regex: Any, *, fullmatch: bool = False, alphabet: Any = None
) -> SearchStrategy[str | bytes]:
    check_type(bool, fullmatch, "fullmatch")
    if alphabet is not None and not isinstance(alphabet, Generator):
        alphabet = characters(whitelist_characters=list(alphabet), categories=())
    return gs.from_regex(regex, fullmatch=fullmatch, alphabet=alphabet)


@defines_strategy
def emails(*, domains: SearchStrategy[str] | None = None) -> SearchStrategy[str]:
    if domains is None:
        return gs.emails()
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


def _to_decimal(value: Any, name: str) -> decimal.Decimal | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise InvalidArgument(f"{name}={value!r} must be a number, not bool")
    try:
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
    decimal places."""
    if places is not None:
        if isinstance(places, bool) or not isinstance(places, int) or places < 0:
            raise InvalidArgument(f"places={places!r} must be a non-negative integer")
    lo = _to_decimal(min_value, "min_value")
    hi = _to_decimal(max_value, "max_value")
    if lo is not None and lo.is_nan():
        raise InvalidArgument(f"min_value={min_value!r} may not be NaN")
    if hi is not None and hi.is_nan():
        raise InvalidArgument(f"max_value={max_value!r} may not be NaN")
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
        raise InvalidArgument("Cannot allow_infinity=True with both min_value and max_value")

    if places is not None:
        factor = decimal.Decimal(10) ** places
        int_lo = None if lo is None else math.ceil(lo * factor)
        int_hi = None if hi is None else math.floor(hi * factor)
        if int_lo is not None and int_hi is not None and int_lo > int_hi:
            raise InvalidArgument(
                f"There are no decimals with {places} places between min_value={min_value!r} "
                f"and max_value={max_value!r}"
            )
        finite: SearchStrategy[decimal.Decimal] = integers(int_lo, int_hi).map(
            lambda n: decimal.Decimal(n).scaleb(-places)
        )
    else:

        def fraction_to_decimal(f: _fractions.Fraction) -> decimal.Decimal:
            with decimal.localcontext(decimal.Context(prec=max(28, 60))):
                return decimal.Decimal(f.numerator) / f.denominator

        finite = fractions(lo, hi).map(fraction_to_decimal)
        if lo is not None or hi is not None:
            finite = finite.filter(lambda d: (lo is None or d >= lo) and (hi is None or d <= hi))
    options = [finite]
    specials: list[decimal.Decimal] = []
    if allow_nan:
        specials += [
            decimal.Decimal("NaN"),
            decimal.Decimal("-NaN"),
            decimal.Decimal("sNaN"),
            decimal.Decimal("-sNaN"),
        ]
    if allow_infinity:
        if hi is None:
            specials.append(decimal.Decimal("Infinity"))
        if lo is None:
            specials.append(decimal.Decimal("-Infinity"))
    if specials:
        options.append(gs.sampled_from(specials))
    return gs.one_of(*options) if len(options) > 1 else finite


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
            return gs.nothing()
        return integers(num_lo, num_hi).map(lambda n, d=denominator: _fractions.Fraction(n, d))

    if lo is not None and hi is not None and lo == hi:
        return just(lo)
    return denominators.flat_map(build)


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
    optionally bounded in magnitude."""
    if width not in (32, 64, 128):
        raise InvalidArgument(
            f"Got width={width!r}, but the only valid values are the integers 32, 64, and 128."
        )
    if (
        isinstance(min_magnitude, bool)
        or not isinstance(min_magnitude, (int, float))
        or min_magnitude < 0
    ):
        raise InvalidArgument(f"min_magnitude={min_magnitude!r} must be a non-negative number")
    if max_magnitude is not None:
        if (
            isinstance(max_magnitude, bool)
            or not isinstance(max_magnitude, (int, float))
            or max_magnitude < 0
        ):
            raise InvalidArgument(
                f"max_magnitude={max_magnitude!r} must be a non-negative number or None"
            )
        if max_magnitude < min_magnitude:
            raise InvalidArgument(
                f"max_magnitude={max_magnitude!r} < min_magnitude={min_magnitude!r}"
            )
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
    component_width = width // 2
    unbounded = floats(
        allow_nan=allow_nan,
        allow_infinity=allow_infinity,
        allow_subnormal=allow_subnormal,
        width=component_width,
    )
    if min_magnitude == 0 and max_magnitude is None:
        return tuples(unbounded, unbounded).map(lambda p: complex(p[0], p[1]))
    parts = floats(
        -math.inf if max_magnitude is None else -max_magnitude,
        math.inf if max_magnitude is None else max_magnitude,
        allow_nan=False,
        allow_infinity=allow_infinity if max_magnitude is None else False,
        allow_subnormal=allow_subnormal,
        width=component_width,
    )

    def in_range(z: complex) -> bool:
        magnitude = abs(z)
        if max_magnitude is not None and magnitude > max_magnitude:
            return False
        return magnitude >= min_magnitude

    return tuples(parts, parts).map(lambda p: complex(p[0], p[1])).filter(in_range)


# -- time -------------------------------------------------------------------------


@defines_strategy
def times(
    min_value: dt.time = dt.time.min, max_value: dt.time = dt.time.max, *, timezones: Any = None
) -> SearchStrategy[dt.time]:
    check_type(dt.time, min_value, "min_value")
    check_type(dt.time, max_value, "max_value")
    if timezones is None or isinstance(timezones, Generator) and _is_none_strategy(timezones):
        return gs.times(min_value, max_value)
    check_strategy(timezones, "timezones")
    return gs.times(min_value, max_value, timezones=timezones)


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
    if timezones is None or isinstance(timezones, Generator) and _is_none_strategy(timezones):
        return gs.datetimes(min_value, max_value, allow_imaginary=allow_imaginary)
    check_strategy(timezones, "timezones")
    return gs.datetimes(min_value, max_value, timezones=timezones, allow_imaginary=allow_imaginary)


def _is_none_strategy(strategy: Generator[Any]) -> bool:
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
    base = gs.sampled_from(keys)
    if not allow_prefix:
        return base
    return tuples(gs.sampled_from(["", "posix/", "right/"]), base).map(lambda p: p[0] + p[1])


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
    return gs.recursive(base, extend, max_leaves=max_leaves)


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
        name = None if label is None else str(label)
        return self._tc.draw(strategy, name=name)

    @property
    def conjecture_data(self) -> TestCase:
        return self._tc


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


@defines_strategy
def data() -> SearchStrategy[DataObject]:
    return _Data()


class DrawFn(Protocol):
    def __call__(self, strategy: SearchStrategy[Ex], label: Any = None) -> Ex: ...


def _draw_fn(tc: TestCase) -> DrawFn:
    def draw(strategy: SearchStrategy[Ex], label: Any = None) -> Ex:
        check_strategy(strategy, "strategy")
        return tc.draw_silent(strategy)

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
    if not params or params[0].kind not in (
        params[0].POSITIONAL_ONLY,
        params[0].POSITIONAL_OR_KEYWORD,
    ):
        raise InvalidArgument(
            "Functions wrapped with composite must take at least one "
            f"positional argument. {f!r} does not"
        )
    if params[0].default is not params[0].empty:
        raise InvalidArgument("A default value for initial argument will never be used")

    @functools.wraps(f)
    def tc_function(tc: TestCase, *args: Any, **kwargs: Any) -> Ex:
        return f(_draw_fn(tc), *args, **kwargs)

    tc_function.__signature__ = signature  # type: ignore[attr-defined]
    build = gs.composite(tc_function)

    @functools.wraps(f)
    def accept(*args: Any, **kwargs: Any) -> SearchStrategy[Ex]:
        return LazyStrategy(build, args, kwargs, name=getattr(f, "__name__", "composite"))

    accept.__signature__ = signature.replace(parameters=params[1:])  # type: ignore[attr-defined]
    return accept


@defines_strategy
def shared(base: SearchStrategy[Ex], *, key: Hashable = None) -> SearchStrategy[Ex]:
    """One value per test case, drawn the first time it is needed and
    reused by every ``shared`` strategy with the same key (or the same
    object when no key is given)."""
    check_strategy(base, "base")
    return _Shared(base, key)


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
            store[key] = tc.draw_silent(self.base)
        return store[key]

    def __repr__(self) -> str:
        return f"shared({self.base!r}, key={self.key!r})"


def _per_case_store(tc: TestCase) -> dict[Any, Any]:
    if currently_in_test_context():
        return current_build_context().shared
    store = getattr(tc._global, "hypothesis_shared", None)
    if store is None:
        store = {}
        tc._global.hypothesis_shared = store  # type: ignore[attr-defined]
    return store


@defines_strategy
def slices(size: int) -> SearchStrategy[slice]:
    """Slices usable on a sequence of ``size`` elements."""
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        raise InvalidArgument(f"size={size!r} must be a non-negative integer")
    if size == 0:
        step = integers().filter(lambda n: n != 0)
        return gs.composite(lambda tc: slice(None, None, tc.draw_silent(step)))()

    def build(tc: TestCase) -> slice:
        min_start = tc.draw_silent(integers(0, size))
        max_start = tc.draw_silent(integers(min_start, size))
        start = tc.draw_silent(integers(min_start, max_start))
        stop = tc.draw_silent(integers(0, size))
        step = tc.draw_silent(integers(1, size))
        if tc.draw_silent(booleans()):
            step = -step
            start, stop = stop, start
        if tc.draw_silent(booleans()) and start == 0:
            start_opt: int | None = None
        else:
            start_opt = start
        if tc.draw_silent(booleans()) and (stop == size if step > 0 else stop == 0):
            stop_opt: int | None = None
        else:
            stop_opt = stop
        step_opt = None if step == 1 and tc.draw_silent(booleans()) else step
        return slice(start_opt, stop_opt, step_opt)

    return gs.composite(build)()


@defines_strategy
def randoms(
    *, note_method_calls: bool = False, use_true_random: bool = False
) -> SearchStrategy[_random.Random]:
    """``random.Random`` instances with a generated seed."""
    check_type(bool, note_method_calls, "note_method_calls")
    check_type(bool, use_true_random, "use_true_random")
    if use_true_random:
        return gs.composite(lambda tc: _random.Random())()
    return integers(0, 2**63 - 1).map(_HypothesisRandom).print_as_call("HypothesisRandom")


class _HypothesisRandom(_random.Random):
    def __init__(self, seed: int = 0) -> None:
        super().__init__(seed)
        self._hegel_seed = seed

    def __repr__(self) -> str:
        return f"HypothesisRandom({self._hegel_seed})"


class _RandomSeeder:
    def __init__(self, seed: int) -> None:
        self.seed = seed

    def __repr__(self) -> str:
        return f"RandomSeeder({self.seed!r})"


@defines_strategy
def random_module() -> SearchStrategy[_RandomSeeder]:
    """Seed the global ``random`` module for the test case, restoring its
    state afterwards."""

    def seed_random(tc: TestCase) -> _RandomSeeder:
        seed = tc.draw_silent(integers(0, 2**32 - 1))
        state = _random.getstate()
        _random.seed(seed)
        if currently_in_test_context():
            from hegel.hypothesis.control import cleanup

            cleanup(lambda: _random.setstate(state))
        return _RandomSeeder(seed)

    return gs.composite(seed_random)()


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

    return gs.composite(get_runner)()


@defines_strategy
def functions(
    *,
    like: Callable[..., Any] = lambda: None,
    returns: SearchStrategy[Any] | None = None,
    pure: bool = False,
) -> SearchStrategy[Callable[..., Any]]:
    """Functions with the same signature as ``like``, returning values
    drawn from ``returns``."""
    if not callable(like):
        raise InvalidArgument(
            "The first argument to functions() must be a callable to imitate, but "
            f"got non-callable like={like!r}"
        )
    if returns is None:
        from hegel.hypothesis.strategies import from_type as _from_type

        hints = {}
        try:
            import typing

            hints = typing.get_type_hints(like)
        except Exception:  # noqa: BLE001
            hints = {}
        returns = _from_type(hints["return"]) if "return" in hints else none()
    check_strategy(returns, "returns")
    check_type(bool, pure, "pure")
    return _Functions(like, returns, pure)


class _Functions(Generator[Callable[..., Any]]):
    def __init__(self, like: Callable[..., Any], returns: Generator[Any], pure: bool) -> None:
        self.like = like
        self.returns = returns
        self.pure = pure
        self.label = returns.label

    def do_draw(self, tc: TestCase) -> Callable[..., Any]:
        like, returns, pure = self.like, self.returns, self.pure
        cache: dict[Any, Any] = {}
        signature = inspect.signature(like)

        @functools.wraps(like)
        def generated(*args: Any, **kwargs: Any) -> Any:
            bound = signature.bind(*args, **kwargs)
            bound.apply_defaults()
            if pure:
                key = (tuple(bound.args), tuple(sorted(bound.kwargs.items())))
                try:
                    if key in cache:
                        return cache[key]
                except TypeError:
                    key = None
                value = tc.draw_silent(returns)
                if key is not None:
                    cache[key] = value
                return value
            return tc.draw_silent(returns)

        generated.__hegel_signature__ = signature  # type: ignore[attr-defined]
        return generated

    def __repr__(self) -> str:
        return f"functions(like={self.like!r}, returns={self.returns!r}, pure={self.pure!r})"


def register_type_strategy(
    custom_type: Any, strategy: SearchStrategy[Any] | Callable[[Any], SearchStrategy[Any]]
) -> None:
    """Make ``from_type(custom_type)`` use ``strategy`` (or the strategy
    the callable returns for the type)."""
    if not isinstance(strategy, Generator) and not callable(strategy):
        raise InvalidArgument(
            f"strategy={strategy!r} must be a SearchStrategy, or a function that "
            "takes a type and returns one"
        )
    if isinstance(strategy, Generator) and strategy.is_empty:
        raise InvalidArgument(f"strategy={strategy!r} cannot be an empty strategy")
    gs.register_type_generator(custom_type, strategy)


@defines_strategy
def from_type(thing: Any) -> SearchStrategy[Any]:
    """A strategy for instances of ``thing``, from its annotations or the
    registered strategy for it."""
    return gs.from_type(thing)
