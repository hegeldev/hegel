"""Generators built from other generators or fixed values."""

from __future__ import annotations

import enum
from collections.abc import Callable, Iterable, MutableSequence, Sequence
from typing import TYPE_CHECKING, Any, TypeVar

from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import FILTER_LABEL, Generator, _name, _PrintsValue
from hegel.pretty import _SILENT_PRINTER, PrettyPrinter

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["just", "none", "nothing", "one_of", "optional", "sampled_from"]

T = TypeVar("T")

JUST_LABEL = label_from_name("hegel.just")
NOTHING_LABEL = label_from_name("hegel.nothing")
SAMPLED_FROM_LABEL = label_from_name("hegel.sampled_from")
ONE_OF_LABEL = label_from_name("hegel.one_of")
OPTIONAL_LABEL = label_from_name("hegel.optional")


def as_generator(value: Any) -> Generator[Any]:
    """Check that ``value`` (an argument, or what a user callback returned)
    is a generator, and return it."""
    if isinstance(value, Generator):
        return value
    raise InvalidArgument(f"expected a Generator but got {value!r} of type {type(value).__name__}")


class Just(_PrintsValue[T]):
    def __init__(self, value: T) -> None:
        self.value = value
        self.label = JUST_LABEL

    def do_draw(self, tc: TestCase) -> T:
        return self.value

    def __repr__(self) -> str:
        return f"just({self.value!r})"


def just(value: T) -> Generator[T]:
    """Always generate ``value``.

    The same object is returned by every draw, so mutating it in one test
    case is visible in the next. It is useful as an argument to
    :func:`one_of` or :func:`~hegel.generators.builds`, or as the result
    of a :meth:`~hegel.generators.Generator.flat_map`.
    """
    return Just(value)


class NoneGenerator(Just[None]):
    def __init__(self) -> None:
        super().__init__(None)

    def __repr__(self) -> str:
        return "none()"


def none() -> Generator[None]:
    """Always generate ``None``. The same as ``just(None)``."""
    return NoneGenerator()


class Nothing(_PrintsValue[Any]):
    label = NOTHING_LABEL

    def do_draw(self, tc: TestCase) -> Any:
        tc.reject()
        raise AssertionError("unreachable")  # nocov

    def __repr__(self) -> str:
        return "nothing()"


def nothing() -> Generator[Any]:
    """A generator with no values: every draw rejects the test case, as
    :meth:`TestCase.reject <hegel.TestCase.reject>` does.

    :func:`one_of` drops it, so ``one_of(gs.nothing(), g)`` is ``g``.
    """
    return Nothing()


class SampledFrom(_PrintsValue[T]):
    def __init__(self, elements: Sequence[T], repr_: str | None = None) -> None:
        self.elements = elements
        self.label = SAMPLED_FROM_LABEL
        self._repr = repr_

    def do_draw(self, tc: TestCase) -> T:
        index = tc._generate_integer(0, len(self.elements) - 1)
        return self.elements[index]

    def filter(self, predicate: Callable[[T], bool]) -> Generator[T]:
        """Filtering a sample narrows the elements rather than retrying
        draws, so even a predicate that accepts one element in thousands
        finds it."""
        return FilteredSample(self, [predicate])

    def __repr__(self) -> str:
        if self._repr is not None:
            return self._repr
        return f"sampled_from({self.elements!r})"


FILTERED_SAMPLE_ATTEMPTS = 3


class FilteredSample(_PrintsValue[T]):
    """``sampled_from(...).filter(...)``: a few random draws, then the
    untried elements in an engine-chosen order until one is accepted, so
    a predicate accepting one element in thousands still finds it."""

    def __init__(self, source: SampledFrom[T], predicates: list[Callable[[T], bool]]) -> None:
        self.source = source
        self.predicates = predicates
        self.label = combine_labels([FILTER_LABEL, source.label])

    def filter(self, predicate: Callable[[T], bool]) -> Generator[T]:
        return FilteredSample(self.source, [*self.predicates, predicate])

    def _accepts(self, element: T) -> bool:
        return all(p(element) for p in self.predicates)

    def do_draw(self, tc: TestCase) -> T:
        elements = self.source.elements
        for _ in range(FILTERED_SAMPLE_ATTEMPTS):
            element = tc.draw_silent(self.source)
            if self._accepts(element):
                return element
        remaining = list(range(len(elements)))
        while remaining:
            index = remaining.pop(tc._generate_integer(0, len(remaining) - 1))
            element = elements[index]
            if self._accepts(element):
                return element
        tc.reject()
        raise AssertionError("unreachable")  # nocov

    def __repr__(self) -> str:
        names = (getattr(p, "__name__", repr(p)) for p in self.predicates)
        return repr(self.source) + "".join(f".filter({name})" for name in names)


def sampled_from(elements: Any) -> Generator[Any]:
    """Pick one of ``elements``, shrinking toward the first.

    ``elements`` is a sequence (copied if mutable, so later changes to it
    do not affect the generator), any other iterable (read once), or an
    :class:`~enum.Enum` class, whose members are sampled in definition
    order. A :class:`~enum.Flag` class also yields the empty flag and
    combinations of its members. An empty collection, an enum with no
    members, or a ``set`` (which has no stable order to shrink in) is an
    :class:`~hegel.InvalidArgument`.

    ::

        colour = tc.draw(gs.sampled_from(["red", "green", "blue"]))

    ``.filter`` on the result narrows the elements rather than retrying
    draws, so even a predicate that accepts one element in thousands finds
    it.
    """
    if isinstance(elements, type) and issubclass(elements, enum.Enum):
        if not elements.__members__:
            raise InvalidArgument(f"Cannot sample from {_name(elements)}: it has no members")
        if issubclass(elements, enum.Flag):
            return FlagCombinations(elements)
        return SampledFrom(tuple(elements), f"sampled_from({_name(elements)})")
    if isinstance(elements, (set, frozenset)):
        raise InvalidArgument(
            "Cannot sample from a set: it has no stable order. Pass a sorted list instead."
        )
    if isinstance(elements, MutableSequence):
        # A copy, so that later changes to the argument leave the generator alone.
        elements = list(elements)
    elif not isinstance(elements, Sequence):
        elements = tuple(elements)
    if not elements:
        raise InvalidArgument("Cannot sample from an empty sequence")
    return SampledFrom(elements)


class FlagCombinations(_PrintsValue[Any]):
    """``sampled_from`` a flag enum: one of its named members, the empty
    flag, or any combination of the named members."""

    def __init__(self, cls: type[enum.Flag]) -> None:
        self.cls = cls
        # Every named member, aliases and the zero member included (iterating
        # the class skips those), fewest bits first so that the smallest
        # value is the first single-bit member, or the empty flag if named.
        named = sorted(set(cls.__members__.values()), key=lambda m: (m.value.bit_count(), m.value))
        self.values = named if named[0].value == 0 else [*named, cls(0)]
        self.flags = [m for m in named if m.value]
        self.label = combine_labels([SAMPLED_FROM_LABEL, label_from_name("hegel.flags")])

    def do_draw(self, tc: TestCase) -> Any:
        if len(self.flags) > 1 and tc._generate_boolean(0.5):
            # One bit per named flag, so every combination is reachable.
            mask = tc._generate_integer(1, (1 << len(self.flags)) - 1)
            result = self.cls(0)
            for index, flag in enumerate(self.flags):
                if mask >> index & 1:
                    result |= flag
            return result
        return self.values[tc._generate_integer(0, len(self.values) - 1)]

    def __repr__(self) -> str:
        return f"sampled_from({_name(self.cls)})"


class OneOf(Generator[Any]):
    def __init__(self, generators: list[Generator[Any]]) -> None:
        self.generators = generators
        self._label: int | None = None

    @property
    def label(self) -> int:  # type: ignore[override]
        # Computed on first use so that ``a | b`` never forces lazily
        # validated operands at definition time.
        if self._label is None:
            self._label = combine_labels(
                [ONE_OF_LABEL, combine_labels(g.label for g in self.generators)]
            )
        return self._label

    def _draw(self, tc: TestCase, printer: PrettyPrinter | None) -> Any:
        index = tc._generate_integer(0, len(self.generators) - 1)
        if printer is None:
            return tc.draw_silent(self.generators[index])
        return tc.draw_and_print(self.generators[index], printer)

    def do_draw(self, tc: TestCase) -> Any:
        return self._draw(tc, None)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> Any:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return "one_of(" + ", ".join(repr(g) for g in self.generators) + ")"


def one_of(*generators: Any) -> Generator[Any]:
    """Draw from one of ``generators``, shrinking toward the earlier ones.

    The generators are passed as separate arguments or as one iterable;
    ``a | b`` is ``one_of(a, b)``. Nested ``one_of`` calls are flattened,
    :func:`nothing` arguments are dropped, and a single generator is
    returned as it is. ``one_of()`` with no arguments, or an argument that
    is not a generator, is an :class:`~hegel.InvalidArgument`.

    ::

        key = tc.draw(gs.one_of(gs.integers(), gs.text()))
        key = tc.draw(gs.integers() | gs.text())
    """
    if (
        len(generators) == 1
        and not isinstance(generators[0], Generator)
        and isinstance(generators[0], Iterable)
    ):
        generators = tuple(generators[0])
    flat: list[Generator[Any]] = []
    for generator in generators:
        if isinstance(generator, Nothing):
            continue
        if isinstance(generator, OneOf):
            flat.extend(generator.generators)
        else:
            flat.append(as_generator(generator))
    if not flat:
        if generators:
            return Nothing()
        raise InvalidArgument("one_of requires at least one generator")
    if len(flat) == 1:
        return flat[0]
    return OneOf(flat)


class Optional(Generator[Any]):
    def __init__(self, inner: Generator[T]) -> None:
        self.inner = inner
        self.label = combine_labels([OPTIONAL_LABEL, inner.label])

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> Any:
        if tc._generate_boolean(0.5):
            return tc.draw_and_print(self.inner, printer)
        printer.text("None")
        return None

    def do_draw(self, tc: TestCase) -> T | None:
        return self._draw(tc, _SILENT_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T | None:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return f"optional({self.inner!r})"


def optional(inner: Generator[T]) -> Generator[T | None]:
    """Generate either ``None`` or a value from ``inner``, shrinking toward
    ``None``. Passing something other than a generator is an
    :class:`~hegel.InvalidArgument`."""
    return Optional(as_generator(inner))
