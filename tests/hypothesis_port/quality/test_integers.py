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


def test_biases_towards_boundary_values():
    trillion = 10**12
    boundary_vals = {-trillion, -trillion + 1, trillion - 1, trillion}

    @hegel.test(test_cases=1000)
    def f(tc):
        n = tc.draw(gs.integers(-trillion, trillion))
        boundary_vals.discard(n)

    f()

    assert not boundary_vals, f"Expected to see all boundary vals, but still have {boundary_vals}"
