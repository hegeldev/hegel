# Dropped from the Hypothesis original:
# test_note_deprecation_blames_right_code_issue_652 (note_deprecation);
# test_explain_phase_label_assertion_4339 (Phase.explain).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import hegel
from hegel import generators as gs


@hegel.test
def test_performance_issue_2027(tc):
    tc.draw(gs.one_of(gs.just(0) | gs.just(1)))
    tc.draw(gs.one_of(gs.just(0) | gs.just(1) | gs.just(2)))


@hegel.test
def test_unique_floats_with_nan_is_not_flaky_3926(tc):
    tc.draw(gs.lists(gs.floats(allow_infinity=False), unique=True))


# this will take a while to find the regression, but will eventually trigger it.
# min_value=0 is critical to trigger the probing behavior which exhausts our buffer.
# https://github.com/pschanely/CrossHair/issues/285 for an upstream fix.
@hegel.test
def test_overrun_during_datatree_simulation_3874(tc):
    tc.draw(gs.integers(min_value=0, max_value=1 << 25_000))
