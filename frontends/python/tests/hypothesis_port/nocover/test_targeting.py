# Dropped from the Hypothesis original: test_target_returns_value (Hegel's
# target() returns None); test_reports_target_results's 'Highest target score'
# assertion (libhegel reports no target scores).
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

pytest_plugins = "pytester"

TESTSUITE = """
import hegel
from hegel import generators as gs

@hegel.test(database="")
def test_threshold_problem(tc):
    x = tc.draw(gs.integers(min_value=0))
    tc.target(float(x))
    {0}tc.target(float(x * 2), label="double")
    {0}assert x <= 100000
    assert x <= 100
"""


@pytest.mark.parametrize("multiple", [False, True])
def test_reports_target_results(testdir, multiple):
    script = testdir.makepyfile(TESTSUITE.format("" if multiple else "# "))
    result = testdir.runpytest(script, "--tb=native", "-rN")
    out = "\n".join(result.stdout.lines)
    assert "x = 101" in out, out
    assert result.ret != 0


def test_targeting_increases_max_length():
    @hegel.test(database="", test_cases=200, phases=[hegel.Phase.generate, hegel.Phase.target])
    def test_with_targeting(tc):
        ls = tc.draw(gs.lists(gs.booleans()))
        tc.target(float(len(ls)))
        assert len(ls) <= 80

    with pytest.raises(AssertionError):
        test_with_targeting()


def test_targeting_can_be_disabled():
    def score(enabled):
        result = 0
        phases = [hegel.Phase.generate]
        if enabled:
            phases.append(hegel.Phase.target)

        @hegel.test(database="", test_cases=100, phases=phases, seed=0)
        def test(tc):
            nonlocal result
            ls = tc.draw(gs.lists(gs.integers(0, 255)))
            # cap the score to avoid long test times by unbounded driving of list
            # length upwards
            score = min(sum(ls), 10_000)
            result = max(result, score)
            tc.target(score)

        test()
        return result

    assert score(enabled=True) > score(enabled=False)


def test_issue_2395_regression():
    @hegel.test(test_cases=1000, database="", seed=13479849077938647928)
    def test_targeting_square_loss(tc):
        d = tc.draw(gs.floats().filter(lambda x: abs(x) < 1000))
        tc.target(-((d - 42.5) ** 2.0))

    test_targeting_square_loss()
