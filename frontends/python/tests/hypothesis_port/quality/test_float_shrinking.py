# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import math

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import minimal


def test_shrinks_to_simple_floats():
    assert minimal(gs.floats(), lambda x: x > 1) == 2.0
    assert minimal(gs.floats(), lambda x: x > 0) == 1.0


@pytest.mark.parametrize("n", [1, 2, 3, 8, 10])
def test_can_shrink_in_variable_sized_context(n):
    x = minimal(gs.lists(gs.floats(), min_size=n), any)
    assert len(x) == n
    assert x.count(0.0) == n - 1
    assert 1 in x


@hegel.test
@hegel.explicit_test_case(1.7976931348623157e308)
@hegel.explicit_test_case(1.5)
def test_shrinks_downwards_to_integers(tc):
    f = tc.draw(gs.floats(min_value=0, allow_infinity=False, allow_nan=False))
    assert minimal(gs.floats(min_value=f)) == math.ceil(f)


@hegel.test
@hegel.explicit_test_case(1)
def test_shrinks_downwards_to_integers_when_fractional(tc):
    b = tc.draw(gs.integers(1, 2**16 - 1))
    g = minimal(
        gs.floats(min_value_exclusive=b, max_value_exclusive=2**53).filter(lambda x: int(x) != x)
    )
    assert g == b + 0.5
