# Dropped from the Hypothesis original: test_skip_exceptions_save_database_entries (a database
# entry saved on skip).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import unittest

import pytest
from _pytest.outcomes import Skipped

import hegel
from hegel.generators import integers
from tests.hypothesis_port.common.utils import capture_out


def skip_exceptions_to_reraise():
    return [unittest.SkipTest, Skipped]


@pytest.mark.parametrize("skip_exception", skip_exceptions_to_reraise())
def test_no_failing_test_case_if_unittest_skip(skip_exception):
    """If a ``SkipTest`` exception is raised during a test, Hegel should
    not continue running the test and shrink process, nor should it print
    anything about failing test cases."""

    class DemoTest(unittest.TestCase):
        @hegel.test
        def test_to_be_skipped(self, tc):
            xs = tc.draw(integers())
            if xs == 0:
                raise skip_exception
            else:
                assert xs == 0

    with capture_out() as o:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(DemoTest)
        unittest.TextTestRunner(stream=o).run(suite)

    assert "xs = " not in o.getvalue()
