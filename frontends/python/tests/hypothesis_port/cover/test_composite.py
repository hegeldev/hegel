# Dropped from the Hypothesis original: test_errors_given_default_for_draw,
# test_errors_given_kwargs_only, test_drawfn_cannot_be_instantiated,
# test_warns_on_strategy_annotation (the DrawFn parameter);
# test_warning_given_no_drawfn_call (deprecation warnings); the
# composite-under-classmethod and composite-under-staticmethod cases of
# test_applying_composite_decorator_to_methods (Hypothesis's descriptor handling).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import typing

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import minimal
from tests.hypothesis_port.common.utils import flaky


@hegel.composite
def badly_draw_lists(tc, m=0):
    length = tc.draw(gs.integers(m, m + 10))
    return [tc.draw(gs.integers()) for _ in range(length)]


def test_simplify_draws():
    assert minimal(badly_draw_lists(), lambda x: len(x) >= 3) == [0] * 3


def test_can_pass_through_arguments():
    assert minimal(badly_draw_lists(5)) == [0] * 5
    assert minimal(badly_draw_lists(m=6)) == [0] * 6


@hegel.composite
def draw_ordered_with_assume(tc):
    x = tc.draw(gs.floats())
    y = tc.draw(gs.floats())
    tc.assume(x < y)
    return (x, y)


@hegel.test(suppress_health_check=[hegel.HealthCheck.filter_too_much])
def test_can_assume_in_draw(tc):
    xy = tc.draw(draw_ordered_with_assume())
    assert xy[0] < xy[1]


def test_uses_definitions_for_reprs():
    assert repr(badly_draw_lists()) == "badly_draw_lists()"
    assert repr(badly_draw_lists(1)) == "badly_draw_lists(1)"
    assert repr(badly_draw_lists(m=1)) == "badly_draw_lists(m=1)"


def test_errors_given_function_of_no_arguments():
    with pytest.raises(hegel.InvalidArgument):

        @hegel.composite
        def foo():
            pass


def test_can_use_pure_args():
    @hegel.composite
    def stuff(tc, *args):
        return tc.draw(gs.sampled_from(args))

    assert minimal(stuff(1, 2, 3, 4, 5)) == 1


def test_composite_of_lists():
    @hegel.composite
    def f(tc):
        return tc.draw(gs.integers()) + tc.draw(gs.integers())

    assert minimal(gs.lists(f()), lambda x: len(x) >= 10) == [0] * 10


@flaky(min_passes=2, max_runs=5)
def test_can_shrink_matrices_with_length_param():
    @hegel.composite
    def matrix(tc):
        rows = tc.draw(gs.integers(1, 10))
        columns = tc.draw(gs.integers(1, 10))
        return [[tc.draw(gs.integers(0, 10000)) for _ in range(columns)] for _ in range(rows)]

    def transpose(m):
        return [[row[i] for row in m] for i in range(len(m[0]))]

    def is_square(m):
        return len(m) == len(m[0])

    value = minimal(matrix(), lambda m: is_square(m) and transpose(m) != m)
    assert len(value) == 2
    assert len(value[0]) == 2
    assert sorted(value[0] + value[1]) == [0, 0, 0, 1]


class MyList(list):
    pass


@hegel.test
def test_does_not_change_arguments(tc):
    # regression test for issue #1017 or other argument mutation
    @hegel.composite
    def strat(tc, arg):
        tc.draw(gs.none())
        return arg

    ls = tc.draw(gs.lists(gs.integers()).map(MyList))
    ex = tc.draw(strat(ls))
    assert ex is ls


class ClsWithGeneratorMethods:
    @classmethod
    @hegel.composite
    def gs_classmethod_then_composite(tc, cls):
        return tc.draw(gs.integers(0, 10))

    @staticmethod
    @hegel.composite
    def gs_staticmethod_then_composite(tc):
        return tc.draw(gs.integers(0, 10))

    @hegel.composite
    def gs_composite_method(tc, self):
        return tc.draw(gs.integers(0, 10))


@hegel.test
def test_applying_composite_decorator_to_methods(tc):
    instance = ClsWithGeneratorMethods()
    for generator in [
        ClsWithGeneratorMethods.gs_classmethod_then_composite(),
        ClsWithGeneratorMethods.gs_staticmethod_then_composite(),
        instance.gs_classmethod_then_composite(),
        instance.gs_staticmethod_then_composite(),
        instance.gs_composite_method(),
    ]:
        x = tc.draw(generator)
        assert isinstance(x, int)
        assert 0 <= x <= 10


def test_composite_allows_overload_without_draw():
    # See https://github.com/HypothesisWorks/hypothesis/issues/3970
    @hegel.composite
    @typing.overload
    def overloaded(tc: hegel.TestCase, *, x: int) -> typing.Literal[True]: ...

    @hegel.composite
    @typing.overload
    def overloaded(tc: hegel.TestCase, *, x: str) -> typing.Literal[False]: ...

    @hegel.composite
    def overloaded(tc: hegel.TestCase, *, x: int | str) -> bool:
        return tc.draw(gs.just(isinstance(x, int)))
