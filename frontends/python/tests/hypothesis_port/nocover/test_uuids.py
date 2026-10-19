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


@pytest.mark.parametrize("version", (1, 2, 3, 4, 5))
def test_can_generate_specified_version(version):
    @hegel.test
    def inner(tc):
        u = tc.draw(gs.uuids(version=version))
        assert version == u.version

    inner()
