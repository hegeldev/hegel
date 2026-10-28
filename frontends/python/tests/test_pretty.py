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
)


class Colour(enum.Enum):
    red = 1


@dataclasses.dataclass
class Point:
    x: int
    y: int
    hidden: int = dataclasses.field(default=0, repr=False)


class Custom:
    def __repr__(self) -> str:
        return "<custom>"


def test_pretty_repr_is_repr_when_it_fits():
    for value in (
        [1, 2, 3],
        (1,),
        (1, 2),
        set(),
        {3, 1, 2},
        frozenset(),
        frozenset([2, 1]),
        {"a": [1], "b": None},
        "text\nwith newline",
        Colour.red,
        Point(1, 2, 3),
        Custom(),
        Point,
    ):
        assert pretty_repr(value) == repr(value)


def test_wrapping_at_width():
    value = list(range(40))
    rendered = pretty_repr(value, max_width=30)
    assert len(rendered.splitlines()) > 1
    assert all(len(line) <= 30 for line in rendered.splitlines())
    assert eval(rendered) == value
    assert len(pretty_repr(value).splitlines()) > 1
    assert pretty_repr(list(range(5)), max_width=DEFAULT_MAX_WIDTH) == "[0, 1, 2, 3, 4]"


def test_wrapping_follows_repr_structure():
    value = {"point": Point(1, 2), "items": list(range(30)), "tags": {"a", "b"}}
    rendered = pretty_repr(value, max_width=40)
    assert all(len(line) <= 40 for line in rendered.splitlines())
    assert eval(rendered, {"Point": Point}) == value
    assert "Point(x=1, y=2)" in rendered


def test_reflow_of_unparseable_text_is_verbatim():
    class Odd:
        def __repr__(self) -> str:
            return "<(unbalanced [brackets>"

    assert pretty_repr(Odd()) == "<(unbalanced [brackets>"
    document = Document(20)
    document.printer.reflow("two\nlines")
    assert document.finish() == "two\nlines"


def test_noop_printer_accepts_everything():
    printer = PrettyPrinter.noop()
    assert not printer.should_print
    printer.text("a\nb")
    printer.reflow("[1]")
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
    assert printer._value() == ""
    pretty_print([1], printer)


def test_document_operations():
    document = Document(20)
    printer = document.printer
    assert printer.should_print
    printer.text("line one\nline two")
    printer.text("\n\n")
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
    assert PrettyPrinter(handle)._value() == "before middle after"


class CountedRepr:
    def __init__(self) -> None:
        self.calls = 0

    def __repr__(self) -> str:
        self.calls += 1
        return "counted"


def test_repr_prints_through_the_reflower():
    document = Document(30)
    document.printer.repr(list(range(40)))
    rendered = document.finish()
    assert eval(rendered) == list(range(40))
    assert all(len(line) <= 30 for line in rendered.splitlines())

    counted = CountedRepr()
    PrettyPrinter.noop().repr(counted)
    pretty_print(counted, PrettyPrinter.noop())
    assert counted.calls == 0
    document = Document()
    pretty_print(counted, document.printer)
    assert document.finish() == "counted"


def test_group_defaults_its_indent_to_the_opening_text():
    document = Document(12)
    with document.printer.group("Some(", ")"):
        document.printer.seq("[", "]", ["first", "second"], lambda p, item: p.text(item))
    assert document.finish() == "Some([first,\n      second])"

    document = Document(8)
    with document.printer.group("f(", ")", indent=1):
        document.printer.text("aaaa")
        document.printer.separator()
        document.printer.text("bbbb")
    assert document.finish() == "f(aaaa,\n bbbb)"


def test_group_closes_when_its_body_raises():
    document = Document()
    try:
        with document.printer.group("(", ")"):
            document.printer.text("x")
            raise KeyError
    except KeyError:
        pass
    assert document.finish() == "(x)"


def test_seq_lays_out_inline_or_one_item_per_line():
    document = Document(8)
    document.printer.seq("{", "}", [1, 2, 3], lambda p, n: p.repr(n))
    assert document.finish() == "{1,\n 2,\n 3}"
    document = Document()
    document.printer.seq("{", "}", [1, 2, 3], lambda p, n: p.repr(n))
    assert document.finish() == "{1, 2, 3}"
    document = Document()
    document.printer.seq("[", "]", [], lambda p, n: p.repr(n))
    assert document.finish() == "[]"


def test_comment_uses_python_comment_syntax_and_breaks_its_group():
    document = Document()
    printer = document.printer
    printer.text("x")
    printer.comment("note")
    assert document.finish() == "x  # note"

    document = Document()
    printer = document.printer
    with printer.group("[", "]"):
        printer.text("1")
        printer.separator()
        printer.text("2")
        printer.comment("two")
    assert document.finish() == "[1,\n 2  # two\n]"


def test_printers_have_no_public_rendering():
    assert not hasattr(PrettyPrinter.noop(), "value")
