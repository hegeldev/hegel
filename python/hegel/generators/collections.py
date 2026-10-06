"""Collection generators: lists, sets, dicts, tuples, and samples."""

from __future__ import annotations

from collections.abc import Callable, Hashable, Iterable, Sequence
from typing import TYPE_CHECKING, Any, TypeVar

from hegel.errors import InvalidArgument
from hegel.generators.base import NOOP_PRINTER, Generator, combine_labels, label_from_name
from hegel.generators.combinators import as_generator
from hegel.pretty import PrettyPrinter
from hegel.test_case import Collection

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "dicts",
    "fixed_dicts",
    "frozensets",
    "iterables",
    "lists",
    "permutations",
    "samples",
    "sets",
    "subsequences",
    "tuples",
]

T = TypeVar("T")

LIST_LABEL = label_from_name("hegel.lists")
SET_LABEL = label_from_name("hegel.sets")
FROZENSET_LABEL = label_from_name("hegel.frozensets")
DICT_LABEL = label_from_name("hegel.dicts")
FIXED_DICT_LABEL = label_from_name("hegel.fixed_dicts")
TUPLE_LABEL = label_from_name("hegel.tuples")
SUBSEQUENCE_LABEL = label_from_name("hegel.subsequence")
PERMUTATION_LABEL = label_from_name("hegel.permutation")
SAMPLE_LABEL = label_from_name("hegel.sample")


def _check_sizes(min_size: int, max_size: int | None) -> tuple[int, int | None]:
    if isinstance(min_size, bool) or not isinstance(min_size, int) or min_size < 0:
        raise InvalidArgument(f"min_size={min_size!r} must be a non-negative integer")
    if max_size is not None:
        if isinstance(max_size, bool) or not isinstance(max_size, int) or max_size < 0:
            raise InvalidArgument(f"max_size={max_size!r} must be a non-negative integer")
        if max_size < min_size:
            raise InvalidArgument(f"Cannot have max_size={max_size!r} < min_size={min_size!r}")
    return min_size, max_size


def _as_key_tuple(unique_by: Any) -> tuple[Callable[[Any], Hashable], ...]:
    if callable(unique_by):
        return (unique_by,)
    keys = tuple(unique_by)
    if not keys or not all(callable(k) for k in keys):
        raise InvalidArgument("unique_by must be a callable or a non-empty tuple of callables")
    return keys


class Lists(Generator[list[T]]):
    def __init__(
        self,
        elements: Generator[T],
        min_size: int,
        max_size: int | None,
        unique_by: tuple[Callable[[T], Hashable], ...] | None,
    ) -> None:
        self.elements = elements
        self.min_size = min_size
        self.max_size = max_size
        self.unique_by = unique_by
        self.label = combine_labels([LIST_LABEL, elements.label])

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> list[T]:
        printer.begin_group(1, "[")
        currently_empty = getattr(self.elements, "is_currently_empty", None)
        if currently_empty is not None and currently_empty(tc):
            # The element generator has no values right now (a stateful
            # bundle nothing has been added to): only the empty list exists.
            if self.min_size > 0:
                tc.reject()
            printer.end_group("]")
            return []
        collection = Collection(tc, self.min_size, self.max_size)
        result: list[T] = []
        seen: list[set[Hashable]] = [set() for _ in (self.unique_by or ())]
        while collection.more():
            with printer.speculate() as speculation:
                if result:
                    printer.text(",")
                    printer.breakable(" ")
                element = tc.draw_and_print(self.elements, printer)
                if self.unique_by is not None:
                    keys = [key(element) for key in self.unique_by]
                    if any(k in s for k, s in zip(keys, seen, strict=True)):
                        speculation.abort()
                        collection.reject("duplicate element")
                        continue
                    for k, s in zip(keys, seen, strict=True):
                        s.add(k)
                speculation.commit()
                result.append(element)
        printer.end_group("]")
        return result

    def do_draw(self, tc: TestCase) -> list[T]:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> list[T]:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return f"lists({self.elements!r}, min_size={self.min_size}, max_size={self.max_size})"


def lists(
    elements: Generator[T],
    *,
    min_size: int = 0,
    max_size: int | None = None,
    unique: bool = False,
    unique_by: Any = None,
) -> Generator[list[T]]:
    """Generate lists of values from ``elements``.

    ``unique`` requires distinct (hashable) elements; ``unique_by`` is a
    key function, or a tuple of them, each of whose results must be
    distinct across the list.
    """
    elements = as_generator(elements)
    min_size, max_size = _check_sizes(min_size, max_size)
    if unique and unique_by is not None:
        raise InvalidArgument("Cannot pass both unique and unique_by")
    keys: tuple[Callable[[Any], Hashable], ...] | None = None
    if unique:
        keys = (_identity,)
    elif unique_by is not None:
        keys = _as_key_tuple(unique_by)
    return Lists(elements, min_size, max_size, keys)


def _identity(value: Any) -> Any:
    return value


class Sets(Generator[Any]):
    def __init__(
        self, elements: Generator[T], min_size: int, max_size: int | None, frozen: bool
    ) -> None:
        self.elements = elements
        self.min_size = min_size
        self.max_size = max_size
        self.frozen = frozen
        self.label = combine_labels([FROZENSET_LABEL if frozen else SET_LABEL, elements.label])

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> Any:
        open_, close = ("frozenset({", "})") if self.frozen else ("{", "}")
        printer.begin_group(len(open_), open_)
        collection = Collection(tc, self.min_size, self.max_size)
        result: set[Any] = set()
        while collection.more():
            with printer.speculate() as speculation:
                if result:
                    printer.text(",")
                    printer.breakable(" ")
                element = tc.draw_and_print(self.elements, printer)
                if element in result:
                    speculation.abort()
                    collection.reject("duplicate element")
                else:
                    speculation.commit()
                    result.add(element)
        if not result:
            printer.if_break("")
        printer.end_group(close)
        return frozenset(result) if self.frozen else result

    def do_draw(self, tc: TestCase) -> Any:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> Any:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        name = "frozensets" if self.frozen else "sets"
        return f"{name}({self.elements!r}, min_size={self.min_size}, max_size={self.max_size})"


def sets(
    elements: Generator[T], *, min_size: int = 0, max_size: int | None = None
) -> Generator[set[T]]:
    """Generate sets of distinct values from ``elements``."""
    min_size, max_size = _check_sizes(min_size, max_size)
    return Sets(as_generator(elements), min_size, max_size, False)


def frozensets(
    elements: Generator[T], *, min_size: int = 0, max_size: int | None = None
) -> Generator[frozenset[T]]:
    """Generate frozensets of distinct values from ``elements``."""
    min_size, max_size = _check_sizes(min_size, max_size)
    return Sets(as_generator(elements), min_size, max_size, True)


def iterables(
    elements: Generator[T],
    *,
    min_size: int = 0,
    max_size: int | None = None,
    unique: bool = False,
    unique_by: Any = None,
) -> Generator[Iterable[T]]:
    """Generate lists, as ``lists`` does, but reported and handed to the
    test as a one-shot iterable."""
    return lists(
        elements, min_size=min_size, max_size=max_size, unique=unique, unique_by=unique_by
    ).map(_OneShot)


class _OneShot:
    def __init__(self, values: list[Any]) -> None:
        self._values = values

    def __iter__(self) -> Any:
        return iter(self._values)

    def __repr__(self) -> str:
        return f"iter({self._values!r})"


class Dicts(Generator[dict[Any, Any]]):
    def __init__(
        self,
        keys: Generator[Any],
        values: Generator[Any],
        min_size: int,
        max_size: int | None,
    ) -> None:
        self.keys = keys
        self.values = values
        self.min_size = min_size
        self.max_size = max_size
        self.label = combine_labels([DICT_LABEL, keys.label, values.label])

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> dict[Any, Any]:
        printer.begin_group(1, "{")
        collection = Collection(tc, self.min_size, self.max_size)
        result: dict[Any, Any] = {}
        while collection.more():
            with printer.speculate() as speculation:
                if result:
                    printer.text(",")
                    printer.breakable(" ")
                key = tc.draw_and_print(self.keys, printer)
                if key in result:
                    speculation.abort()
                    collection.reject("duplicate key")
                    continue
                printer.text(": ")
                value = tc.draw_and_print(self.values, printer)
                speculation.commit()
                result[key] = value
        printer.end_group("}")
        return result

    def do_draw(self, tc: TestCase) -> dict[Any, Any]:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> dict[Any, Any]:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return (
            f"dicts({self.keys!r}, {self.values!r}, "
            f"min_size={self.min_size}, max_size={self.max_size})"
        )


def dicts(
    keys: Generator[Any],
    values: Generator[Any],
    *,
    min_size: int = 0,
    max_size: int | None = None,
) -> Generator[dict[Any, Any]]:
    """Generate dicts with keys and values from the given generators."""
    min_size, max_size = _check_sizes(min_size, max_size)
    return Dicts(as_generator(keys), as_generator(values), min_size, max_size)


class FixedDicts(Generator[dict[Any, Any]]):
    def __init__(
        self, mapping: dict[Any, Generator[Any]], optional: dict[Any, Generator[Any]]
    ) -> None:
        self.mapping = mapping
        self.optional = optional
        self.label = combine_labels(
            [
                FIXED_DICT_LABEL,
                *(g.label for g in mapping.values()),
                *(g.label for g in optional.values()),
            ]
        )

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> dict[Any, Any]:
        from hegel.pretty import pretty_print

        printer.begin_group(1, "{")
        result: dict[Any, Any] = {}
        for key, generator in self.mapping.items():
            if result:
                printer.text(",")
                printer.breakable(" ")
            pretty_print(key, printer)
            printer.text(": ")
            result[key] = tc.draw_and_print(generator, printer)
        for key, generator in self.optional.items():
            if not tc.generate_boolean(0.5):
                continue
            with printer.speculate() as speculation:
                if result:
                    printer.text(",")
                    printer.breakable(" ")
                pretty_print(key, printer)
                printer.text(": ")
                result[key] = tc.draw_and_print(generator, printer)
                speculation.commit()
        printer.end_group("}")
        return result

    def do_draw(self, tc: TestCase) -> dict[Any, Any]:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> dict[Any, Any]:
        return self._draw(tc, printer)


def fixed_dicts(
    mapping: dict[Any, Generator[Any]], *, optional: dict[Any, Generator[Any]] | None = None
) -> Generator[dict[Any, Any]]:
    """Generate dicts with the fixed keys of ``mapping``, each value drawn
    from its generator; keys in ``optional`` are present or absent."""
    fixed = {k: as_generator(v) for k, v in dict(mapping).items()}
    opt = {k: as_generator(v) for k, v in dict(optional or {}).items()}
    for key in opt:
        if key in fixed:
            raise InvalidArgument(f"key {key!r} is both required and optional")
    return FixedDicts(fixed, opt)


class Tuples(Generator[tuple[Any, ...]]):
    def __init__(self, generators: tuple[Generator[Any], ...]) -> None:
        self.generators = generators
        self.label = combine_labels([TUPLE_LABEL, *(g.label for g in generators)])

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> tuple[Any, ...]:
        printer.begin_group(1, "(")
        result = []
        for index, generator in enumerate(self.generators):
            if index > 0:
                printer.text(",")
                printer.breakable(" ")
            result.append(tc.draw_and_print(generator, printer))
        if len(result) == 1:
            printer.text(",")
        printer.end_group(")")
        return tuple(result)

    def do_draw(self, tc: TestCase) -> tuple[Any, ...]:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> tuple[Any, ...]:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return "tuples(" + ", ".join(repr(g) for g in self.generators) + ")"


def tuples(*generators: Generator[Any]) -> Generator[tuple[Any, ...]]:
    """Generate tuples with one element from each generator."""
    return Tuples(tuple(as_generator(g) for g in generators))


def _draw_index_sample(tc: TestCase, n: int, min_size: int, max_size: int) -> list[int]:
    remaining = list(range(n))
    chosen: list[int] = []
    collection = Collection(tc, min_size, max_size)
    while remaining and collection.more():
        j = tc.generate_integer(0, len(remaining) - 1)
        chosen.append(remaining.pop(j))
    return chosen


class Subsequences(Generator[list[T]]):
    def __init__(self, elements: Sequence[T], min_size: int, max_size: int | None) -> None:
        self.elements = elements
        self.min_size = min_size
        self.max_size = max_size
        self.label = SUBSEQUENCE_LABEL

    def do_draw(self, tc: TestCase) -> list[T]:
        n = len(self.elements)
        if self.min_size > n:
            raise InvalidArgument(
                f"Cannot generate a subsequence: min_size {self.min_size} is larger than "
                f"the {n} elements in the sequence"
            )
        max_size = n if self.max_size is None else min(self.max_size, n)
        indices = sorted(_draw_index_sample(tc, n, self.min_size, max_size))
        return [self.elements[i] for i in indices]


def subsequences(
    elements: Iterable[T], *, min_size: int = 0, max_size: int | None = None
) -> Generator[list[T]]:
    """Generate subsequences of ``elements``: a subset in the original
    order."""
    min_size, max_size = _check_sizes(min_size, max_size)
    return Subsequences(list(elements), min_size, max_size)


class Permutations(Generator[list[T]]):
    def __init__(self, elements: Sequence[T]) -> None:
        self.elements = elements
        self.label = PERMUTATION_LABEL

    def do_draw(self, tc: TestCase) -> list[T]:
        n = len(self.elements)
        return [self.elements[i] for i in _draw_index_sample(tc, n, n, n)]


def permutations(elements: Iterable[T]) -> Generator[list[T]]:
    """Generate reorderings of ``elements``, shrinking toward the original
    order."""
    return Permutations(list(elements))


class Samples(Generator[list[T]]):
    def __init__(
        self, elements: Sequence[T], min_size: int, max_size: int | None, replacement: bool
    ) -> None:
        self.elements = elements
        self.min_size = min_size
        self.max_size = max_size
        self.replacement = replacement
        self.label = SAMPLE_LABEL

    def do_draw(self, tc: TestCase) -> list[T]:
        n = len(self.elements)
        if self.replacement:
            if n == 0 and self.min_size > 0:
                raise InvalidArgument("Cannot generate a non-empty sample from an empty sequence")
            max_size = 0 if n == 0 else self.max_size
            collection = Collection(tc, self.min_size, max_size)
            result = []
            while collection.more():
                result.append(self.elements[tc.generate_integer(0, n - 1)])
            return result
        if self.min_size > n:
            raise InvalidArgument(
                f"Cannot generate a sample without replacement: min_size {self.min_size} is "
                f"larger than the {n} elements in the sequence"
            )
        max_size = n if self.max_size is None else min(self.max_size, n)
        return [self.elements[i] for i in _draw_index_sample(tc, n, self.min_size, max_size)]


def samples(
    elements: Iterable[T],
    *,
    min_size: int = 0,
    max_size: int | None = None,
    replacement: bool = True,
) -> Generator[list[T]]:
    """Generate samples of ``elements``: with replacement (the default) each
    element is chosen independently; without, each is used at most once."""
    min_size, max_size = _check_sizes(min_size, max_size)
    return Samples(list(elements), min_size, max_size, bool(replacement))
