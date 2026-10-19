# Dropped from the Hypothesis original: test_threadlocal_setattr_and_getattr,
# test_nonexistent_getattr_raises, test_nonexistent_setattr_raises,
# test_raises_if_not_passed_callable (the internal ThreadLocal helper);
# TestNoDifferingExecutorsHealthCheck (HealthCheck.differing_executors).
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

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.utils import skipif_emscripten


@skipif_emscripten
def test_run_given_concurrently():
    # this is just a basic covering test. The more complicated and complete threading
    # tests are in nocover/test_threading.py.

    n_threads = 2
    barrier = Barrier(n_threads)

    @hegel.test
    def test(tc):
        tc.draw(gs.integers())
        barrier.wait()

    threads = [Thread(target=test) for _ in range(n_threads)]

    for thread in threads:
        thread.start()

    for thread in threads:
        thread.join(timeout=10)
