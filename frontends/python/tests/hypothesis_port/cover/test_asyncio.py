# Dropped from the Hypothesis original: TestAsyncio and test_foo_yield_from, which
# build generator-based coroutines with asyncio.coroutine (removed in Python 3.11);
# TestAsyncioRun (executors: Hegel does not run async tests).
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


def test_async_test_without_an_executor_is_rejected():
    with pytest.raises(hegel.InvalidArgument, match="does not run async tests"):

        @hegel.test
        async def test(tc):
            tc.draw(gs.integers())
