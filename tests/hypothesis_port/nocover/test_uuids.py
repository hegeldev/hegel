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
from tests.hypothesis_port.common.debug import minimal


@pytest.mark.xfail(
    strict=False,
    reason="Engine: uuids() repeats values within a run (no duplicate suppression, "
    "docs/engine-issues.md), so a list of them can contain the same UUID twice",
)
@hegel.test
def test_are_unique(tc):
    ls = tc.draw(gs.lists(gs.uuids()))
    assert len(set(ls)) == len(ls)


@pytest.mark.xfail(
    strict=True,
    reason="Engine: shrinking a list of uuids() drives every element to the same "
    "minimal UUID (observed: five copies of UUID(int=1))",
)
def test_retains_uniqueness_in_simplify():
    ts = minimal(gs.lists(gs.uuids()), lambda x: len(x) >= 5)
    assert len(ts) == len(set(ts)) == 5


@pytest.mark.parametrize("version", (1, 2, 3, 4, 5))
def test_can_generate_specified_version(version):
    @hegel.test
    def inner(tc):
        u = tc.draw(gs.uuids(version=version))
        assert version == u.version

    inner()
