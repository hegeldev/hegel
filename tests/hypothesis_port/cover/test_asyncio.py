# Dropped from the Hypothesis original: TestAsyncio and test_foo_yield_from, which
# build generator-based coroutines with asyncio.coroutine (removed in Python 3.11).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import asyncio
from unittest import TestCase

import pytest

from hegel.hypothesis import assume, given, settings, strategies as st
from hegel.hypothesis.errors import InvalidArgument

from tests.hypothesis_port.common.utils import skipif_emscripten

pytestmark = pytest.mark.skipif(
    settings.get_current_profile_name() == "threading",
    reason="bad interactions when mixing threads and asyncio",
)


class TestAsyncioRun(TestCase):
    # In principle, these tests could indeed run on emscripten if we grab the existing
    # event loop and run them there.  However, that seems to have hit an infinite loop
    # and so we're just skipping them for now and will revisit later.

    def execute_example(self, f):
        asyncio.run(f())

    @skipif_emscripten
    @given(st.text())
    async def test_foo_await(self, x):
        assume(x)
        await asyncio.sleep(0.001)
        assert x


def test_async_test_without_an_executor_is_rejected():
    @given(st.integers())
    async def test(x):
        pass

    with pytest.raises(InvalidArgument, match="doesn't know how to run async test functions"):
        test()
