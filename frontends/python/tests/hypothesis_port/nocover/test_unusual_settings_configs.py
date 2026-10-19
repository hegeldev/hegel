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
from hegel import HealthCheck, Verbosity
from hegel import generators as gs


@hegel.test(test_cases=1, database="")
def test_single_example(tc):
    tc.draw(gs.integers())


@hegel.test(
    test_cases=1,
    database="",
    suppress_health_check=[HealthCheck.filter_too_much, HealthCheck.too_slow],
    verbosity=Verbosity.debug,
)
def test_hard_to_find_single_example(tc):
    n = tc.draw(gs.integers())
    # Numbers are arbitrary, just deliberately unlikely to hit this too soon.
    tc.assume(n % 50 == 11)
