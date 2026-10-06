"""The generator protocol and the universal combinators."""

from __future__ import annotations

from collections.abc import Callable
from typing import TYPE_CHECKING, Any, Generic, TypeVar

from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.pretty import PrettyPrinter, pretty_print

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "FlatMapped",
    "Filtered",
    "Generator",
    "Mapped",
    "combine_labels",
    "label_from_name",
]

T = TypeVar("T")


U = TypeVar("U")

NOOP_PRINTER = PrettyPrinter.noop()

MAP_LABEL = label_from_name("hegel.map")
FLAT_MAP_LABEL = label_from_name("hegel.flat_map")
FILTER_LABEL = label_from_name("hegel.filter")
FILTER_ATTEMPT_LABEL = label_from_name("hegel.filter.attempt")


class Generator(Generic[T]):
    """A recipe for drawing values of some type from a :class:`TestCase`.

    Subclasses implement :meth:`do_draw` (and, when they draw component
    generators whose output should be printed structurally,
    :meth:`do_draw_and_print`; the default prints the drawn value with
    :func:`hegel.pretty.pretty_print`). ``label`` identifies the generator
    to the engine's shrinker: generators drawing the same shape of value
    share one, built from :func:`label_from_name` and
    :func:`combine_labels`.
    """

    label: int = label_from_name("hegel.generator")

    def do_draw(self, tc: TestCase) -> T:
        """Produce a value. Draw component generators with
        ``tc.draw_silent`` (or ``tc.draw_and_print``), never by calling
        their ``do_draw`` directly, so each gets a span of its own."""
        raise NotImplementedError

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        """Produce a value, printing its representation to ``printer``.
        Must draw exactly the same choices as :meth:`do_draw`."""
        value = self.do_draw(tc)
        pretty_print(value, printer)
        return value

    def map(self, f: Callable[[T], U]) -> Generator[U]:
        """Transform generated values with ``f``."""
        return Mapped(self, f)

    def flat_map(self, f: Callable[[T], Generator[U]]) -> Generator[U]:
        """Generate a value, then draw from the generator ``f`` builds
        from it."""
        return FlatMapped(self, f)

    def filter(self, predicate: Callable[[T], bool]) -> Generator[T]:
        """Keep only values satisfying ``predicate``. After three rejected
        attempts the test case is rejected as if by ``assume``."""
        return Filtered(self, predicate)

    def print_with(self, print: Callable[[T, PrettyPrinter], None]) -> Generator[T]:
        """Print each drawn value with ``print`` instead of its ``repr``."""
        return PrintedWith(self, print)

    def print_as_call(self, function: str) -> Generator[T]:
        """For a mapped generator: print ``function(input)`` around the
        source generator's own output instead of the mapped value."""
        raise InvalidArgument("print_as_call is only available on a mapped generator")

    def __or__(self, other: Generator[Any]) -> Generator[Any]:
        from hegel.generators.combinators import one_of

        return one_of(self, other)

    @property
    def is_empty(self) -> bool:
        """Whether this generator can never produce a value."""
        return False

    def __repr__(self) -> str:
        return f"{type(self).__name__}()"


class Mapped(Generator[U]):
    def __init__(self, source: Generator[T], f: Callable[[T], U]) -> None:
        self.source = source
        self.f = f
        self.label = combine_labels([MAP_LABEL, source.label])

    @property
    def is_empty(self) -> bool:
        return self.source.is_empty

    def do_draw(self, tc: TestCase) -> U:
        return self.f(tc.draw_silent(self.source))

    def print_as_call(self, function: str) -> Generator[U]:
        if not function:
            raise InvalidArgument("print_as_call requires a non-empty function name")
        return PrintedAsCall(self.source, self.f, function, self.label)

    def __repr__(self) -> str:
        return f"{self.source!r}.map({_name(self.f)})"


class PrintedAsCall(Generator[U]):
    def __init__(
        self, source: Generator[T], f: Callable[[T], U], function: str, label: int
    ) -> None:
        self.source = source
        self.f = f
        self.open = function + "("
        self.label = label

    @property
    def is_empty(self) -> bool:
        return self.source.is_empty

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> U:
        printer.begin_group(len(self.open), self.open)
        value = tc.draw_and_print(self.source, printer)
        printer.end_group(")")
        return self.f(value)

    def do_draw(self, tc: TestCase) -> U:
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> U:
        return self._draw(tc, printer)


class PrintedWith(Generator[T]):
    def __init__(self, source: Generator[T], print: Callable[[T, PrettyPrinter], None]) -> None:
        self.source = source
        self.print = print
        self.label = source.label

    @property
    def is_empty(self) -> bool:
        return self.source.is_empty

    def do_draw(self, tc: TestCase) -> T:
        return self.source.do_draw(tc)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        value = self.source.do_draw(tc)
        if printer.should_print:
            self.print(value, printer)
        return value


class FlatMapped(Generator[U]):
    def __init__(self, source: Generator[T], f: Callable[[T], Generator[U]]) -> None:
        self.source = source
        self.f = f
        self.label = combine_labels([FLAT_MAP_LABEL, source.label])

    @property
    def is_empty(self) -> bool:
        return self.source.is_empty

    def _next(self, tc: TestCase) -> Generator[U]:
        intermediate = tc.draw_silent(self.source)
        from hegel.generators.combinators import as_generator

        return as_generator(self.f(intermediate))

    def do_draw(self, tc: TestCase) -> U:
        return tc.draw_silent(self._next(tc))

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> U:
        return tc.draw_and_print(self._next(tc), printer)

    def __repr__(self) -> str:
        return f"{self.source!r}.flat_map({_name(self.f)})"


class Filtered(Generator[T]):
    def __init__(
        self,
        source: Generator[T],
        predicate: Callable[[T], bool],
        names: tuple[str, ...] | None = None,
    ) -> None:
        self.source = source
        self.predicate = predicate
        self.names = names if names is not None else (_name(predicate),)
        self.label = combine_labels([FILTER_LABEL, source.label])

    def filter(self, predicate: Callable[[T], bool]) -> Generator[T]:
        """Chained filters combine into one, so a long chain costs one span
        and one set of attempts rather than nesting a draw per filter."""
        first = self.predicate

        def both(value: T) -> bool:
            return bool(first(value)) and bool(predicate(value))

        return Filtered(self.source, both, (*self.names, _name(predicate)))

    @property
    def is_empty(self) -> bool:
        return self.source.is_empty

    def _draw(self, tc: TestCase, printer: PrettyPrinter, silent: bool) -> T:
        for _ in range(3):
            tc.start_span(FILTER_ATTEMPT_LABEL)
            speculation = printer.speculate()
            try:
                if silent:
                    value = tc.draw_silent(self.source)
                else:
                    value = tc.draw_and_print(self.source, printer)
                accepted = bool(self.predicate(value))
            except BaseException:
                speculation.abort()
                raise
            if accepted:
                speculation.commit()
                tc.stop_span(False)
                return value
            speculation.abort()
            tc.stop_span(True)
        tc.reject()
        raise AssertionError("unreachable")  # nocov

    def do_draw(self, tc: TestCase) -> T:
        return self._draw(tc, NOOP_PRINTER, True)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        return self._draw(tc, printer, False)

    def __repr__(self) -> str:
        return repr(self.source) + "".join(f".filter({name})" for name in self.names)


def _name(f: Any) -> str:
    return getattr(f, "__name__", None) or getattr(f, "__qualname__", None) or repr(f)
