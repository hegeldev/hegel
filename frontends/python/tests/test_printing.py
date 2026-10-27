from __future__ import annotations

import enum
import threading

import pytest

import hegel
from hegel import Document, NotPrintable, Pool, PrettyPrinter, pretty_repr
from hegel import generators as gs
from tests.helpers import failing, notes, run_test


def report(body, exception=ValueError, **settings):
    lines = notes(failing(body, exception, **settings))[0].splitlines()
    assert lines[0] == "Falsifying example:"
    return lines[1:]


def report_of(generator):
    def body(tc):
        tc.draw(generator)
        raise ValueError

    return report(body)


class Bare(gs.Generator):
    def do_draw(self, tc):
        return tc.draw_silent(gs.integers(0, 3))


class Coords(gs.Generator):
    def do_draw_and_print(self, tc, printer):
        with printer.group("Coord(", ")"):
            x = tc.draw_and_print(gs.integers(0, 9), printer)
            printer.separator()
            y = tc.draw_and_print(gs.integers(0, 9), printer)
        return (x, y)


class Recorder:
    def __init__(self, tc, printer):
        self._tc = tc
        self._printer = printer
        self._first = True

    def next(self):
        value = self._tc.draw_silent(gs.integers(0, 10))
        if not self._first:
            self._printer.separator()
        self._first = False
        self._printer.repr(value)
        return value


class Recorders(gs.Generator):
    def do_draw_and_print(self, tc, printer):
        printer.begin_group(len("Recorder(["), "Recorder([")
        slot = printer.deferred()
        printer.end_group("])")
        return Recorder(tc, slot)


ANGLED = gs.integers(0, 3).print_with(lambda value, printer: printer.text(f"<{value}>"))


def test_custom_printed_elements_print_inside_their_containers():
    assert report_of(gs.lists(ANGLED, min_size=3, max_size=3)) == ["draw_1 = [<0>, <0>, <0>]"]
    assert report_of(gs.lists(Coords(), min_size=1, max_size=1)) == ["draw_1 = [Coord(0, 0)]"]
    assert report_of(gs.tuples(ANGLED, Coords())) == ["draw_1 = (<0>, Coord(0, 0))"]
    assert report_of(gs.dicts(ANGLED, Coords(), min_size=1, max_size=1)) == [
        "draw_1 = {<0>: Coord(0, 0)}"
    ]
    assert report_of(gs.fixed_dicts({"a": ANGLED})) == ["draw_1 = {'a': <0>}"]
    assert report_of(gs.one_of(ANGLED, Coords())) == ["draw_1 = <0>"]
    assert report_of(gs.just(0).flat_map(lambda _: Coords())) == ["draw_1 = Coord(0, 0)"]
    assert report_of(Coords().filter(lambda c: c[0] == 0)) == ["draw_1 = Coord(0, 0)"]
    assert report_of(gs.recursive(ANGLED, lambda inner: gs.lists(inner, max_size=1))) == [
        "draw_1 = <0>"
    ]


def test_optional_prints_its_inner_generator():
    def body(tc):
        value = tc.draw(gs.optional(ANGLED))
        if value is not None:
            raise ValueError

    assert report(body) == ["value = <0>"]
    assert report_of(gs.optional(ANGLED)) == ["draw_1 = None"]


class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y


def test_builds_prints_its_arguments_with_their_generators():
    assert report_of(gs.builds(Point, ANGLED, y=Coords())) == ["draw_1 = Point(<0>, y=Coord(0, 0))"]


def test_deferred_values_print_where_they_were_drawn():
    def body(tc):
        r = tc.draw(Recorders())
        y = tc.draw(gs.integers(0, 0))
        r.next()
        r.next()
        raise ValueError(y)

    assert report(body) == ["r = Recorder([0, 0])", "y = 0"]

    def in_a_list(tc):
        rs = tc.draw(gs.lists(Recorders(), min_size=2, max_size=2))
        rs[0].next()
        raise ValueError

    assert report(in_a_list) == ["rs = [Recorder([0]), Recorder([])]"]


def test_deferred_values_drawn_silently_record_nothing():
    def body(tc):
        recorder = tc.draw_silent(Recorders())
        assert not recorder._printer.should_print
        recorder.next()

    run_test(body, test_cases=3)


def test_map_prints_the_finished_value():
    assert report_of(gs.integers(0, 0).map(lambda n: [n, n])) == ["draw_1 = [0, 0]"]
    source = gs.integers(0, 0).print_with(lambda value, printer: printer.text("SRC"))
    assert report_of(source.map(str)) == ["draw_1 = '0'"]
    assert report_of(source.map(str).print_as_call("str")) == ["draw_1 = str(SRC)"]

    @hegel.composite
    def wrapped(tc):
        return [tc.draw(ANGLED)]

    assert report_of(wrapped()) == ["draw_1 = [0]"]


def test_layout_matches_the_rust_library():
    assert report_of(gs.lists(gs.just("a" * 20), min_size=3, max_size=3)) == [
        "draw_1 = ['aaaaaaaaaaaaaaaaaaaa',",
        " 'aaaaaaaaaaaaaaaaaaaa',",
        " 'aaaaaaaaaaaaaaaaaaaa']",
    ]
    assert report_of(gs.tuples(gs.booleans(), gs.integers())) == ["draw_1 = (False, 0)"]
    assert report_of(gs.optional(gs.integers())) == ["draw_1 = None"]
    assert report_of(gs.one_of(gs.just(5), gs.just(7))) == ["draw_1 = 5"]
    assert report_of(gs.sets(gs.sampled_from([1, 2, 3]), min_size=1)) == ["draw_1 = {1}"]
    assert report_of(gs.dicts(gs.text(max_size=2), gs.booleans(), min_size=1)) == [
        "draw_1 = {'': False}"
    ]
    assert report_of(gs.integers(0, 100).filter(lambda n: n > 50)) == ["draw_1 = 51"]
    wide = gs.lists(gs.just("b" * 30), min_size=3, max_size=3).map(tuple)
    assert report_of(wide.print_as_call("make")) == [
        "draw_1 = make(['bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',",
        "      'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',",
        "      'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'])",
    ]


class Colour(enum.Enum):
    RED = 1


class Perm(enum.Flag):
    R = 1
    W = 2


BUILT_INS = [
    gs.integers(),
    gs.floats(),
    gs.booleans(),
    gs.text(),
    gs.characters(),
    gs.from_regex("a+", fullmatch=True),
    gs.binary(),
    gs.emails(),
    gs.urls(),
    gs.domains(),
    gs.ip_addresses(),
    gs.uuids(),
    gs.from_lark('start: "x"'),
    gs.dates(),
    gs.times(),
    gs.datetimes(),
    gs.timedeltas(),
    gs.timezones(),
    gs.just(object),
    gs.none(),
    gs.sampled_from([1, 2]),
    gs.sampled_from([1, 2]).filter(lambda n: n > 1),
    gs.sampled_from(Colour),
    gs.sampled_from(Perm),
    gs.subsequences([1, 2]),
    gs.permutations([1, 2]),
    gs.samples([1, 2], min_size=1),
    gs.from_type(int),
    gs.from_type(list[str]),
    gs.from_type(complex),
    gs.iterables(gs.integers()),
]


@pytest.mark.parametrize("generator", BUILT_INS, ids=repr)
def test_every_built_in_prints_its_value(generator):
    seen = []

    def body(tc):
        x = tc.draw(generator)
        seen.append(x)
        raise ValueError

    assert report(body) == [f"x = {pretty_repr(seen[-1])}"]


def test_pool_values_print_their_value():
    def body(tc):
        pool = Pool(tc)
        pool.add("v", tc)
        reused = tc.draw(pool.values_reusable())
        consumed = tc.draw(pool.values_consumed().filter(lambda v: v == "v"))
        raise ValueError(reused, consumed)

    assert report(body) == ["reused = 'v'", "consumed = 'v'"]


def test_a_draw_only_generator_is_not_printable():
    calls = []

    def body(tc):
        calls.append(1)
        tc.draw(Bare())

    with pytest.raises(NotPrintable) as info:
        run_test(body)
    assert calls == [1]
    message = str(info.value)
    assert message.startswith("Bare() cannot print the values it draws")
    for fix in ("do_draw_and_print", "print_with", "print_as_repr", "draw_silent"):
        assert fix in message
    assert isinstance(info.value, hegel.InvalidArgument)


@pytest.mark.parametrize(
    "generator",
    [
        gs.lists(Bare(), min_size=1),
        gs.integers().flat_map(lambda _: Bare()),
        gs.tuples(Bare()),
        gs.optional(Bare()).filter(lambda value: value is not None),
    ],
    ids=repr,
)
def test_structures_of_a_draw_only_generator_are_not_printable(generator):
    def body(tc):
        tc.draw(generator)

    with pytest.raises(NotPrintable):
        run_test(body)


def test_nested_draws_need_a_printable_generator():
    @hegel.composite
    def outer(tc):
        return tc.draw(Bare())

    def body(tc):
        tc.draw(outer())

    with pytest.raises(NotPrintable):
        run_test(body)


def test_draw_only_generators_can_be_drawn_silently_or_made_printable():
    def silent(tc):
        assert 0 <= tc.draw_silent(Bare()) <= 3
        assert len(tc.draw_silent(gs.lists(Bare(), max_size=2))) <= 2

    run_test(silent, test_cases=5)

    assert report_of(Bare().print_as_repr()) == ["draw_1 = 0"]
    assert repr(Bare().print_as_repr()) == "Bare().print_as_repr()"
    assert report_of(Bare().print_with(lambda value, printer: printer.text("bare"))) == [
        "draw_1 = bare"
    ]
    assert report_of(Bare().map(lambda n: n + 1)) == ["draw_1 = 1"]


class DeferringToBare(gs.Generator):
    def do_draw_and_print(self, tc, printer):
        return tc.draw_and_print(Bare(), printer.deferred())


def test_silence_reaches_deferred_regions_of_a_silent_draw():
    def body(tc):
        assert 0 <= tc.draw_silent(DeferringToBare()) <= 3

    run_test(body, test_cases=5)


def test_a_no_op_printer_still_needs_a_printable_generator():
    def body(tc):
        tc.draw_and_print(Bare(), PrettyPrinter.noop())

    with pytest.raises(NotPrintable):
        run_test(body)


def test_print_only_generators_draw_silently_with_the_same_choices():
    def printed(tc):
        x, y = tc.draw(Coords())
        if x + y >= 5:
            raise ValueError

    def silent(tc):
        x, y = tc.draw_silent(Coords())
        tc.note(f"{(x, y)}")
        if x + y >= 5:
            raise ValueError

    assert report(printed) == ["draw_1 = Coord(0, 5)"]
    assert report(silent) == ["(0, 5)"]


def test_a_generator_implementing_neither_method():
    class Empty(gs.Generator):
        pass

    def body(tc):
        tc.draw_silent(Empty())

    with pytest.raises(NotImplementedError, match="Empty must implement"):
        run_test(body)


def test_value_printing_base_needs_do_draw():
    from hegel.generators.base import _PrintsValue

    class Half(_PrintsValue):
        pass

    def body(tc):
        tc.draw(Half())

    with pytest.raises(NotImplementedError, match="Half must implement"):
        run_test(body)


def test_explicit_test_cases_accept_draw_only_generators():
    @hegel.explicit_test_case(b=7)
    @hegel.test(test_cases=1)
    def check(tc):
        b = tc.draw(Bare())
        if b == 7:
            raise ValueError

    with pytest.raises(ValueError) as info:
        check()
    assert "b = 7" in "\n".join(notes(info.value)).splitlines()


def test_a_failed_draw_reports_its_nested_draws_as_printed():
    @hegel.composite
    def broken(tc):
        tc.draw(gs.integers(0, 0).print_with(lambda value, printer: printer.text("<zero>")))
        tc.draw(gs.integers(0, 0), name="named")
        raise KeyError

    def body(tc):
        tc.draw(broken())

    lines = report(body, KeyError)
    assert lines == [
        "draw_1 = <KeyError raised while drawing from broken()>",
        "  draw = <zero>",
        "  named = 0",
    ]


def test_deferred_regions_on_threads():
    document = Document()
    printer = document.printer
    printer.text("[")
    slot = printer.deferred()
    printer.text("]")
    thread = threading.Thread(target=lambda: slot.text("from a thread"))
    thread.start()
    thread.join()
    assert document.finish() == "[from a thread]"
    assert not slot.should_print
    slot.text("late")


def test_deferred_regions_die_with_their_speculation():
    document = Document()
    printer = document.printer
    with printer.speculate():
        slot = printer.deferred()
    assert not slot.should_print
    slot.text("gone")
    dead = slot.deferred()
    assert not dead.should_print
    assert document.finish() == ""
    assert not PrettyPrinter.noop().deferred().should_print
