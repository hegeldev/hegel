# Dropped from the Hypothesis original: test_signature_mismatch_error_message
# (@given's argument binding) and
# test_contains_the_test_function_name_in_the_exception_string (Hypothesis's
# message wording).
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
def test_fixed_dict_preserves_keys(tc):
    keys = tc.draw(gs.lists(gs.integers(), unique=True))
    d = tc.draw(gs.fixed_dicts({k: gs.none() for k in keys}))
    assert set(d) == set(keys)
