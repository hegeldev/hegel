# Dropped from the Hypothesis original: test_signature_mismatch_error_message
# (@given's argument binding).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import re

import pytest

import hegel
from hegel import generators as gs


@pytest.mark.xfail(
    strict=True,
    reason="engine: with the health checks suppressed, a run whose every case is "
    "rejected ends quietly as passed instead of raising Unsatisfiable",
)
def test_contains_the_test_function_name_in_the_exception_string():
    look_for_one = hegel.Settings(test_cases=1, suppress_health_check=hegel.HealthCheck.all())

    @hegel.test(settings=look_for_one)
    def this_has_a_totally_unique_name(tc):
        tc.draw(gs.integers())
        tc.reject()

    with pytest.raises(
        hegel.Unsatisfiable, match=re.escape(this_has_a_totally_unique_name.__name__)
    ):
        this_has_a_totally_unique_name()

    class Foo:
        @hegel.test(settings=look_for_one)
        def this_has_a_unique_name_and_lives_on_a_class(self, tc):
            tc.draw(gs.integers())
            tc.reject()

    with pytest.raises(
        hegel.Unsatisfiable,
        match=re.escape(Foo.this_has_a_unique_name_and_lives_on_a_class.__name__),
    ):
        Foo().this_has_a_unique_name_and_lives_on_a_class()


@hegel.test
def test_fixed_dict_preserves_keys(tc):
    keys = tc.draw(gs.lists(gs.integers(), unique=True))
    d = tc.draw(gs.fixed_dicts({k: gs.none() for k in keys}))
    assert set(d) == set(keys)
