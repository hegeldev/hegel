"""Collection generators: lists, sets, dicts, tuples, and samples."""

from __future__ import annotations

from collections.abc import Callable, Hashable, Iterable, Mapping, Sequence
from typing import TYPE_CHECKING, Any, TypeVar

from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Filtered, Generator, Mapped, _PrintsValue
from hegel.generators.combinators import FilteredSample, Just, SampledFrom, as_generator
from hegel.generators.numeric import Booleans, Integers
from hegel.pretty import _SILENT_PRINTER, PrettyPrinter
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
    keys = tuple(unique_by) if isinstance(unique_by, Iterable) else ()
    if not keys or not all(callable(k) for k in keys):
        raise InvalidArgument("unique_by must be a callable or a non-empty tuple of callables")
    return keys


def _distinct_values(generator: Generator[Any]) -> int | None:
    """How many distinct values ``generator`` can produce, when its
    definition says."""
    if isinstance(generator, Just):
        return 1
    if isinstance(generator, Booleans):
        return 1 if generator.p in (0.0, 1.0) else 2
    if isinstance(generator, SampledFrom):
        try:
            return len(set(generator.elements))
        except TypeError:
            return len(generator.elements)
    if (
        isinstance(generator, Integers)
        and generator.min_value is not None
        and generator.max_value is not None
    ):
        return generator.max_value - generator.min_value + 1
    return None


def _check_distinct(what: str, elements: Generator[Any], min_size: int) -> None:
    """A collection of ``min_size`` distinct values needs that many to
    choose from."""
    count = _distinct_values(elements)
    if count is not None and count < min_size:
        raise InvalidArgument(
            f"Cannot generate {min_size} unique {what} from {elements!r}, which has only "
            f"{count} distinct value{'' if count == 1 else 's'}"
        )


class _WithoutReplacement:
    """Elements of a ``sampled_from``, under any ``map`` and ``filter``
    layers, drawn without replacement: each draw uses up one of the
    sampled elements, so a unique collection that needs most of them is
    filled without rejecting duplicates, and the draws stop when the
    elements run out."""

    __slots__ = ("elements", "layers", "remaining")

    def __init__(
        self, elements: Sequence[Any], layers: list[tuple[bool, Callable[[Any], Any]]]
    ) -> None:
        self.elements = elements
        self.layers = layers
        self.remaining = list(range(len(elements)))

    @classmethod
    def of(cls, generator: Generator[Any]) -> _WithoutReplacement | None:
        """A sampler for ``generator``, when it is a ``sampled_from`` under
        ``map`` and ``filter`` layers, else None."""
        layers: list[tuple[bool, Callable[[Any], Any]]] = []
        while not isinstance(generator, SampledFrom):
            if isinstance(generator, Mapped):
                layers.append((False, generator.f))
            elif isinstance(generator, Filtered):
                layers.append((True, generator.predicate))
            elif isinstance(generator, FilteredSample):
                layers.append((True, generator._accepts))
            else:
                return None
            generator = generator.source
        layers.reverse()
        return cls(generator.elements, layers)

    def draw(self, tc: TestCase, printer: PrettyPrinter) -> tuple[bool, Any]:
        """Use up one element: ``(True, value)`` when it passes the
        filters, printed, else ``(False, None)``."""
        index = self.remaining.pop(tc._generate_integer(0, len(self.remaining) - 1))
        value: Any = self.elements[index]
        for is_filter, function in self.layers:
            if is_filter:
                if not function(value):
                    return False, None
            else:
                value = function(value)
        printer.repr(value)
        return True, value


def _more(collection: Collection, sampler: _WithoutReplacement | None) -> bool:
    return (sampler is None or bool(sampler.remaining)) and collection.more()


def _short(tc: TestCase, sampler: _WithoutReplacement | None, size: int, min_size: int) -> None:
    """Reject the test case when sampling without replacement ran out
    of elements before the collection reached its minimum size."""
    if sampler is not None and size < min_size:
        tc.reject()


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
        collection = Collection(tc, self.min_size, self.max_size)
        result: list[T] = []
        seen: list[set[Hashable]] = [set() for _ in (self.unique_by or ())]
        sampler = _WithoutReplacement.of(self.elements) if self.unique_by else None
        while _more(collection, sampler):
            with printer.speculate() as speculation:
                if result:
                    printer.text(",")
                    printer.breakable(" ")
                if sampler is None:
                    element = tc.draw_and_print(self.elements, printer)
                else:
                    found, element = sampler.draw(tc, printer)
                    if not found:
                        speculation.abort()
                        collection.reject("filtered element")
                        continue
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
        _short(tc, sampler, len(result), self.min_size)
        printer.end_group("]")
        return result

    def do_draw(self, tc: TestCase) -> list[T]:
        return self._draw(tc, _SILENT_PRINTER)

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
    """Generate lists of values from ``elements``, with between
    ``min_size`` and ``max_size`` elements.

    ``max_size`` defaults to no fixed limit: the engine decides when to
    stop, and keeps lists reasonably short. Lists shrink toward fewer
    elements, and each element shrinks as ``elements`` does.

    ``unique=True`` requires the elements to be distinct (and hashable).
    ``unique_by`` is a key function, or a tuple of them, each of whose
    results must be distinct across the list; it cannot be combined with
    ``unique``. A duplicate element is discarded and another drawn in its
    place. When the elements are a :func:`sampled_from` (under any
    :meth:`~Generator.map` and :meth:`~Generator.filter`), unique lists
    draw its elements without replacement, so a unique list needing most
    of them is still found quickly.

    A negative size, ``max_size < min_size``, or a ``min_size`` larger than
    the number of distinct values ``elements`` is known to have, is an
    :class:`~hegel.InvalidArgument`.

    ::

        xs = tc.draw(gs.lists(gs.integers(), min_size=1, max_size=10))
        ids = tc.draw(gs.lists(gs.integers(), unique=True))
        pairs = tc.draw(gs.lists(gs.tuples(gs.text(), gs.integers()), unique_by=lambda p: p[0]))
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
    if keys is not None:
        _check_distinct("elements", elements, min_size)
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
        name = "frozenset" if self.frozen else "set"
        open_, close = ("frozenset({", "})") if self.frozen else ("{", "}")
        result: set[Any] = set()
        with printer.speculate() as whole:
            printer.begin_group(len(open_), open_)
            collection = Collection(tc, self.min_size, self.max_size)
            sampler = _WithoutReplacement.of(self.elements)
            while _more(collection, sampler):
                with printer.speculate() as speculation:
                    if result:
                        printer.text(",")
                        printer.breakable(" ")
                    if sampler is None:
                        element = tc.draw_and_print(self.elements, printer)
                    else:
                        found, element = sampler.draw(tc, printer)
                        if not found:
                            speculation.abort()
                            collection.reject("filtered element")
                            continue
                    if element in result:
                        speculation.abort()
                        collection.reject("duplicate element")
                    else:
                        speculation.commit()
                        result.add(element)
            _short(tc, sampler, len(result), self.min_size)
            printer.end_group(close)
            if result:
                whole.commit()
        if not result:
            printer.text(f"{name}()")
        return frozenset(result) if self.frozen else result

    def do_draw(self, tc: TestCase) -> Any:
        return self._draw(tc, _SILENT_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> Any:
        return self._draw(tc, printer)

    @property
    def _name(self) -> str:
        return "frozensets" if self.frozen else "sets"

    def __repr__(self) -> str:
        return (
            f"{self._name}({self.elements!r}, min_size={self.min_size}, max_size={self.max_size})"
        )


def _sets(elements: Any, min_size: int, max_size: int | None, frozen: bool) -> Sets:
    elements = as_generator(elements)
    min_size, max_size = _check_sizes(min_size, max_size)
    _check_distinct("elements", elements, min_size)
    return Sets(elements, min_size, max_size, frozen)


def sets(
    elements: Generator[T], *, min_size: int = 0, max_size: int | None = None
) -> Generator[set[T]]:
    """Generate sets of values from ``elements``, with between
    ``min_size`` and ``max_size`` members; the elements must be hashable.
    Sizes are as for :func:`lists`, and a drawn duplicate is discarded and
    another element drawn in its place."""
    return _sets(elements, min_size, max_size, False)


def frozensets(
    elements: Generator[T], *, min_size: int = 0, max_size: int | None = None
) -> Generator[frozenset[T]]:
    """Generate frozensets of values from ``elements``, as :func:`sets`
    does."""
    return _sets(elements, min_size, max_size, True)


def iterables(
    elements: Generator[T],
    *,
    min_size: int = 0,
    max_size: int | None = None,
    unique: bool = False,
    unique_by: Any = None,
) -> Generator[Iterable[T]]:
    """Generate one-shot iterables of values from ``elements``: an
    iterator over a list that :func:`lists` would generate, with the same
    arguments. Iterating once exhausts it, which tests code that must not
    iterate its input twice. The failure report prints the values it
    started with."""
    one_shot: Generator[Iterable[T]] = lists(
        elements, min_size=min_size, max_size=max_size, unique=unique, unique_by=unique_by
    ).map(_OneShot)
    return one_shot.print_as_call("iter")


class _OneShot:
    """The iterator ``iterables`` hands to the test: one pass exhausts it,
    and it is reported as the values it started with."""

    def __init__(self, values: list[Any]) -> None:
        self._values = values
        self._iterator = iter(values)

    def __iter__(self) -> _OneShot:
        return self

    def __next__(self) -> Any:
        return next(self._iterator)

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
        result: dict[Any, Any] = {}
        collection = Collection(tc, self.min_size, self.max_size)
        sampler = _WithoutReplacement.of(self.keys)
        while _more(collection, sampler):
            with printer.speculate() as speculation:
                if result:
                    printer.text(",")
                    printer.breakable(" ")
                if sampler is None:
                    key = tc.draw_and_print(self.keys, printer)
                else:
                    found, key = sampler.draw(tc, printer)
                    if not found:
                        speculation.abort()
                        collection.reject("filtered key")
                        continue
                if key in result:
                    speculation.abort()
                    collection.reject("duplicate key")
                    continue
                printer.text(": ")
                value = tc.draw_and_print(self.values, printer)
                speculation.commit()
                result[key] = value
        _short(tc, sampler, len(result), self.min_size)
        printer.end_group("}")
        return result

    def do_draw(self, tc: TestCase) -> dict[Any, Any]:
        return self._draw(tc, _SILENT_PRINTER)

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
    """Generate dicts with keys drawn from ``keys`` and values from
    ``values``, with between ``min_size`` and ``max_size`` entries.

    Sizes are as for :func:`lists`. A drawn duplicate key is discarded and
    another drawn in its place. For dicts with a fixed set of keys, use
    :func:`fixed_dicts`.

    ::

        scores = tc.draw(gs.dicts(gs.text(), gs.integers(0, 100)))
    """
    keys, values = as_generator(keys), as_generator(values)
    min_size, max_size = _check_sizes(min_size, max_size)
    _check_distinct("keys", keys, min_size)
    return Dicts(keys, values, min_size, max_size)


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
        printer.begin_group(1, "{")
        result: dict[Any, Any] = {}
        for key, generator in self.mapping.items():
            if result:
                printer.text(",")
                printer.breakable(" ")
            printer.repr(key)
            printer.text(": ")
            result[key] = tc.draw_and_print(generator, printer)
        for key, generator in self.optional.items():
            if not tc._generate_boolean(0.5):
                continue
            with printer.speculate() as speculation:
                if result:
                    printer.text(",")
                    printer.breakable(" ")
                printer.repr(key)
                printer.text(": ")
                result[key] = tc.draw_and_print(generator, printer)
                speculation.commit()
        printer.end_group("}")
        return result

    def do_draw(self, tc: TestCase) -> dict[Any, Any]:
        return self._draw(tc, _SILENT_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> dict[Any, Any]:
        return self._draw(tc, printer)


def fixed_dicts(
    mapping: dict[Any, Generator[Any]], *, optional: dict[Any, Generator[Any]] | None = None
) -> Generator[dict[Any, Any]]:
    """Generate dicts with the keys of ``mapping``, each value drawn from
    the generator that ``mapping`` gives for its key.

    Each key of ``optional`` (a dict of the same shape) is present or
    absent independently, shrinking toward absent. A key in both, or a
    value that is not a generator, is an :class:`~hegel.InvalidArgument`.
    The keys appear in the order given, required keys first.

    ::

        user = tc.draw(gs.fixed_dicts(
            {"name": gs.text(), "age": gs.integers(0, 120)},
            optional={"email": gs.emails()},
        ))
    """
    if not isinstance(mapping, Mapping):
        raise InvalidArgument(f"mapping={mapping!r} must be a dict of generators")
    if optional is not None and not isinstance(optional, Mapping):
        raise InvalidArgument(f"optional={optional!r} must be a dict of generators")
    fixed = {k: as_generator(v) for k, v in mapping.items()}
    opt = {k: as_generator(v) for k, v in (optional or {}).items()}
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
        return self._draw(tc, _SILENT_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> tuple[Any, ...]:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return "tuples(" + ", ".join(repr(g) for g in self.generators) + ")"


def tuples(*generators: Generator[Any]) -> Generator[tuple[Any, ...]]:
    """Generate tuples with one element from each generator, in order:
    ``tuples(gs.integers(), gs.text())`` generates pairs like
    ``(0, "")``. With no generators it generates only ``()``."""
    return Tuples(tuple(as_generator(g) for g in generators))


def _draw_index_sample(tc: TestCase, n: int, min_size: int, max_size: int) -> list[int]:
    remaining = list(range(n))
    chosen: list[int] = []
    collection = Collection(tc, min_size, max_size)
    while remaining and collection.more():
        j = tc._generate_integer(0, len(remaining) - 1)
        chosen.append(remaining.pop(j))
    return chosen


class Subsequences(_PrintsValue[list[T]]):
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

    def __repr__(self) -> str:
        return (
            f"subsequences({list(self.elements)!r}, "
            f"min_size={self.min_size}, max_size={self.max_size})"
        )


def subsequences(
    elements: Iterable[T], *, min_size: int = 0, max_size: int | None = None
) -> Generator[list[T]]:
    """Generate subsequences of ``elements``: lists of some of its
    elements, each used at most once, in their original order. Sizes are
    as for :func:`lists`, capped at ``len(elements)``; a ``min_size``
    larger than that is an :class:`~hegel.InvalidArgument` when drawn."""
    min_size, max_size = _check_sizes(min_size, max_size)
    return Subsequences(list(elements), min_size, max_size)


class Permutations(_PrintsValue[list[T]]):
    def __init__(self, elements: Sequence[T]) -> None:
        self.elements = elements
        self.label = PERMUTATION_LABEL

    def do_draw(self, tc: TestCase) -> list[T]:
        n = len(self.elements)
        return [self.elements[i] for i in _draw_index_sample(tc, n, n, n)]

    def __repr__(self) -> str:
        return f"permutations({list(self.elements)!r})"


def permutations(elements: Iterable[T]) -> Generator[list[T]]:
    """Generate reorderings of ``elements`` (any iterable, read once), as
    lists, shrinking toward the original order."""
    return Permutations(list(elements))


class Samples(_PrintsValue[list[T]]):
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
                result.append(self.elements[tc._generate_integer(0, n - 1)])
            return result
        if self.min_size > n:
            raise InvalidArgument(
                f"Cannot generate a sample without replacement: min_size {self.min_size} is "
                f"larger than the {n} elements in the sequence"
            )
        max_size = n if self.max_size is None else min(self.max_size, n)
        return [self.elements[i] for i in _draw_index_sample(tc, n, self.min_size, max_size)]

    def __repr__(self) -> str:
        return (
            f"samples({list(self.elements)!r}, min_size={self.min_size}, "
            f"max_size={self.max_size}, replacement={self.replacement})"
        )


def samples(
    elements: Iterable[T],
    *,
    min_size: int = 0,
    max_size: int | None = None,
    replacement: bool = True,
) -> Generator[list[T]]:
    """Generate lists sampled from ``elements``, with between ``min_size``
    and ``max_size`` items.

    With ``replacement=True`` (the default) each item is chosen
    independently, so an element may appear more than once. With
    ``replacement=False`` each element is used at most once, in the order
    drawn, so the size is capped at ``len(elements)``. A ``min_size`` that
    cannot be met is an :class:`~hegel.InvalidArgument` when drawn.
    """
    min_size, max_size = _check_sizes(min_size, max_size)
    return Samples(list(elements), min_size, max_size, bool(replacement))
