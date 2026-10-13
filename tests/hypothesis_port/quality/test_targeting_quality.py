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

common_settings = {
    "database": "",
    "suppress_health_check": hegel.HealthCheck.all(),
    "phases": [hegel.Phase.generate, hegel.Phase.target],
}


@pytest.mark.xfail(
    strict=True,
    reason=(
        "Engine: targeting only starts halfway through the budget, so the threshold is "
        "found after about 5000 cases, more than the 4000 allowed (docs/engine-issues.md)"
    ),
)
@pytest.mark.parametrize("seed_val", [0, 1, 2])
def test_finds_threshold_bug_quickly_with_large_budget(seed_val):
    # check that we optimize sooner than halfway for large test case budgets.
    calls = 0
    found_at = None

    @hegel.test(test_cases=10_000, seed=seed_val, **common_settings)
    def test(tc):
        nonlocal calls, found_at
        ls = tc.draw(gs.lists(gs.integers(0, 1000)))
        calls += 1
        tc.target(min(sum(ls), 50_000))
        if sum(ls) >= 30_000:
            if found_at is None:
                found_at = calls
            raise AssertionError

    with pytest.raises(AssertionError):
        test()
    assert found_at is not None
    assert found_at < 4_000


@pytest.mark.parametrize("seed_val", [0, 1, 2])
def test_reaches_high_scores_with_moderate_budget(seed_val):
    best = 0.0

    @hegel.test(test_cases=2000, seed=seed_val, **common_settings)
    def test(tc):
        nonlocal best
        ls = tc.draw(gs.lists(gs.integers(0, 1000)))
        score = min(sum(ls), 100_000)
        best = max(best, score)
        tc.target(score)

    test()
    # generous margin to account for unrelated engine changes
    assert best >= 50_000
