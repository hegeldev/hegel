# Dropped from the Hypothesis original: test_warns_on_bool_strategy (HypothesisWarning).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import re

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import notes


def test_error_is_in_finally():
    @hegel.test
    def test(tc):
        try:
            tc.draw(gs.lists(gs.integers(), min_size=3, unique=True))
        finally:
            raise ValueError

    with pytest.raises(ValueError) as err:
        test()

    assert "[0, 1, -1]" in "\n".join(notes(err.value))


def test_adds_note_showing_which_strategy():
    class X:
        def __init__(self, y: int) -> None:
            assert y == 7

    @hegel.test
    def inner(tc):
        value = tc.draw(gs.from_type(X))
        assert isinstance(value, X)

    rep = re.escape(repr(gs.from_type(X)))
    with pytest.raises(
        AssertionError, match=f"value = <AssertionError raised while drawing from {rep}>"
    ):
        inner()


def test_adds_note_showing_which_strategy_stateful():
    strategy = gs.integers().map(lambda x: x / 0)

    class Machine(hegel.StateMachine):
        @hegel.rule()
        def take_a_step(self, tc):
            value = tc.draw(strategy)
            assert value

    msg = f"value = <ZeroDivisionError raised while drawing from {strategy!r}>"
    with pytest.raises(ZeroDivisionError, match=re.escape(msg)) as err:
        hegel.state_machine_test(Machine)()
    assert "take_a_step" in "\n".join(notes(err.value))
