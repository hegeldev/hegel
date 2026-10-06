from __future__ import annotations

import enum

import pytest

import hegel
from hegel import generators as gs
from hegel.generators.base import Filtered, FlatMapped, Generator, Mapped
from hegel.generators.combinators import Nothing, OneOf, as_generator
from hegel.pretty import PrettyPrinter
from tests.helpers import failing, find_example, notes, run_test


class Colour(enum.Enum):
    red = 1
    green = 2


class Perm(enum.Flag):
    read = 1
    write = 2
    exec_ = 4


class Empty(enum.Flag):
    pass


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


def test_just_and_none():
    check_all(gs.just([1]), lambda v: v == [1])
    check_all(gs.none(), lambda v: v is None)
    assert repr(gs.just(3)) == "just(3)"


def test_nothing_rejects():
    seen = []

    def body(tc):
        seen.append(tc.draw(gs.nothing()))

    failing(body, hegel.Unsatisfiable, test_cases=5)
    assert seen == []
    assert repr(gs.nothing()) == "nothing()"


def test_sampled_from():
    check_all(gs.sampled_from([1, 2, 3]), lambda v: v in (1, 2, 3))
    check_all(gs.sampled_from("ab"), lambda v: v in "ab")
    check_all(gs.sampled_from(range(3)), lambda v: v in range(3))
    check_all(gs.sampled_from(Colour), lambda v: isinstance(v, Colour))
    check_all(gs.sampled_from(Perm), lambda v: isinstance(v, Perm) and v.value != 0)
    assert (
        find_example(gs.sampled_from(Perm), lambda p: p == Perm.read | Perm.write)
        == Perm.read | Perm.write
    )
    assert repr(gs.sampled_from((1, 2))) == "sampled_from([1, 2])"
    assert find_example(gs.sampled_from([3, 2, 1])) == 3
    with pytest.raises(hegel.InvalidArgument):
        gs.sampled_from([])
    with pytest.raises(hegel.InvalidArgument):
        gs.sampled_from(Empty)
    with pytest.raises(hegel.InvalidArgument):
        gs.sampled_from({1, 2})


def test_one_of():
    check_all(gs.one_of(gs.just(1), gs.just("a")), lambda v: v in (1, "a"))
    check_all(gs.one_of([gs.just(1), gs.just(2)]), lambda v: v in (1, 2))
    check_all(gs.just(1) | gs.just(2) | gs.just(3), lambda v: v in (1, 2, 3))
    assert isinstance(gs.just(1) | gs.just(2) | gs.just(3), OneOf)
    assert len((gs.just(1) | gs.just(2) | gs.just(3)).generators) == 3
    assert gs.one_of(gs.just(1)) is not None and not isinstance(gs.one_of(gs.just(1)), OneOf)
    assert isinstance(gs.one_of(gs.nothing(), gs.nothing()), Nothing)
    assert repr(gs.one_of(gs.just(1), gs.just(2))) == "one_of(just(1), just(2))"
    assert find_example(gs.one_of(gs.just(2), gs.just(1))) == 2
    with pytest.raises(hegel.InvalidArgument):
        gs.one_of()
    with pytest.raises(hegel.InvalidArgument):
        gs.one_of(gs.just(1), 5)


def test_one_of_prints_chosen_branch():
    def body(tc):
        value = tc.draw(gs.one_of(gs.lists(gs.just(1), min_size=1, max_size=1), gs.just("x")))
        assert value != [1]

    assert notes(failing(body))[0] == "Falsifying example:\nvalue = [1]"


def test_optional():
    check_all(gs.optional(gs.integers()), lambda v: v is None or isinstance(v, int))
    assert repr(gs.optional(gs.just(1))) == "optional(just(1))"
    assert find_example(gs.optional(gs.integers())) is None
    assert find_example(gs.optional(gs.integers()), lambda v: v is not None) == 0

    def body(tc):
        value = tc.draw(gs.optional(gs.just([1])))
        assert value is None

    assert notes(failing(body))[0] == "Falsifying example:\nvalue = [1]"

    def body2(tc):
        value = tc.draw(gs.optional(gs.just([1])))
        assert value is not None

    assert notes(failing(body2))[0] == "Falsifying example:\nvalue = None"


def test_map_flat_map_filter():
    check_all(gs.integers().map(str), lambda v: isinstance(v, str))
    check_all(
        gs.integers(1, 3).flat_map(lambda n: gs.lists(gs.just(n), min_size=n, max_size=n)),
        lambda v: len(v) == v[0],
    )
    check_all(gs.integers().filter(lambda x: x % 2 == 0), lambda v: v % 2 == 0)
    mapped = gs.integers().map(str)
    assert isinstance(mapped, Mapped)
    assert repr(mapped) == "integers().map(str)"
    assert isinstance(gs.integers().flat_map(gs.just), FlatMapped)
    assert repr(gs.integers().flat_map(gs.just)) == "integers().flat_map(just)"
    assert isinstance(gs.integers().filter(bool), Filtered)
    assert repr(gs.integers().filter(bool)) == "integers().filter(bool)"
    assert "lambda" in repr(gs.integers().filter(lambda x: x))
    assert find_example(gs.integers().filter(lambda x: x > 5)) == 6


def test_filter_prints_only_accepted_value():
    def body(tc):
        x = tc.draw(gs.integers(0, 10).filter(lambda v: v >= 3))
        assert x == 100

    assert notes(failing(body))[0] == "Falsifying example:\nx = 3"


def test_filter_rejects_after_three_attempts():
    attempts = []

    def body(tc):
        tc.draw(gs.integers(0, 1).filter(lambda v: attempts.append(v) or False))

    run_test(body, test_cases=2, suppress_health_check=hegel.HealthCheck.all())
    assert len(attempts) >= 3


def test_filter_propagates_predicate_errors():
    def body(tc):
        tc.draw(gs.integers().filter(lambda v: 1 / 0))

    failing(body, ZeroDivisionError)


def test_flat_map_requires_generator():
    def body(tc):
        tc.draw(gs.integers().flat_map(lambda n: n))

    failing(body, hegel.InvalidArgument)
    with pytest.raises(hegel.InvalidArgument):
        as_generator("x")


def test_flat_map_prints_inner_value():
    def body(tc):
        v = tc.draw(gs.just(2).flat_map(lambda n: gs.lists(gs.just(n), min_size=n, max_size=n)))
        raise ValueError(v)

    assert notes(failing(body, ValueError))[0] == "Falsifying example:\nv = [2, 2]"


def test_print_with_and_print_as_call():
    def body(tc):
        a = tc.draw(gs.just(5).print_with(lambda value, printer: printer.text(f"<{value}>")))
        b = tc.draw(gs.just(5).map(lambda v: v * 2).print_as_call("double"))
        c = tc.draw_silent(gs.just(5).print_with(lambda value, printer: printer.text("never")))
        assert (a, b, c) == (5, 10, 5)
        raise ValueError

    text = notes(failing(body, ValueError))[0]
    assert "a = <5>" in text
    assert "b = double(5)" in text
    with pytest.raises(hegel.InvalidArgument):
        gs.just(1).print_as_call("f")
    with pytest.raises(hegel.InvalidArgument):
        gs.just(1).map(str).print_as_call("")


def test_generator_base_class():
    class Bare(Generator):
        pass

    with pytest.raises(NotImplementedError):
        Bare().do_draw(None)
    assert repr(Bare()) == "Bare()"
    assert Bare().label == gs.label_from_name("hegel.generator")

    printer = PrettyPrinter.noop()

    def body(tc):
        assert gs.just(1).do_draw_and_print(tc, printer) == 1

    run_test(body, test_cases=1)


def test_print_with_is_skipped_for_silent_printers():
    calls = []
    printed = gs.just(1).print_with(lambda value, printer: calls.append(value))

    def body(tc):
        assert printed.do_draw_and_print(tc, PrettyPrinter.noop()) == 1

    run_test(body, test_cases=1)
    assert calls == []


def test_chained_filters_combine_into_one():
    g = gs.integers(0, 100).filter(lambda x: x % 2 == 0).filter(lambda x: x % 3 == 0)
    assert isinstance(g, Filtered)
    assert not isinstance(g.source, Filtered)
    assert repr(g) == "integers(min_value=0, max_value=100).filter(<lambda>).filter(<lambda>)"
    assert find_example(g, lambda x: x > 0) == 6


def test_filtered_samples_narrow_the_elements():
    g = gs.sampled_from(range(60))
    for i in range(60):
        if i != 41:
            g = g.filter(lambda x, i=i: x != i)
    assert "filter" in repr(g)
    assert find_example(g) == 41
    assert find_example(gs.sampled_from(range(60)).filter(lambda x: x > 50)) == 51
    empty = gs.sampled_from([1, 2]).filter(lambda x: x > 5)
    with pytest.raises(hegel.InvalidArgument, match="Could not find"):
        find_example(empty)


def test_emptiness_propagates_through_wrappers_and_collections():
    empty = gs.nothing()
    assert empty.map(str).is_empty
    assert empty.map(str).print_as_call("f").is_empty
    assert empty.print_with(lambda value, printer: printer.text("x")).is_empty
    assert empty.flat_map(lambda v: gs.just(v)).is_empty
    assert empty.filter(bool).is_empty
    assert not gs.integers().filter(bool).is_empty
    assert gs.lists(empty, min_size=1).is_empty
    assert not gs.lists(empty).is_empty
    assert gs.sets(empty, min_size=1).is_empty
    assert gs.dicts(empty, gs.integers(), min_size=1).is_empty
    assert not gs.dicts(gs.integers(), empty).is_empty
    assert gs.fixed_dicts({"a": empty}).is_empty
    assert not gs.fixed_dicts({"a": gs.integers()}).is_empty
    assert gs.tuples(gs.integers(), empty).is_empty
    assert not gs.tuples().is_empty
