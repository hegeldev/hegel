# Dropped from the Hypothesis original: test_given_on_setUp_fails_health_check
# (HealthCheck.not_a_test_method); test_subTest and the -Werror case of
# test_subTest_no_self (Hypothesis's warning that subTest swallows failures).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import sys

import pytest

from tests.hypothesis_port.common.utils import skipif_emscripten

SUBTEST_SUITE = """
import unittest
import hegel
from hegel import generators as gs

class MyTest(unittest.TestCase):
    @hegel.test
    def test_subtest(self, tc):
        s = tc.draw(gs.text())
        with self.subTest(text=s):
            self.assertIsInstance(s, str)

if __name__ == "__main__":
    unittest.main()
"""


@skipif_emscripten
def test_subTest_no_self(testdir):
    # https://github.com/HypothesisWorks/hypothesis/issues/2462
    # for some reason this issue happens only when running unittest from commandline
    fname = testdir.makepyfile(tests=SUBTEST_SUITE)
    result = testdir.run(sys.executable, str(fname))
    assert result.ret == pytest.ExitCode.OK, result.stderr.str()
