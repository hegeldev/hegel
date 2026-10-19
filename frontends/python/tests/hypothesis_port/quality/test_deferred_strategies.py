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

from hegel import generators as gs
from tests.hypothesis_port.common.debug import minimal


def test_large_branching_tree():
    tree = gs.deferred(lambda: gs.integers() | gs.tuples(tree, tree, tree, tree, tree))
    assert minimal(tree) == 0
    assert minimal(tree, lambda x: isinstance(x, tuple)) == (0,) * 5


@pytest.mark.xfail(
    strict=True,
    reason="hegel rarely generates a dict with key '' holding a list from this definition "
    "(about 9 in 2000 draws), so minimal()'s 500-case search is Unsatisfiable; "
    "with 2000 cases it finds and shrinks to {'': []}",
)
def test_non_trivial_json():
    json = gs.deferred(lambda: gs.none() | gs.floats() | gs.text() | lists | objects)

    lists = gs.lists(json)
    objects = gs.dicts(gs.text(), json)

    assert minimal(json) is None
    assert minimal(json, lambda x: isinstance(x, list) and x) == [None]
    assert minimal(json, lambda x: isinstance(x, dict) and isinstance(x.get(""), list)) == {"": []}


def test_self_recursive_lists():
    x = gs.deferred(lambda: gs.lists(x))
    assert minimal(x) == []
    assert minimal(x, bool) == [[]]
    assert minimal(x, lambda x: len(x) > 1) == [[], []]
