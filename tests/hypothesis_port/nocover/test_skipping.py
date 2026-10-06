# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import os
import unittest

import pytest
from _pytest.outcomes import Skipped

from hegel.hypothesis import given, settings
from hegel.hypothesis.database import InMemoryExampleDatabase
from hegel.hypothesis.strategies import integers

from tests.hypothesis_port.common.utils import capture_out


def _db_entries(database):
    # Hegel's database is a directory; count the files it holds.
    return sum(len(files) for _, _, files in os.walk(database._hegel_path))


def skip_exceptions_to_reraise():
    return [unittest.SkipTest, Skipped]


@pytest.mark.parametrize("skip_exception", skip_exceptions_to_reraise())
def test_no_failing_test_case_if_unittest_skip(skip_exception):
    """If a ``SkipTest`` exception is raised during a test, Hypothesis should
    not continue running the test and shrink process, nor should it print
    anything about failing test cases."""

    class DemoTest(unittest.TestCase):
        @given(xs=integers())
        def test_to_be_skipped(self, xs):
            if xs == 0:
                raise skip_exception
            else:
                assert xs == 0

    with capture_out() as o:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(DemoTest)
        unittest.TextTestRunner().run(suite)

    assert "Failing test case" not in o.getvalue()


def test_skip_exceptions_save_database_entries():
    """Skip exceptions should save database entries for immediate replay (issue #4484)."""
    database = InMemoryExampleDatabase()
    call_count = 0
    skip_value = None
    first_value = None

    @settings(database=database, max_examples=100)
    @given(integers())
    def test_func(n):
        nonlocal call_count, skip_value, first_value
        call_count += 1
        if first_value is None:
            first_value = n

        # Skip on the 5th value in the first run (the choice of 5 is arbitrary)
        if call_count == 5 and skip_value is None:
            skip_value = n

        if n == skip_value:
            pytest.skip()

    # First run should raise a skip exception and save to database
    with pytest.raises(Skipped):
        test_func()
    assert _db_entries(database) >= 1  # hegel may store more than one file per entry

    # Second run should immediately replay the skip value
    first_value = None
    call_count = 0
    with pytest.raises(Skipped):
        test_func()

    # first call should be the replayed skip value
    assert first_value == skip_value
    # hegel re-runs the replayed case once more (to confirm it) but generates nothing new
    assert call_count <= 2
