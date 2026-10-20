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

# Tests that composites with only keyword-only arguments can be called.
# See https://github.com/HypothesisWorks/hypothesis/issues/1999


@hegel.composite
def kwonlyargs_composites(tc, *, kwarg1=None):
    return tc.draw(gs.fixed_dicts({"kwarg1": gs.just(kwarg1), "i": gs.integers()}))


@hegel.test
def test_composite_with_keyword_only_args(tc):
    tc.draw(gs.lists(gs.one_of(kwonlyargs_composites(kwarg1="test")), unique_by=lambda x: x["i"]))
