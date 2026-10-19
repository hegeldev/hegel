from __future__ import annotations

import datetime as dt
import uuid

import pytest

import hegel
from hegel import _native
from hegel import generators as gs
from hegel._draw_names import DrawName
from hegel.errors import AssumeFailed, InternalError, LoopDone
from hegel.pretty import PrettyPrinter
from hegel.test_case import Collection, TestCase
from tests.helpers import failing, notes, run_test


def test_draw_reports_values_with_inferred_names():
    def body(tc):
        xs = tc.draw(gs.lists(gs.integers()))
        ys = tc.draw(gs.integers(), name="named")
        tc.draw(gs.just(5))
        assert not (xs == [0, 0] or len(xs) > 1), (xs, ys)

    error = failing(body)
    falsifying = notes(error)[0]
    assert falsifying.startswith("Falsifying example:\nxs = [0, 0]\n")
    assert "\nnamed_1 = " in falsifying
    assert "\ndraw_1 = 5" in falsifying


def test_repeated_names_get_suffixes():
    def body(tc):
        for _ in range(3):
            x = tc.draw(gs.just(1))
        x_2 = tc.draw(gs.just(2))
        assert x is None or x_2 is None

    lines = notes(failing(body))[0].splitlines()
    assert lines[1:] == ["x_1 = 1", "x_2 = 1", "x_3 = 1", "x_2_1 = 2"]


def test_a_name_drawn_more_often_than_inferred_is_numbered():
    def draw_x(tc):
        x = tc.draw(gs.just(1))
        return x

    def body(tc):
        draw_x(tc)
        draw_x(tc)
        x_2 = tc.draw(gs.just(2), name="x_2")
        raise AssertionError(x_2)

    lines = notes(failing(body))[0].splitlines()
    assert lines[1:] == ["x = 1", "x_2 = 1", "x_2_1 = 2"]


def test_nested_draws_are_silent():
    class Pair(gs.Generator):
        def do_draw(self, tc):
            a = tc.draw(gs.just(1))
            b = tc.draw(gs.just(2))
            return (a, b)

    def body(tc):
        pair = tc.draw(Pair())
        raise ValueError(pair)

    lines = notes(failing(body, ValueError))[0].splitlines()
    assert lines == ["Falsifying example:", "pair = (1, 2)"]


def test_draw_silent_and_generate_alias():
    def body(tc):
        a = tc.draw_silent(gs.integers(0, 3))
        b = tc.draw_silent(gs.integers(0, 3))
        assert 0 <= a <= 3 and 0 <= b <= 3
        raise KeyError

    error = failing(body, KeyError)
    assert notes(error) == []


def test_note_assume_reject(output):
    seen = []

    def body(tc):
        x = tc.draw(gs.integers(0, 10))
        tc.assume(x % 2 == 0)
        tc.note(f"x is {x}")
        seen.append(x)
        if x >= 4:
            tc.reject()
        assert x < 2

    error = failing(body)
    assert all(x % 2 == 0 for x in seen)
    assert notes(error)[0] == "Falsifying example:\nx = 2\nx is 2"


def test_target_event_and_event_value(output):
    def body(tc):
        x = tc.draw(gs.integers(0, 1000))
        tc.target(float(x), "size")
        tc.target(1.0)
        tc.event(f"parity-{x % 2}")
        tc.event_value("x", x)

    run_test(body, show_statistics=True, test_cases=20)


def test_repeat_runs_body_and_reports_repetitions():
    counts = []

    def body(tc):
        n = 0

        def step():
            nonlocal n
            n += 1
            x = tc.draw(gs.integers(0, 5))
            assert x != 3 or n < 2, n

        tc.repeat(step)
        counts.append(n)  # unreachable: repeat raises LoopDone

    error = failing(body)
    assert counts == []
    text = notes(error)[0]
    assert "// Repetition #1" in text
    assert "// Repetition #2" in text
    assert "  x = " in text


def test_repeat_with_assumption_and_control_signals():
    def body(tc):
        calls = 0

        def step():
            nonlocal calls
            calls += 1
            tc.draw(gs.integers(0, 1))
            # A failed assumption ends only this repetition, not the case.
            tc.assume(calls > 1)
            raise LoopDone()

        tc.repeat(step)

    run_test(body, test_cases=20)

    def invalid(tc):
        def step():
            raise hegel.InvalidArgument("boom")

        tc.repeat(step)

    failing(invalid, hegel.InvalidArgument, test_cases=2)


def test_repeat_can_run_zero_times(monkeypatch):
    # A body that draws nothing is stopped only by the test case's choice
    # limit, about a million forced repetitions, which is far too slow under
    # coverage on CI; have the collection stop at once instead.
    monkeypatch.setattr(Collection, "more", lambda self: False)

    def body(tc):
        tc.repeat(lambda: None)

    run_test(body, test_cases=3)


def test_child_and_clone_handles():
    def body(tc):
        child = tc._child(4)
        a = child.draw(gs.just("a"))
        child.note("inside")
        clone = tc._clone()
        clone._set_worker(0)
        b = clone.draw(gs.just("b"))
        assert repr(tc) == "TestCase()"
        raise ValueError(a + b)

    error = failing(body, ValueError)
    text = notes(error)[0]
    assert "    a = 'a'" in text
    assert "    inside" in text
    assert "b = 'b'" in text


def test_span_bookkeeping():
    def body(tc):
        assert tc._open_span_depth == 0
        tc.start_span(7)
        assert tc._open_span_depth == 1
        tc.stop_span()
        with pytest.raises(InternalError):
            tc.stop_span()
        tc.start_span(1)
        tc.start_span(2)
        tc._reset_open_spans_to(0)
        assert tc._open_span_depth == 0
        with pytest.raises(InternalError):
            tc._reset_open_spans_to(5)
        with pytest.raises(OverflowError):
            tc.start_span(-1)
        assert tc._open_span_depth == 0

    run_test(body, test_cases=1)


def test_primitive_draws():
    def body(tc):
        assert tc._generate_boolean(1.0) is True
        assert tc._generate_boolean(0.0) is False
        assert tc._generate_boolean(0.5, forced=True) is True
        assert tc._generate_integer(3, 3) == 3
        big = tc._generate_integer(-(10**30), 10**30)
        assert -(10**30) <= big <= 10**30
        f = tc._generate_float(64, 0.0, 1.0, False, False, False, False, 5e-324)
        assert 0.0 <= f <= 1.0
        assert len(tc._generate_bytes(2, 2)) == 2
        s = tc._generate_string(
            _native.StringGenerator.text(1, 1, "ascii", 0, None, None, None, None, None)
        )
        assert len(s) == 1
        d = tc._generate_date(dt.date(2000, 1, 1), dt.date(2000, 1, 2))
        assert d in (dt.date(2000, 1, 1), dt.date(2000, 1, 2))
        t = tc._generate_time(dt.time(1, 2, 3, 4), dt.time(1, 2, 3, 4))
        assert t == dt.time(1, 2, 3, 4)
        when = tc._generate_datetime(
            dt.datetime(2000, 1, 1, 1, 2, 3, 4), dt.datetime(2000, 1, 1, 1, 2, 3, 4)
        )
        assert when == dt.datetime(2000, 1, 1, 1, 2, 3, 4)
        assert uuid.UUID(bytes=tc._generate_uuid(4)).version == 4
        assert isinstance(uuid.UUID(bytes=tc._generate_uuid(None)), uuid.UUID)
        assert len(tc._generate_ipv4()) == 4
        assert len(tc._generate_ipv6()) == 16
        with pytest.raises(hegel.InvalidArgument):
            tc._generate_integer(2, 1)

    run_test(body, test_cases=20)


def test_collection_protocol():
    def body(tc):
        collection = Collection(tc, 2, 4)
        n = 0
        while collection.more():
            n += 1
            if n == 1:
                collection.reject("drop the first")
        assert 2 <= n <= 5
        assert collection.more() is False
        collection.reject()  # finished: no-op
        empty = Collection(tc, 0, 0)
        assert empty.more() is False

    run_test(body, test_cases=10)


def test_collection_more_propagates_errors():
    def body(tc):
        collection = Collection(tc, 5, 1)
        with pytest.raises(hegel.InvalidArgument):
            collection.more()

    run_test(body, test_cases=1)


def test_test_case_without_emit_discards_output():
    def body(tc):
        tc.note("never shown")
        assert tc._global.sink("x") is None
        assert tc._allocate_display_name(DrawName("x", False)) is None
        tc._emit_rendered_output()

    run_test(body, verbosity=hegel.Verbosity.quiet, test_cases=1)


def test_with_printer_is_cached_and_reset_by_blocks():
    def body(tc):
        printer = tc._with_printer()
        assert tc._with_printer() is printer
        assert isinstance(printer, PrettyPrinter)
        previous = tc._enter_block(2)
        assert tc._with_printer() is not printer
        tc._restore_handle(previous)
        raise ValueError

    failing(body, ValueError, test_cases=1)


def test_draw_and_print_runs_inside_span():
    def body(tc):
        printer = tc._with_printer()
        value = tc.draw_and_print(gs.just([1, 2]), printer)
        assert value == [1, 2]
        assert tc._open_span_depth == 0
        raise ValueError

    error = failing(body, ValueError, test_cases=1)
    assert "[1, 2]" in notes(error)[0]


def test_assume_failed_is_base_exception():
    assert issubclass(AssumeFailed, BaseException)
    assert not issubclass(AssumeFailed, Exception)
    assert isinstance(TestCase.__test__, bool)


def test_draws_nested_too_deep_abandon_the_case():
    from hegel.test_case import MAX_DRAW_DEPTH

    def body(tc):
        for _ in range(MAX_DRAW_DEPTH + 1):
            tc.start_span(1)

    with pytest.raises(hegel.errors.FailedHealthCheck):
        run_test(body, test_cases=10)


def test_numbered_names_skip_names_already_taken():
    def body(tc):
        x_1 = tc.draw(gs.just(0))
        for _ in range(2):
            x = tc.draw(gs.just(1))
        raise AssertionError((x_1, x))

    lines = notes(failing(body))[0].splitlines()
    assert lines[1:] == ["x_1 = 0", "x_2 = 1", "x_3 = 1"]


def test_per_case_state_is_shared_by_every_handle_onto_a_test_case():
    seen = []

    def body(tc):
        store = tc._per_case("key", list)
        assert tc._clone()._per_case("key", list) is store
        assert tc._child()._per_case("key", dict) is store
        seen.append(store)

    run_test(body, test_cases=2, database="")
    assert len(seen) == 2 and seen[0] is not seen[1]


def test_a_draw_that_raises_reports_what_was_drawn_inside_it():
    @gs.composite
    def failing_composite(tc):
        inner = tc.draw(gs.integers(0, 10))
        tc.draw(gs.booleans())
        tc.draw(gs.just(1), name="named")
        if inner > 5:
            raise KeyError(inner)
        return inner

    def body(tc):
        tc.draw(gs.integers(0, 3), name="first")
        tc.draw(failing_composite())

    lines = notes(failing(body, KeyError))[0].splitlines()
    assert lines == [
        "Falsifying example:",
        "first_1 = 0",
        "draw_1 = <KeyError raised while drawing from failing_composite()>",
        "  inner = 6",
        "  draw = False",
        "  named = 1",
    ]


def test_a_rejected_draw_reports_nothing():
    def body(tc):
        tc.draw(gs.integers(0, 10).filter(lambda x: x > 100))

    with pytest.raises(hegel.FailedHealthCheck):
        hegel.run(body, database="")


@pytest.mark.parametrize("score", ["1", None, [0.0], True])
def test_target_rejects_a_non_numeric_score(score):
    def body(tc):
        tc.draw(gs.none())
        with pytest.raises(hegel.InvalidArgument, match="must be an int or float"):
            tc.target(score, label="score")
        with pytest.raises(hegel.InvalidArgument, match="must be an int or float"):
            tc.event_value("value", score)

    run_test(body, test_cases=1)


@pytest.mark.parametrize("label", [["a"], 1, None])
def test_target_rejects_a_non_string_label(label):
    def body(tc):
        tc.draw(gs.none())
        with pytest.raises(hegel.InvalidArgument, match="must be a string"):
            tc.target(0.0, label=label)

    run_test(body, test_cases=1)
