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


def test_max_examples_are_respected():
    counter = 0

    @hegel.test(test_cases=100)
    def test(tc):
        nonlocal counter
        tc.draw(gs.integers())
        counter += 1

    test()
    assert counter == 100
