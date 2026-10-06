# Dropped from the Hypothesis original: test_threadlocal_setattr_and_getattr,
# test_nonexistent_getattr_raises, test_nonexistent_setattr_raises and
# test_raises_if_not_passed_callable (test the internal ThreadLocal helper).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

from threading import Barrier, Thread

import pytest

from hegel.hypothesis import given, settings, strategies as st

from tests.hypothesis_port.common.utils import skipif_emscripten


@skipif_emscripten
@pytest.mark.skipif(
    settings.get_current_profile_name() == "crosshair",
    reason="crosshair is not thread-safe",
)
def test_run_given_concurrently():
    # this is just a basic covering test. The more complicated and complete threading
    # tests are in nocover/test_threading.py.

    n_threads = 2
    barrier = Barrier(n_threads)

    @given(st.integers())
    def test(n):
        barrier.wait()

    threads = [Thread(target=test) for _ in range(n_threads)]

    for thread in threads:
        thread.start()

    for thread in threads:
        thread.join(timeout=10)


# rely on our pytest-run-parallel job to test this, since this only happens because
# pytest instantiates a new class instance for each parametrization.
class TestNoDifferingExecutorsHealthCheck:
    @given(st.integers())
    @pytest.mark.parametrize("x", range(2))
    def test_a(self, x, i):
        pass
