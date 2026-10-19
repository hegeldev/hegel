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
from tests.hypothesis_port.common.utils import fails_with


@fails_with(hegel.Unsatisfiable)
@hegel.test(test_cases=50, suppress_health_check=hegel.HealthCheck.all())
def test_raises_unsatisfiable_if_all_false(tc):
    tc.draw(gs.integers())
    tc.reject()
