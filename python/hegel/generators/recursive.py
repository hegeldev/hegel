"""Recursive and forward-referenced generators."""

from __future__ import annotations

import reprlib
import threading
from collections.abc import Callable
from typing import TYPE_CHECKING, TypeVar

from hegel._native import RetrySignal
from hegel.errors import InvalidArgument
from hegel.generators.base import NOOP_PRINTER, Generator, combine_labels, label_from_name
from hegel.generators.combinators import as_generator
from hegel.pretty import PrettyPrinter

if TYPE_CHECKING:
    from hegel import _native
    from hegel.test_case import TestCase

__all__ = ["deferred", "recursive"]

T = TypeVar("T")

DEFAULT_MAX_DEPTH = 32
DEFAULT_MAX_LEAVES = 100
RECURSIVE_LABEL = label_from_name("hegel.recursive")
DEFERRED_LABEL = label_from_name("hegel.deferred")
# A deferred definition drawn more deeply nested than this (a definition
# that always refers back to itself) rejects the test case instead of
# recursing until Python gives up.
# Below the test case's own span-depth limit, so a definition that only
# refers back to itself is rejected as an assumption rather than overrunning.
MAX_DEFERRED_DEPTH = 64


class _Subtree(Generator[T]):
    """The generator a ``recursive`` branch function receives: produces
    sub-values (each a leaf or a further branch) of the value under
    construction."""

    def __init__(self, owner: Recursive[T], recursion: _native.Recursion, depth: int) -> None:
        self.owner = owner
        self.recursion = recursion
        self.depth = depth
        self.label = owner.label

    def _child(self) -> _Subtree[T]:
        return _Subtree(self.owner, self.recursion, self.depth + 1)

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> T:
        if self.recursion.branch(tc._handle, self.depth):
            generator = as_generator(self.owner.branch(self._child()))
            result = tc.draw_and_print(generator, printer)
        else:
            self.recursion.leaf(tc._handle)
            result = tc.draw_and_print(self.owner.leaf, printer)
        if self.depth == 0:
            self.recursion.finish(tc._handle)
        return result

    def do_draw(self, tc: TestCase) -> T:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return f"<subtrees of {self.owner!r}>"


class Recursive(Generator[T]):
    def __init__(
        self,
        leaf: Generator[T],
        branch: Callable[[Generator[T]], Generator[T]],
        max_depth: int,
        max_leaves: int,
    ) -> None:
        self.leaf = leaf
        self.branch = branch
        self.max_depth = max_depth
        self.max_leaves = max_leaves
        self.label = combine_labels([RECURSIVE_LABEL, leaf.label])

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> T:
        base_depth = tc._open_span_depth
        recursion = tc._handle.new_recursion(self.max_depth, self.max_leaves)
        while True:
            root = _Subtree(self, recursion, 0)
            speculation = printer.speculate()
            try:
                value = root._draw(tc, printer)
            except RetrySignal:
                speculation.abort()
                try:
                    recursion.retry(tc._handle)
                finally:
                    tc._reset_open_spans_to(base_depth)
                continue
            except BaseException:
                speculation.abort()
                raise
            speculation.commit()
            return value

    def do_draw(self, tc: TestCase) -> T:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        return f"recursive({self.leaf!r}, {getattr(self.branch, '__name__', self.branch)!r})"

    @property
    def is_empty(self) -> bool:
        return self.leaf.is_empty


def recursive(
    leaf: Generator[T],
    branch: Callable[[Generator[T]], Generator[T]],
    *,
    max_depth: int = DEFAULT_MAX_DEPTH,
    max_leaves: int = DEFAULT_MAX_LEAVES,
) -> Generator[T]:
    """Generate recursively structured data.

    ``leaf`` generates the base cases; ``branch`` receives a generator of
    sub-values and returns a generator combining some of them into a
    compound value. Values are bounded by ``max_depth`` (nesting) and
    ``max_leaves`` (an attempt drawing more leaves is retried).
    """
    if not isinstance(max_depth, int) or max_depth < 0:
        raise InvalidArgument(f"max_depth={max_depth!r} must be a non-negative integer")
    if not isinstance(max_leaves, int) or max_leaves < 1:
        raise InvalidArgument(f"max_leaves={max_leaves!r} must be a positive integer")
    return Recursive(as_generator(leaf), branch, max_depth, max_leaves)


_calculating = threading.local()


class Deferred(Generator[T]):
    """A generator defined by a function called on first use, so a
    definition can refer to itself."""

    def __init__(self, definition: Callable[[], Generator[T]]) -> None:
        self.definition = definition
        self._generator: Generator[T] | None = None
        self._label: int | None = None
        self._lock = threading.RLock()

    def _get(self) -> Generator[T]:
        with self._lock:
            if self._generator is None:
                self._generator = as_generator(self.definition())
            return self._generator

    @property  # type: ignore[override]
    def label(self) -> int:
        if self._label is not None:
            return self._label
        generator = self._generator
        if generator is None:
            try:
                generator = self._get()
            except Exception:
                return DEFERRED_LABEL
        in_progress: set[int] = getattr(_calculating, "keys", None) or set()
        _calculating.keys = in_progress
        if id(self) in in_progress:
            return DEFERRED_LABEL
        in_progress.add(id(self))
        try:
            label = generator.label
        finally:
            in_progress.discard(id(self))
        self._label = label
        return label

    @label.setter
    def label(self, value: int) -> None:  # nocov
        self._label = value

    @property
    def is_empty(self) -> bool:
        """Whether the definition can never produce a value. A definition
        that reaches itself while this is computed is taken to be empty on
        that path, so only self-references with another way out count."""
        in_progress: set[int] = getattr(_calculating, "empty", None) or set()
        _calculating.empty = in_progress
        if id(self) in in_progress:
            return True
        in_progress.add(id(self))
        try:
            return self._get().is_empty
        finally:
            in_progress.discard(id(self))

    def _check_depth(self, tc: TestCase) -> None:
        if tc._open_span_depth > MAX_DEFERRED_DEPTH:
            tc.reject()

    def do_draw(self, tc: TestCase) -> T:
        self._check_depth(tc)
        return self._get().do_draw(tc)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        self._check_depth(tc)
        return self._get().do_draw_and_print(tc, printer)

    @reprlib.recursive_repr(fillvalue="...")
    def __repr__(self) -> str:
        if self._generator is None:
            return f"deferred({getattr(self.definition, '__name__', self.definition)!r})"
        return repr(self._generator)


def deferred(definition: Callable[[], Generator[T]]) -> Generator[T]:
    """A generator whose definition is evaluated on first use, enabling
    recursive and mutually recursive definitions."""
    if not callable(definition):
        raise InvalidArgument("deferred() takes a zero-argument function returning a generator")
    return Deferred(definition)
