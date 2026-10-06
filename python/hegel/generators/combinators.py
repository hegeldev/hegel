"""Generators built from other generators or fixed values."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import TYPE_CHECKING, Any, TypeVar

from hegel.errors import InvalidArgument
from hegel.generators.base import (
    FILTER_LABEL,
    NOOP_PRINTER,
    Generator,
    combine_labels,
    label_from_name,
)
from hegel.pretty import PrettyPrinter, pretty_print

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["as_generator", "just", "none", "nothing", "one_of", "optional", "sampled_from"]

T = TypeVar("T")

JUST_LABEL = label_from_name("hegel.just")
NOTHING_LABEL = label_from_name("hegel.nothing")
SAMPLED_FROM_LABEL = label_from_name("hegel.sampled_from")
ONE_OF_LABEL = label_from_name("hegel.one_of")
OPTIONAL_LABEL = label_from_name("hegel.optional")


def as_generator(value: Any) -> Generator[Any]:
    """Coerce the result of a user callback into a generator."""
    if isinstance(value, Generator):
        return value
    raise InvalidArgument(f"expected a Generator but got {value!r} of type {type(value).__name__}")


class Just(Generator[T]):
    def __init__(self, value: T) -> None:
        self.value = value
        self.label = JUST_LABEL

    def do_draw(self, tc: TestCase) -> T:
        return self.value

    def __repr__(self) -> str:
        return f"just({self.value!r})"


def just(value: T) -> Generator[T]:
    """Always generate ``value``."""
    return Just(value)


def none() -> Generator[None]:
    """Always generate ``None``."""
    return Just(None)


class Nothing(Generator[Any]):
    label = NOTHING_LABEL

    def do_draw(self, tc: TestCase) -> Any:
        tc.reject()
        raise AssertionError("unreachable")  # nocov

    @property
    def is_empty(self) -> bool:
        return True

    def __repr__(self) -> str:
        return "nothing()"


def nothing() -> Generator[Any]:
    """A generator with no values: every draw rejects the test case."""
    return Nothing()


class SampledFrom(Generator[T]):
    def __init__(self, elements: Sequence[T]) -> None:
        self.elements = elements
        self.label = SAMPLED_FROM_LABEL

    def do_draw(self, tc: TestCase) -> T:
        index = tc.generate_integer(0, len(self.elements) - 1)
        return self.elements[index]

    def filter(self, predicate: Callable[[T], bool]) -> Generator[T]:
        """Filtering a sample narrows the elements rather than retrying
        draws, so even a predicate that accepts one element in thousands
        finds it."""
        return FilteredSample(self, [predicate])

    @property
    def is_empty(self) -> bool:
        return not self.elements

    def __repr__(self) -> str:
        return f"sampled_from({list(self.elements)!r})"


FILTERED_SAMPLE_ATTEMPTS = 3


class FilteredSample(Generator[T]):
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
            index = remaining.pop(tc.generate_integer(0, len(remaining) - 1))
            element = elements[index]
            if self._accepts(element):
                return element
        tc.reject()
        raise AssertionError("unreachable")  # nocov

    def __repr__(self) -> str:
        names = (getattr(p, "__name__", repr(p)) for p in self.predicates)
        return repr(self.source) + "".join(f".filter({name})" for name in names)


def sampled_from(elements: Any) -> Generator[Any]:
    """Pick one of ``elements`` (a sequence, or an ``Enum`` class)."""
    import enum

    if isinstance(elements, type) and issubclass(elements, enum.Enum):
        if issubclass(elements, enum.Flag):
            members = list(elements)
            if members:
                return Lists_of_flags(elements, members)
        elements = list(elements)
    elif isinstance(elements, (set, frozenset)):
        raise InvalidArgument(
            "Cannot sample from a set: it has no stable order. Pass a sorted list instead."
        )
    else:
        elements = list(elements)
    if not elements:
        raise InvalidArgument("Cannot sample from an empty sequence")
    return SampledFrom(elements)


class Lists_of_flags(Generator[Any]):
    """Flag enums sample any combination of members."""

    def __init__(self, cls: type, members: list[Any]) -> None:
        self.cls = cls
        self.members = members
        self.label = combine_labels([SAMPLED_FROM_LABEL, label_from_name("hegel.flags")])

    def do_draw(self, tc: TestCase) -> Any:
        # One bit per member, so every non-empty combination is reachable
        # and the smallest value is the first member alone.
        mask = tc.generate_integer(1, (1 << len(self.members)) - 1)
        result = self.cls(0)
        for index, member in enumerate(self.members):
            if mask >> index & 1:
                result |= member
        return result


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
        index = tc.generate_integer(0, len(self.generators) - 1)
        if printer is None:
            return tc.draw_silent(self.generators[index])
        return tc.draw_and_print(self.generators[index], printer)

    def do_draw(self, tc: TestCase) -> Any:
        return self._draw(tc, None)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> Any:
        return self._draw(tc, printer)

    @property
    def is_empty(self) -> bool:
        return all(g.is_empty for g in self.generators)

    def __repr__(self) -> str:
        return "one_of(" + ", ".join(repr(g) for g in self.generators) + ")"


def one_of(*generators: Any) -> Generator[Any]:
    """Draw from one of ``generators``, chosen uniformly. Accepts the
    generators as separate arguments or as one iterable."""
    if len(generators) == 1 and not isinstance(generators[0], Generator):
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
        if tc.generate_boolean(0.5):
            return tc.draw_and_print(self.inner, printer)
        pretty_print(None, printer)
        return None

    def do_draw(self, tc: TestCase) -> T | None:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T | None:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return f"optional({self.inner!r})"


def optional(inner: Generator[T]) -> Generator[T | None]:
    """Generate either ``None`` or a value from ``inner``."""
    return Optional(as_generator(inner))
