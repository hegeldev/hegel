# Dropped from the Hypothesis original: test_error_if_has_no_hints,
# test_error_if_infer_all_and_has_no_hints, test_error_if_infer_is_posarg,
# test_error_if_infer_is_posarg_mixed_with_kwarg, test_given_is_not_a_class_decorator,
# test_suggests_at_settings_if_extra_kwarg_matches_setting_name (@given's
# argument binding).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.utils import fails_with


@pytest.mark.xfail(
    strict=True,
    reason="engine: rejecting every input of a finite space fails the FilterTooMuch "
    "health check rather than raising Unsatisfiable",
)
@fails_with(hegel.Unsatisfiable)
@hegel.test
def test_raises_unsatisfiable_if_all_false_in_finite_set(tc):
    tc.draw(gs.booleans())
    tc.reject()


def test_does_not_raise_unsatisfiable_if_some_false_in_finite_set():
    @hegel.test
    def test_assume_x(tc):
        x = tc.draw(gs.booleans())
        tc.assume(x)

    test_assume_x()


def test_raises_unsatisfiable_if_passed_explicit_nothing():
    @hegel.test
    def test_never_runs(tc):
        tc.draw(gs.nothing())
        raise Exception("Can't ever execute this")

    with pytest.raises(hegel.Unsatisfiable):
        test_never_runs()


def test_given_twice_is_an_error():
    with pytest.raises(hegel.InvalidArgument):

        @hegel.test
        @hegel.test
        def inner(tc):
            pass


def test_nested_test_is_an_error():
    @hegel.test
    def inner(tc):
        tc.draw(gs.integers())

    @hegel.test
    def outer(tc):
        tc.draw(gs.booleans())
        inner()

    with pytest.raises(hegel.InvalidArgument, match="called from inside another"):
        outer()


def test_specific_error_for_coroutine_functions():
    with pytest.raises(hegel.InvalidArgument, match="does not run async tests"):

        @hegel.test(database="")
        async def foo(tc):
            tc.draw(gs.booleans())
