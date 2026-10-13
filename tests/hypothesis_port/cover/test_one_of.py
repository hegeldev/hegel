# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_one_of_empty (one_of() with no
# arguments as nothing(); hegel rejects it).

from collections.abc import Sequence

import pytest

import hegel
from hegel import generators as gs


@hegel.test
def test_one_of_filtered(tc):
    i = tc.draw(gs.one_of(gs.integers().filter(bool)))
    assert bool(i)


@hegel.test
def test_one_of_flatmapped(tc):
    i = tc.draw(gs.one_of(gs.just(100).flat_map(gs.integers)))
    assert i >= 100


def test_one_of_single_strategy_is_noop():
    s = gs.integers()
    assert gs.one_of(s) is s
    assert gs.one_of([s]) is s


def test_one_of_without_strategies_is_an_error():
    with pytest.raises(hegel.InvalidArgument):
        gs.one_of(1, 2, 3)


@pytest.mark.parametrize(
    "strategy, count",
    [
        (gs.one_of(gs.integers(), gs.integers(), gs.integers()), 1),
        # Hypothesis expects 2 here: its reprs are lazy, so a nested one_of
        # shows as written. hegel's one_of flattens eagerly.
        (gs.one_of(gs.integers(), gs.one_of(gs.integers(), gs.integers())), 1),
        ((gs.integers() | gs.integers()) | gs.integers(), 1),
        (gs.integers() | (gs.integers() | gs.integers()), 1),
        (gs.integers() | gs.text() | gs.booleans(), 1),
        # Hypothesis expects 2 here, as its from_type(Sequence) is a lazy
        # one_of that is not unwrapped; hegel's is built flat.
        (gs.from_type(int | Sequence[int]), 1),
    ],
)
def test_one_of_unwrapping(strategy, count):
    assert repr(strategy).count("one_of(") == count
