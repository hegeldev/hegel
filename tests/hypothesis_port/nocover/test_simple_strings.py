# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import unicodedata

import hegel
from hegel import generators as gs


@hegel.test(test_cases=2000)
def test_does_not_generate_surrogates(tc):
    t = tc.draw(gs.text(min_size=1, max_size=1))
    assert unicodedata.category(t) != "Cs"
