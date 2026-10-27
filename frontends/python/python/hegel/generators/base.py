"""The generator protocol and the universal combinators."""

from __future__ import annotations

from collections.abc import Callable
from typing import TYPE_CHECKING, Any, Generic, TypeVar

from hegel import _native
from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument, NotPrintable
from hegel.pretty import _SILENT_PRINTER, PrettyPrinter

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "Filtered",
    "FlatMapped",
    "Generator",
    "Mapped",
    "PrintedAsCall",
    "PrintedAsRepr",
    "PrintedWith",
]

T = TypeVar("T")


U = TypeVar("U")

MAP_LABEL = label_from_name("hegel.map")
FLAT_MAP_LABEL = label_from_name("hegel.flat_map")
FILTER_LABEL = label_from_name("hegel.filter")
FILTER_ATTEMPT_LABEL = label_from_name("hegel.filter.attempt")


class Generator(Generic[T]):
    """A recipe for drawing values of some type from a :class:`~hegel.TestCase`.

    Generators are built by the functions in :mod:`hegel.generators` and
    drawn from with :meth:`TestCase.draw <hegel.TestCase.draw>`. They are
    immutable descriptions of how to make a value: the same generator can
    be drawn from any number of times, in any number of tests. Every
    generator has the combinators :meth:`map`, :meth:`filter` and
    :meth:`flat_map`, and ``a | b`` is :func:`~hegel.generators.one_of`
    of the two::

        evens = gs.integers().map(lambda n: n * 2)
        small = gs.integers().filter(lambda n: abs(n) < 100)
        numbers = gs.integers() | gs.floats()

    Most custom generators are best written with
    :func:`~hegel.generators.composite`. Subclassing is for generators that
    need to control how their values are printed or how they are labelled.

    Printing is the generator's job. :meth:`TestCase.draw
    <hegel.TestCase.draw>` reports the drawn value, so it needs a
    *printable* generator: one that implements :meth:`do_draw_and_print`,
    drawing its components with ``tc.draw_and_print(component, printer)``
    and writing its own syntax to the printer as it goes. Such a subclass
    gets :meth:`do_draw` for free::

        class Coords(gs.Generator[tuple[int, int]]):
            def do_draw_and_print(self, tc, printer):
                with printer.group("Coord(", ")"):
                    x = tc.draw_and_print(gs.integers(0, 9), printer)
                    printer.separator()
                    y = tc.draw_and_print(gs.integers(0, 9), printer)
                return (x, y)

    A subclass that implements only :meth:`do_draw` draws silently: it can
    be drawn with :meth:`TestCase.draw_silent <hegel.TestCase.draw_silent>`
    or used as the source of :meth:`map`, and :meth:`print_as_repr` or
    :meth:`print_with` make it printable. Drawing it with ``tc.draw``
    raises :class:`~hegel.NotPrintable`. ``label`` identifies the generator
    to the engine's shrinker: generators drawing the same shape of value
    share one. A subclass that sets no ``label`` of its own is labelled by
    its qualified class name, so two generator classes shrink apart.
    """

    label: int = label_from_name("hegel.generator")
    _label_is_default = True

    def __init_subclass__(cls, **kwargs: Any) -> None:
        super().__init_subclass__(**kwargs)
        if "label" in cls.__dict__:
            cls._label_is_default = False
        elif cls._label_is_default:
            cls.label = label_from_name(f"{cls.__module__}.{cls.__qualname__}")

    def do_draw(self, tc: TestCase) -> T:
        """Produce a value without printing it. Draw component generators
        with ``tc.draw_silent`` (or ``tc.draw_and_print``), never by calling
        their ``do_draw`` directly, so each gets a span of its own.

        The default runs :meth:`do_draw_and_print` with a no-op printer, so
        a printable generator writes its drawing logic once."""
        if _prints_itself(type(self)):
            return self.do_draw_and_print(tc, _SILENT_PRINTER)
        raise NotImplementedError(
            f"{type(self).__name__} must implement do_draw or do_draw_and_print"
        )

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        """Produce a value, printing its representation to ``printer`` as
        it is drawn. Must draw exactly the same choices as :meth:`do_draw`.

        There is no default: a generator that does not implement it cannot
        print what it draws, and drawing it with ``tc.draw`` raises
        :class:`~hegel.NotPrintable`."""
        raise NotPrintable(
            f"{self!r} cannot print the values it draws, so tc.draw() cannot report it. "
            f"Implement do_draw_and_print(tc, printer) on {type(self).__name__}, or make "
            "it printable with .print_with(lambda value, printer: ...) or .print_as_repr(), "
            "or draw it with tc.draw_silent() if it is not worth reporting."
        )

    def map(self, f: Callable[[T], U]) -> Generator[U]:
        """A generator of ``f(value)`` for each ``value`` this one generates.

        The mapped value is what the failure report prints. ``f`` runs every
        time a value is drawn, and shrinking happens on the underlying value,
        so a mapped generator shrinks toward ``f`` of the source's simplest
        value::

            evens = gs.integers().map(lambda n: n * 2)
        """
        return Mapped(self, f)

    def flat_map(self, f: Callable[[T], Generator[U]]) -> Generator[U]:
        """Generate a value, then draw from the generator ``f`` builds
        from it.

        ``f`` must return a :class:`Generator`; anything else is an
        :class:`~hegel.InvalidArgument`. Only the final value is printed.
        Use this when the shape of one value depends on another::

            lists_and_index = gs.integers(1, 10).flat_map(
                lambda n: gs.tuples(
                    gs.lists(gs.integers(), min_size=n, max_size=n),
                    gs.integers(0, n - 1),
                )
            )

        A :func:`~hegel.generators.composite` generator expresses the same
        dependency more readably, and is usually the better choice.
        """
        return FlatMapped(self, f)

    def filter(self, predicate: Callable[[T], bool]) -> Generator[T]:
        """Keep only values satisfying ``predicate``.

        Each draw tries the source as many times as the engine's
        ``HEGEL_FILTER_ATTEMPTS``, which every Hegel library's filter
        shares; if no attempt satisfies ``predicate``,
        the test case is rejected as if by
        :meth:`TestCase.assume <hegel.TestCase.assume>`. A filter that
        rejects most values therefore rejects many test cases, and may fail
        the ``filter_too_much`` health check: prefer generating the values
        you want directly (``gs.integers(min_value=0)`` rather than
        ``gs.integers().filter(lambda n: n >= 0)``). Chained filters
        combine into one, and filtering
        :func:`~hegel.generators.sampled_from` narrows its elements instead
        of retrying, so even a rare element is found.
        """
        return Filtered(self, predicate)

    def print_with(self, print: Callable[[T, PrettyPrinter], None]) -> Generator[T]:
        """A generator of the same values that prints each one with
        ``print``, in place of however this generator prints.

        ``print`` is called as ``print(value, printer)`` with a
        :class:`~hegel.PrettyPrinter` to write the value's representation
        into the failure report; it is only called when the output will be
        shown. It works on any generator, printable or not, and is the way
        to print a custom representation or mask a secret::

            tokens = gs.text().print_with(lambda value, printer: printer.text("<token>"))
        """
        return PrintedWith(self, print)

    def print_as_repr(self) -> Generator[T]:
        """A generator of the same values that prints each one as its
        ``repr`` (through :meth:`PrettyPrinter.repr
        <hegel.PrettyPrinter.repr>`), in place of however this generator
        prints. This makes a subclass that implements only :meth:`do_draw`
        printable."""
        return PrintedAsRepr(self)

    def print_as_call(self, function: str) -> Generator[T]:
        """For a mapped generator: print ``function(input)`` around the
        source generator's own output instead of the mapped value.

        This makes a value whose ``repr`` is unhelpful print as the call
        that built it::

            points = gs.tuples(gs.integers(), gs.integers()).map(
                lambda t: Point(*t)
            ).print_as_call("Point")

        prints a drawn point as ``Point((0, 0))``. Calling it on a
        generator that is not the result of :meth:`map`, or with an empty
        name, is an :class:`~hegel.InvalidArgument`.
        """
        raise InvalidArgument("print_as_call is only available on a mapped generator")

    def __or__(self, other: Generator[Any]) -> Generator[Any]:
        """``a | b`` is ``one_of(a, b)``."""
        from hegel.generators.combinators import one_of

        return one_of(self, other)

    def __repr__(self) -> str:
        return f"{type(self).__name__}()"


def _prints_itself(cls: type[Generator[Any]]) -> bool:
    return cls.do_draw_and_print not in (
        Generator.do_draw_and_print,
        _PrintsValue.do_draw_and_print,
    )


class _PrintsValue(Generator[T]):
    """A generator that prints the value it draws as its ``repr``: the
    leaves and the value-producing generators. Subclasses implement
    :meth:`do_draw`."""

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        value = self.do_draw(tc)
        printer.repr(value)
        return value


class PrintedAsRepr(_PrintsValue[T]):
    def __init__(self, source: Generator[T]) -> None:
        self.source = source
        self.label = source.label

    def do_draw(self, tc: TestCase) -> T:
        return self.source.do_draw(tc)

    def __repr__(self) -> str:
        return f"{self.source!r}.print_as_repr()"


class Mapped(_PrintsValue[U]):
    def __init__(self, source: Generator[T], f: Callable[[T], U]) -> None:
        self.source = source
        self.f = f
        self.label = combine_labels([MAP_LABEL, source.label])

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

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> U:
        printer.begin_group(len(self.open), self.open)
        value = tc.draw_and_print(self.source, printer)
        printer.end_group(")")
        return self.f(value)


class PrintedWith(Generator[T]):
    def __init__(self, source: Generator[T], print: Callable[[T, PrettyPrinter], None]) -> None:
        self.source = source
        self.print = print
        self.label = source.label

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

    def _draw(self, tc: TestCase, printer: PrettyPrinter, silent: bool) -> T:
        for _ in range(_native.FILTER_ATTEMPTS):
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
        return self._draw(tc, _SILENT_PRINTER, True)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        return self._draw(tc, printer, False)

    def __repr__(self) -> str:
        return repr(self.source) + "".join(f".filter({name})" for name in self.names)


def _name(f: Any) -> str:
    return getattr(f, "__name__", None) or getattr(f, "__qualname__", None) or repr(f)
