# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_complex_forward_ref_in_typed_dict_local
# (from_type does not handle function-local forward references in a TypedDict).

from __future__ import annotations

from typing import TypedDict

import hegel
from hegel import generators as gs

alias = int | str


class A(TypedDict):
    a: int


class B(TypedDict):
    a: A
    b: alias


@hegel.test
def test_complex_forward_ref_in_typed_dict(tc):
    d = tc.draw(gs.from_type(B))
    assert isinstance(d["a"], dict)
    assert isinstance(d["a"]["a"], int)
    assert isinstance(d["b"], (int, str))
