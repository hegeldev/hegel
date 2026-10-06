"""Pretty printing of drawn values, laid out by the engine's printer.

Every printed draw is rendered through a :class:`PrettyPrinter` onto the
test case's document, so large values wrap at the configured width. Values
print with :func:`pretty_print`: containers structurally, dataclasses by
field, objects defining ``__hegel_pretty__`` through that method, and
everything else by ``repr``.
"""

from __future__ import annotations

import dataclasses
import enum
from collections.abc import Callable
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
    "register_pretty_printer",
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
    """``value`` as :func:`pretty_print` lays it out, within ``max_width``."""
    document = Document(max_width)
    pretty_print(value, document.printer)
    return document.finish()


def _print_sequence(
    items: Any, printer: PrettyPrinter, open: str, close: str, trailing: str = ""
) -> None:
    printer.begin_group(len(open), open)
    for index, item in enumerate(items):
        if index > 0:
            printer.text(",")
            printer.breakable(" ")
        pretty_print(item, printer)
    if trailing:
        printer.text(trailing)
    printer.end_group(close)


def _print_mapping(items: Any, printer: PrettyPrinter, open: str, close: str) -> None:
    printer.begin_group(len(open), open)
    for index, (key, value) in enumerate(items):
        if index > 0:
            printer.text(",")
            printer.breakable(" ")
        pretty_print(key, printer)
        printer.text(": ")
        pretty_print(value, printer)
    printer.end_group(close)


def _print_call(name: str, kwargs: list[tuple[str, Any]], printer: PrettyPrinter) -> None:
    printer.begin_group(len(name) + 1, name + "(")
    for index, (key, value) in enumerate(kwargs):
        if index > 0:
            printer.text(",")
            printer.breakable(" ")
        printer.text(key + "=")
        pretty_print(value, printer)
    printer.end_group(")")


_PRINTERS: dict[type, Callable[[Any, PrettyPrinter], None]] = {}


def register_pretty_printer(cls: type, printer: Callable[[Any, PrettyPrinter], None]) -> None:
    """Register how instances of ``cls`` (exactly, not subclasses) print."""
    _PRINTERS[cls] = printer


def pretty_print(value: Any, printer: PrettyPrinter) -> None:
    """Print ``value`` through ``printer`` as a Python expression."""
    if not printer.should_print:
        return
    custom = getattr(type(value), "__hegel_pretty__", None)
    if custom is not None:
        custom(value, printer)
        return
    cls = type(value)
    registered = _PRINTERS.get(cls)
    if registered is not None:
        registered(value, printer)
        return
    if cls is list:
        _print_sequence(value, printer, "[", "]")
    elif cls is tuple:
        _print_sequence(value, printer, "(", ")", trailing="," if len(value) == 1 else "")
    elif cls is set:
        if value:
            _print_sequence(_sorted_if_possible(value), printer, "{", "}")
        else:
            printer.text("set()")
    elif cls is frozenset:
        if value:
            _print_sequence(_sorted_if_possible(value), printer, "frozenset({", "})")
        else:
            printer.text("frozenset()")
    elif cls is dict:
        _print_mapping(value.items(), printer, "{", "}")
    elif dataclasses.is_dataclass(value) and not isinstance(value, type):
        fields = [
            (f.name, getattr(value, f.name)) for f in dataclasses.fields(value) if f.repr and f.init
        ]
        _print_call(cls.__qualname__, fields, printer)
    elif isinstance(value, enum.Enum):
        printer.text(f"{cls.__qualname__}.{value.name}")
    else:
        printer.text(repr(value))


def _sorted_if_possible(items: Any) -> list[Any]:
    try:
        return sorted(items)
    except TypeError:
        return list(items)
