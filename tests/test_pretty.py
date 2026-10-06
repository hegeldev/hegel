from __future__ import annotations

import dataclasses
import enum

from hegel import _native
from hegel.pretty import (
    DEFAULT_MAX_WIDTH,
    Document,
    PrettyPrinter,
    Speculation,
    pretty_print,
    pretty_repr,
    register_pretty_printer,
)


class Colour(enum.Enum):
    red = 1


@dataclasses.dataclass
class Point:
    x: int
    y: int
    hidden: int = dataclasses.field(default=0, repr=False)


class Custom:
    def __hegel_pretty__(self, printer: PrettyPrinter) -> None:
        printer.text("<custom>")


class Registered:
    pass


def test_pretty_repr_of_builtins():
    assert pretty_repr([1, 2, 3]) == "[1, 2, 3]"
    assert pretty_repr((1,)) == "(1,)"
    assert pretty_repr((1, 2)) == "(1, 2)"
    assert pretty_repr(set()) == "set()"
    assert pretty_repr({3, 1, 2}) == "{1, 2, 3}"
    assert pretty_repr({1, "a"}) in ("{1, 'a'}", "{'a', 1}")
    assert pretty_repr(frozenset()) == "frozenset()"
    assert pretty_repr(frozenset([2, 1])) == "frozenset({1, 2})"
    assert pretty_repr({"a": [1], "b": None}) == "{'a': [1], 'b': None}"
    assert pretty_repr("text\nwith newline") == "'text\\nwith newline'"
    assert pretty_repr(Colour.red) == "Colour.red"
    assert pretty_repr(Point(1, 2, 3)) == "Point(x=1, y=2)"
    assert pretty_repr(Custom()) == "<custom>"
    assert pretty_repr(Point) == repr(Point)


def test_registered_printer_is_exact_type():
    register_pretty_printer(Registered, lambda value, printer: printer.text("REG"))

    class Sub(Registered):
        pass

    assert pretty_repr(Registered()) == "REG"
    assert pretty_repr(Sub()) == repr(Sub()) or "Sub" in pretty_repr(Sub())


def test_wrapping_at_width():
    value = list(range(40))
    rendered = pretty_repr(value, max_width=30)
    assert len(rendered.splitlines()) > 1
    assert all(len(line) <= 30 for line in rendered.splitlines())
    assert eval(rendered) == value
    assert len(pretty_repr(value).splitlines()) > 1
    assert pretty_repr(list(range(5)), max_width=DEFAULT_MAX_WIDTH) == "[0, 1, 2, 3, 4]"


def test_noop_printer_accepts_everything():
    printer = PrettyPrinter.noop()
    assert not printer.should_print
    printer.text("a\nb")
    printer.breakable()
    printer.if_break(",")
    printer.hard_break()
    printer.comment("c")
    printer.begin_group(1, "(")
    printer.end_group(")")
    printer.shift_indent(2)
    with printer.speculate() as speculation:
        speculation.commit()
    speculation.abort()
    assert printer.value() == ""
    pretty_print([1], printer)


def test_document_operations():
    document = Document(20)
    printer = document.printer
    assert printer.should_print
    printer.text("line one\nline two")
    printer.comment("note")
    printer.hard_break()
    printer.begin_group(2, "[")
    for i in range(10):
        if i:
            printer.text(",")
            printer.breakable(" ")
        printer.text(str(i))
    printer.if_break(",")
    printer.end_group("]")
    printer.shift_indent(0)
    rendered = document.finish()
    assert rendered.startswith("line one\nline two")
    assert "note" in rendered
    assert "9," in rendered  # if_break fired because the group broke
    assert all(len(line) <= 20 for line in rendered.splitlines())
    # printing after reading is a silent no-op
    printer.text("late")
    assert not printer.should_print


def test_speculation_commit_and_abort():
    document = Document()
    printer = document.printer
    with printer.speculate() as speculation:
        printer.text("kept")
        speculation.commit()
    with printer.speculate():
        printer.text("dropped")
    speculation = printer.speculate()
    printer.text("also dropped")
    speculation.abort()
    speculation.abort()  # idempotent
    speculation.commit()  # ignored after abort
    assert document.finish() == "kept"


def test_speculation_object():
    speculation = Speculation(PrettyPrinter.noop())
    assert speculation.__enter__() is speculation
    speculation.commit()
    speculation.commit()


def test_deferred_slots_are_resolved_on_read():
    handle = _native.Printer.new(79)
    handle.text("before ")
    slot = handle.deferred()
    assert slot is not None
    handle.text(" after")
    slot.text("middle")
    assert PrettyPrinter(handle).value() == "before middle after"
