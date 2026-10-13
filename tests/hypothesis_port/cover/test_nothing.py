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
from tests.hypothesis_port.common.debug import assert_no_examples, minimal


def test_resampling():
    x = minimal(
        gs.lists(gs.integers(), min_size=1).flat_map(lambda x: gs.lists(gs.sampled_from(x))),
        lambda x: len(x) >= 10 and len(set(x)) == 1,
    )
    assert x == [0] * 10


@hegel.test
def test_list_of_nothing(tc):
    xs = tc.draw(gs.lists(gs.nothing()))
    assert xs == []


@hegel.test
def test_set_of_nothing(tc):
    xs = tc.draw(gs.sets(gs.nothing()))
    assert xs == set()


def test_validates_min_size():
    with pytest.raises(hegel.InvalidArgument):
        gs.lists(gs.nothing(), min_size=1)


def test_function_composition():
    assert gs.nothing().map(lambda x: "hi").is_empty
    assert gs.nothing().filter(lambda x: True).is_empty
    assert gs.nothing().flat_map(lambda x: gs.integers()).is_empty


def test_tuples_detect_empty_elements():
    assert gs.tuples(gs.nothing()).is_empty


def test_fixed_dictionaries_detect_empty_values():
    assert gs.fixed_dicts({"a": gs.nothing()}).is_empty


def test_no_examples():
    assert_no_examples(gs.nothing())


@pytest.mark.parametrize(
    "s",
    [
        gs.nothing(),
        gs.nothing().map(lambda x: x),
        gs.nothing().filter(lambda x: True),
        gs.nothing().flat_map(lambda x: gs.integers()),
    ],
)
def test_empty(s):
    assert s.is_empty
