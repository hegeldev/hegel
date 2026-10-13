# Dropped from the Hypothesis original: test_cannot_target_outside_test
# (target() is a method of the test case, which only exists inside a test).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

from operator import itemgetter

import pytest

import hegel
from hegel import generators as gs


@hegel.explicit_test_case(0.0, "this covers the branch where target() is ignored")
@hegel.test
def test_allowed_inputs_to_target(tc):
    observation = tc.draw(gs.integers() | gs.floats(allow_nan=False, allow_infinity=False))
    label = tc.draw(gs.text())
    tc.target(observation, label=label)


@hegel.test
def test_allowed_inputs_to_target_fewer_labels(tc):
    observation = tc.draw(
        gs.integers(min_value=1) | gs.floats(min_value=1, allow_nan=False, allow_infinity=False)
    )
    label = tc.draw(gs.sampled_from(["a", "few", "labels"]))
    tc.target(observation, label=label)


@hegel.test
def test_target_without_label(tc):
    observation = tc.draw(gs.floats(min_value=1, max_value=10))
    tc.target(observation)


@hegel.test
def test_multiple_target_calls(tc):
    args = tc.draw(
        gs.lists(
            gs.tuples(gs.floats(allow_nan=False, allow_infinity=False), gs.text()),
            min_size=1,
            unique_by=itemgetter(1),
        )
    )
    for observation, label in args:
        tc.target(observation, label=label)


@hegel.test
def test_respects_max_pool_size(tc):
    """Using many examples of several labels like this stresses the
    pool-size logic and internal assertions in TargetSelector.
    """
    observations = tc.draw(
        gs.lists(gs.floats(allow_nan=False, allow_infinity=False), min_size=11, max_size=20)
    )
    for i, obs in enumerate(observations):
        tc.target(obs, label=str(i))


# target() is ignored in an explicit test case, so the invalid inputs are
# checked in generated ones instead of as @example cases.
@pytest.mark.parametrize("observation", [float("nan"), float("inf"), float("-inf")])
def test_disallowed_non_finite_observations(observation):
    @hegel.test
    def test(tc):
        tc.draw(gs.none())
        with pytest.raises(hegel.InvalidArgument):
            tc.target(observation, label="")

    test()


@pytest.mark.xfail(
    strict=True,
    reason="hegel's TestCase.target coerces the score with float(), so a numeric "
    "string is accepted and other non-numbers raise TypeError rather than InvalidArgument",
)
@pytest.mark.parametrize("observation", ["1", None, [0.0]])
def test_disallowed_non_numeric_observations(observation):
    @hegel.test
    def test(tc):
        tc.draw(gs.none())
        with pytest.raises(hegel.InvalidArgument):
            tc.target(observation, label="Non-float observations are invalid")

    test()


@pytest.mark.xfail(
    strict=True,
    reason="hegel's TestCase.target raises TypeError rather than InvalidArgument "
    "for a label that is not a string",
)
@pytest.mark.parametrize("label", [["a list of strings is not a valid label"], 1, None])
def test_disallowed_labels(label):
    @hegel.test
    def test(tc):
        tc.draw(gs.none())
        with pytest.raises(hegel.InvalidArgument):
            tc.target(0.0, label=label)

    test()


@hegel.test
def test_cannot_target_same_label_twice(tc):
    tc.draw(gs.none())
    tc.target(0.0, label="label")
    with pytest.raises(hegel.InvalidArgument):
        tc.target(1.0, label="label")


@hegel.test
def test_cannot_target_default_label_twice(tc):
    tc.draw(gs.none())
    tc.target(0.0)
    with pytest.raises(hegel.InvalidArgument):
        tc.target(1.0)
