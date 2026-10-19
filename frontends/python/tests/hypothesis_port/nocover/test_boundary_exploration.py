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
from tests.hypothesis_port.common.debug import minimal
from tests.hypothesis_port.common.utils import no_shrink


@pytest.mark.parametrize("strat", [gs.text(min_size=5)])
@hegel.test(phases=no_shrink, suppress_health_check=hegel.HealthCheck.all())
def test_explore_arbitrary_function(tc, strat):
    cache = {}

    def predicate(x):
        try:
            return cache[x]
        except KeyError:
            return cache.setdefault(x, tc.draw(gs.booleans(), name=repr(x)))

    try:
        minimal(
            strat,
            predicate,
            settings=hegel.Settings(test_cases=10, database="", verbosity=hegel.Verbosity.quiet),
        )
    except hegel.Unsatisfiable:
        tc.reject()
