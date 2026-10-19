"""Pretty printing of drawn values, laid out by the engine's printer.

Every printed draw is rendered through a :class:`PrettyPrinter` onto the
test case's document, so large values wrap at the configured width. A
value prints as its ``repr``, reflowed by the engine: it recovers the
bracket structure of the one-line representation and lays it out as if it
had been printed structurally, so there is nothing to register or
implement for a type beyond a sensible ``__repr__``. Generators that draw
component values print them structurally as they go
(:meth:`hegel.generators.Generator.do_draw_and_print`).
"""

from __future__ import annotations

from typing import Any

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


class PrettyPrinter:
    """A handle onto a region of an engine-side document.

    A no-op printer (``PrettyPrinter.noop()``) accepts every call and prints
    nothing, so a generator can run one code path for silent and printing
    draws.
    """

    __slots__ = ("_handle",)

    def __init__(self, handle: _native.Printer | None) -> None:
        self._handle = handle

    @classmethod
    def noop(cls) -> PrettyPrinter:
        return cls(None)

    @property
    def should_print(self) -> bool:
        """Whether output written now would appear anywhere: guard any work
        done purely for printing with this."""
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
        if self._handle is not None:
            self._handle.hard_break()

    def comment(self, text: str) -> None:
        """Attach a comment to the end of the current line."""
        if self._handle is not None:
            self._handle.comment(text)

    def begin_group(self, indent: int, open: str) -> None:
        """Open a group: emit ``open`` and indent break points inside it."""
        if self._handle is not None:
            self._handle.begin_group(indent, open)

    def end_group(self, close: str) -> None:
        if self._handle is not None:
            self._handle.end_group(close)

    def shift_indent(self, delta: int) -> None:
        if self._handle is not None:
            self._handle.shift_indent(delta)

    def speculate(self) -> Speculation:
        """Open a speculative region whose output is kept only if the
        returned :class:`Speculation` is committed."""
        if self._handle is not None:
            self._handle.begin_speculative()
        return Speculation(self)

    def value(self) -> str:
        """Flush and read everything printed so far."""
        if self._handle is None:
            return ""
        try:
            self._handle.resolve()
        except InvalidArgument:
            pass  # no deferred session outstanding
        return self._handle.value().decode("utf-8", "surrogatepass")


class Speculation:
    """A speculative print region; see :meth:`PrettyPrinter.speculate`.

    Usable as a context manager: leaving the block without a commit aborts
    the region, so an unwinding draw never leaves partial output behind.
    """

    __slots__ = ("_resolved", "printer")

    def __init__(self, printer: PrettyPrinter) -> None:
        self.printer = printer
        self._resolved = False

    def commit(self) -> None:
        if self._resolved:
            return
        self._resolved = True
        if self.printer._handle is not None:
            self.printer._handle.commit_speculative()

    def abort(self) -> None:
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
    """A standalone document for pretty printing outside a test case."""

    def __init__(self, max_width: int = DEFAULT_MAX_WIDTH) -> None:
        self.printer = PrettyPrinter(_native.Printer.new(max_width))

    def finish(self) -> str:
        return self.printer.value()


def pretty_repr(value: Any, max_width: int = DEFAULT_MAX_WIDTH) -> str:
    """``repr(value)`` as the engine lays it out, within ``max_width``."""
    document = Document(max_width)
    pretty_print(value, document.printer)
    return document.finish()


def pretty_print(value: Any, printer: PrettyPrinter) -> None:
    """Print ``repr(value)`` through ``printer``, reflowed to fit."""
    if printer.should_print:
        printer.reflow(repr(value))
