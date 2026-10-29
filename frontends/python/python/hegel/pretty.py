"""Pretty printing of drawn values, laid out by the engine's printer.

When a test fails, Hegel replays the minimal failing example and prints
every drawn value as a ``name = value`` line. Printing is a capability of
the *generator*, not the value: a generator draws and prints in one step
(:meth:`hegel.generators.Generator.do_draw_and_print`), writing to a
:class:`PrettyPrinter` as it draws. Containers print element by element, so
an element's own printing shows up inside its container, and a generator
can describe a value that can only be described after it is drawn by
opening a :meth:`PrettyPrinter.deferred` region. Value-producing generators
(``map``, ``just``, ``sampled_from``, ``composite``) print the finished
value's ``repr``. Printing a finished value through a function
(:meth:`~hegel.generators.Generator.print_with`,
:meth:`~hegel.generators.Generator.print_as_repr`) is an escape hatch for
generators that do not print themselves.

The engine does all layout: a :class:`Document` owns one pretty-printed
document, its :attr:`~Document.printer` is the write surface, and
:meth:`Document.finish` renders it.
"""

from __future__ import annotations

from collections.abc import Callable, Iterable, Iterator
from contextlib import contextmanager
from typing import Any, TypeVar

from hegel import _native
from hegel.errors import InvalidArgument

__all__ = [
    "DEFAULT_MAX_WIDTH",
    "Document",
    "PrettyPrinter",
    "Speculation",
    "pretty_print",
    "pretty_repr",
]

DEFAULT_MAX_WIDTH = 79

T = TypeVar("T")


class PrettyPrinter:
    """The write surface onto a region of an engine-side document: what a
    printable generator's ``do_draw_and_print``, or a
    :meth:`~hegel.generators.Generator.print_with` function, writes a
    value's representation to.

    The engine lays the document out with an Oppen-style pretty-printer.
    Output is built from three primitives: :meth:`text` emits literal text,
    :meth:`breakable` marks a point that renders as a separator when the
    enclosing group fits on one line and as a newline plus indentation when
    it does not, and :meth:`begin_group` / :meth:`end_group` delimit the
    groups those decisions are made over. :meth:`repr` prints a value's
    ``repr`` with its bracket structure recovered, so it wraps like a value
    printed structurally; :meth:`group`, :meth:`seq` and :meth:`separator`
    are the conveniences most printing code wants.

    A printer is only ever handed out, never constructed by printing code:
    get one from a :class:`Document`, or from the test case through
    :meth:`TestCase.draw_and_print <hegel.TestCase.draw_and_print>`. A
    no-op printer (``PrettyPrinter.noop()``) accepts every call and prints
    nothing, so a generator runs one code path for silent and printing
    draws.
    """

    __slots__ = ("_handle", "_silent")

    def __init__(self, _handle: _native.Printer | None = None, _silent: bool = False) -> None:
        self._handle = _handle
        self._silent = _silent

    @classmethod
    def noop(cls) -> PrettyPrinter:
        """A printer that discards everything written to it. Drawing
        through it with :meth:`TestCase.draw_and_print
        <hegel.TestCase.draw_and_print>` still needs a printable generator;
        only :meth:`TestCase.draw_silent <hegel.TestCase.draw_silent>`
        makes a draw silent."""
        return cls(None)

    @property
    def should_print(self) -> bool:
        """Whether output written now would appear anywhere: ``False`` for a
        no-op printer and for a dead region (see :meth:`deferred`). Guard
        any work done purely for printing with this."""
        return self._handle is not None and self._handle.is_live()

    def text(self, text: str) -> None:
        """Emit literal text. Newlines become hard breaks."""
        if self._handle is None:
            return
        first = True
        for line in text.split("\n"):
            if not first:
                self._handle.hard_break()
            first = False
            if line:
                self._handle.text(line)

    def repr(self, value: Any) -> None:
        """Print ``repr(value)``, reflowed by the engine so it wraps like a
        value printed structurally. ``repr`` is only called when the output
        will be shown."""
        if self.should_print:
            self.reflow(repr(value))

    def reflow(self, text: str) -> None:
        """Emit a one-line representation (a ``repr``), letting the engine
        recover its bracket structure so it wraps like a value printed
        structurally. Text that does not parse is emitted verbatim."""
        if self._handle is not None:
            self._handle.reflow(text)

    def breakable(self, sep: str = " ") -> None:
        """A break point: ``sep`` when the enclosing group fits on one line,
        a newline plus indentation otherwise."""
        if self._handle is not None:
            self._handle.breakable(sep)

    def if_break(self, text: str) -> None:
        """Text emitted only when the enclosing group breaks (a trailing
        comma, say)."""
        if self._handle is not None:
            self._handle.if_break(text)

    def hard_break(self) -> None:
        """A newline, whether or not the enclosing group fits."""
        if self._handle is not None:
            self._handle.hard_break()

    def comment(self, text: str) -> None:
        """Attach a comment to the line currently being written: ``text``
        renders as ``  # text`` at the end of that line, and every group
        open at this position breaks, since nothing else may share a line
        with a comment. ``text`` must not contain newlines."""
        if self._handle is not None:
            self._handle.comment(f"  # {text}")

    def begin_group(self, indent: int, open: str) -> None:
        """Open a group: emit ``open`` and indent break points inside it by
        ``indent`` columns. Every ``begin_group`` needs a matching
        :meth:`end_group`; :meth:`group` pairs them for you."""
        if self._handle is not None:
            self._handle.begin_group(indent, open)

    def end_group(self, close: str) -> None:
        """Close the group :meth:`begin_group` opened, emitting ``close``."""
        if self._handle is not None:
            self._handle.end_group(close)

    @contextmanager
    def group(self, open: str, close: str, indent: int | None = None) -> Iterator[None]:
        """A group around the body of a ``with`` block: ``open``, the
        body's output with break points indented by ``indent`` columns
        (default ``len(open)``, aligning continuation lines just inside
        ``open``), then ``close``, even if the body raises. Note the
        argument order differs from :meth:`begin_group`'s, so that the
        indent can default::

            with printer.group("Point(", ")"):
                printer.repr(x)
                printer.separator()
                printer.repr(y)
        """
        self.begin_group(len(open) if indent is None else indent, open)
        try:
            yield
        finally:
            self.end_group(close)

    def seq(
        self,
        open: str,
        close: str,
        items: Iterable[T],
        print_item: Callable[[PrettyPrinter, T], None],
    ) -> None:
        """Print ``items`` as a delimited, comma-separated sequence:
        ``open``, each item as ``print_item(printer, item)`` prints it with
        a :meth:`separator` between consecutive items, then ``close``. It
        is laid out inline when it fits, and one item per line aligned just
        inside ``open`` when it does not::

            printer.seq("{", "}", [1, 2, 3], lambda p, n: p.repr(n))
        """
        with self.group(open, close):
            for index, item in enumerate(items):
                if index > 0:
                    self.separator()
                print_item(self, item)

    def separator(self) -> None:
        """The separator between two items of a group: a ``,`` followed by
        a break point that renders as a space when the group fits."""
        self.text(",")
        self.breakable(" ")

    def shift_indent(self, delta: int) -> None:
        """Adjust the indentation that later break points apply by
        ``delta`` columns."""
        if self._handle is not None:
            self._handle.shift_indent(delta)

    def speculate(self) -> Speculation:
        """Open a speculative region whose output is kept only if the
        returned :class:`Speculation` is committed. This is how printing
        survives rejection: a generator that may retract a draw (a filter
        retry, a duplicate collection element) prints each attempt into a
        speculation and commits only the accepted one."""
        if self._handle is not None:
            self._handle.begin_speculative()
        return Speculation(self)

    def deferred(self) -> PrettyPrinter:
        """A child region anchored at the current position: whatever the
        returned printer prints, at any later point and from any thread
        that owns it, appears here when the document renders, laid out as
        if it had been printed inline.

        This is how a generator prints a value that can only be described
        after it is drawn: it opens a deferred region at draw time and
        hands it to the value, which records into it as it is used. A
        region dies when the document renders, or when a speculation its
        anchor sat inside is aborted; writes to a dead region are silent
        no-ops and its :attr:`should_print` is ``False``. A deferred
        region of a no-op printer is a no-op printer."""
        if self._handle is None:
            return PrettyPrinter(None, self._silent)
        return PrettyPrinter(self._handle.deferred())

    def _value(self) -> str:
        if self._handle is None:
            return ""
        try:
            self._handle.resolve()
        except InvalidArgument:
            pass  # no deferred session outstanding
        return self._handle.value().decode("utf-8", "surrogatepass")


class Speculation:
    """A speculative print region; see :meth:`PrettyPrinter.speculate`.
    Print through :attr:`printer`, then :meth:`commit` to keep the output or
    :meth:`abort` to discard it.

    Usable as a context manager: leaving the block without a commit aborts
    the region, so an unwinding draw never leaves partial output behind.
    """

    __slots__ = ("_resolved", "printer")

    def __init__(self, printer: PrettyPrinter) -> None:
        self.printer = printer
        self._resolved = False

    def commit(self) -> None:
        """Close the region, keeping its output."""
        if self._resolved:
            return
        self._resolved = True
        if self.printer._handle is not None:
            self.printer._handle.commit_speculative()

    def abort(self) -> None:
        """Close the region, discarding its output."""
        if self._resolved:
            return
        self._resolved = True
        if self.printer._handle is not None:
            self.printer._handle.abort_speculative()

    def __enter__(self) -> Speculation:
        return self

    def __exit__(self, *exc: object) -> None:
        self.abort()


class Document:
    """One pretty-printed document, laid out to ``max_width`` columns:
    write through :attr:`printer`, then render with :meth:`finish`. Use it
    to try out or test printing code outside a test case::

        document = hegel.Document(max_width=8)
        document.printer.seq("{", "}", [1, 2, 3], lambda p, n: p.repr(n))
        assert document.finish() == "{1,\\n 2,\\n 3}"
    """

    def __init__(self, max_width: int = DEFAULT_MAX_WIDTH) -> None:
        self.printer = PrettyPrinter(_native.Printer.new(max_width))
        """The :class:`PrettyPrinter` writing to this document."""

    def finish(self) -> str:
        """Render everything printed so far, splicing in deferred regions.
        Regions deferred from this document die once it has rendered."""
        return self.printer._value()


_SILENT_PRINTER = PrettyPrinter(None, _silent=True)


def pretty_repr(value: Any, max_width: int = DEFAULT_MAX_WIDTH) -> str:
    """``repr(value)`` as the engine lays it out, wrapped to fit within
    ``max_width`` columns (default 79): what :meth:`PrettyPrinter.repr`
    prints, rendered into a fresh :class:`Document`."""
    document = Document(max_width)
    document.printer.repr(value)
    return document.finish()


def pretty_print(value: Any, printer: PrettyPrinter) -> None:
    """Print ``repr(value)`` through ``printer``, reflowed to fit: the
    same as ``printer.repr(value)``, which is the preferred spelling."""
    printer.repr(value)
