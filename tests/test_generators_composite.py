from __future__ import annotations

import dataclasses
import inspect

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import failing, notes, run_test


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


@hegel.composite
def ordered_pair(tc, limit: int = 10):
    a = tc.draw(gs.integers(0, limit))
    b = tc.draw(gs.integers(a, limit))
    return (a, b)


@dataclasses.dataclass
class Point:
    x: int
    y: int = 0


class Widget:
    def __init__(self, name: str, size: int, *, colour: str = "red"):
        self.name = name
        self.size = size
        self.colour = colour


def test_composite():
    check_all(ordered_pair(), lambda p: p[0] <= p[1] <= 10)
    check_all(ordered_pair(3), lambda p: p[1] <= 3)
    check_all(ordered_pair(limit=2), lambda p: p[1] <= 2)
    assert list(inspect.signature(ordered_pair).parameters) == ["limit"]
    assert repr(ordered_pair(3)) == "ordered_pair(3)"
    assert repr(ordered_pair(limit=3)) == "ordered_pair(limit=3)"
    assert hegel.find_example(ordered_pair(), lambda p: p[1] > 0) == (0, 1)
    with pytest.raises(hegel.InvalidArgument):
        hegel.composite(5)
    with pytest.raises(hegel.InvalidArgument):
        hegel.composite(lambda: None)


def test_composite_labels_depend_on_generator_arguments():
    @hegel.composite
    def wrap(tc, inner):
        return tc.draw(inner)

    assert wrap(gs.integers()).label != wrap(gs.text()).label
    assert wrap(gs.integers()).label == wrap(gs.integers()).label


def test_composite_draws_are_reported_as_one_value():
    def body(tc):
        pair = tc.draw(ordered_pair())
        assert pair == (0, 0)

    assert notes(failing(body))[0] == "Falsifying example:\npair = (0, 1)"


def test_builds():
    check_all(gs.builds(Point, gs.integers(), gs.integers()), lambda p: isinstance(p, Point))
    check_all(gs.builds(Point, x=gs.just(1)), lambda p: p.x == 1 and p.y == 0)
    check_all(gs.builds(Point), lambda p: isinstance(p.x, int))
    check_all(gs.builds(Widget, gs.just("w")), lambda w: w.name == "w" and w.colour == "red")
    check_all(
        gs.builds(Widget, size=gs.just(3), colour=gs.just("blue")), lambda w: w.colour == "blue"
    )
    check_all(gs.builds(lambda: 5), lambda v: v == 5)
    assert repr(gs.builds(Point, gs.just(1), y=gs.just(2))) == "builds('Point', just(1), y=just(2))"
    assert hegel.find_example(gs.builds(Point)) == Point(0, 0)
    with pytest.raises(hegel.InvalidArgument):
        gs.builds(5)
    with pytest.raises(hegel.InvalidArgument):
        gs.builds(Point, 5)
    with pytest.raises(hegel.InvalidArgument):
        gs.builds(Point, x=5)


def test_builds_prints_as_call():
    def body(tc):
        p = tc.draw(gs.builds(Point, gs.just(1), y=gs.just(2)))
        w = tc.draw(gs.builds(Widget, gs.just("n"), gs.just(0)))
        assert isinstance(w, Widget)
        raise ValueError(p)

    text = notes(failing(body, ValueError))[0]
    assert "p = Point(1, y=2)" in text
    assert "w = Widget('n', 0)" in text


def test_builds_without_signature():
    assert hegel.find_example(gs.builds(int)) == 0
    check_all(gs.builds(dict), lambda d: d == {})
