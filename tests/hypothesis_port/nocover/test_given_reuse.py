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

few_cases = hegel.test(test_cases=10)


@few_cases
def test_has_an_arg_named_x(tc):
    x = tc.draw(gs.booleans())
    assert isinstance(x, bool)


@few_cases
def test_has_an_arg_named_y(tc):
    y = tc.draw(gs.booleans())
    assert isinstance(y, bool)


def test_fail_independently():
    @few_cases
    def test_z1(tc):
        tc.draw(gs.text())
        raise AssertionError

    @few_cases
    def test_z2(tc):
        tc.draw(gs.text())

    with pytest.raises(AssertionError):
        test_z1()

    test_z2()
