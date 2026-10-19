# Dropped from the Hypothesis original: test_prints_initial_attempts_on_find (find()).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import hegel
from hegel import generators as gs
from hegel.generators import booleans, integers, lists
from tests.hypothesis_port.common.utils import capture_out, fails

# Hegel's verbose output announces each example with "Running test case"
# rather than Hypothesis's "Test case:" lines.


def test_prints_intermediate_in_success():
    with capture_out() as o:

        @hegel.test(verbosity=hegel.Verbosity.verbose)
        def test_works(tc):
            tc.draw(booleans())

        test_works()
    assert "Running test case" in o.getvalue()


def test_notes_are_not_printed_twice_in_debug_mode():
    # the engine's debug output must not repeat notes which were already
    # printed
    @hegel.test(verbosity=hegel.Verbosity.debug, test_cases=1, database="")
    def test_works(tc):
        tc.draw(booleans())
        tc.note("this is a note")

    with capture_out() as o:
        test_works()
    assert o.getvalue().count("this is a note") == 1


def test_does_not_log_in_quiet_mode():
    with capture_out() as o:

        @fails
        @hegel.test(verbosity=hegel.Verbosity.quiet, print_blob=False)
        def test_foo(tc):
            tc.draw(integers())
            raise AssertionError

        test_foo()
    assert not o.getvalue()


def test_includes_progress_in_verbose_mode():
    class Found(Exception):
        pass

    def search(tc):
        x = tc.draw(lists(integers(), min_size=1))
        if sum(x) >= 100:
            raise Found

    with capture_out() as o:
        try:
            hegel.run(search, verbosity=hegel.Verbosity.verbose)
        except Found:
            pass
    out = o.getvalue()
    assert out
    assert "Running test case" in out


def test_includes_intermediate_results_in_verbose_mode():
    with capture_out() as o:

        @fails
        @hegel.test(
            verbosity=hegel.Verbosity.verbose,
            database="",
            derandomize=True,
            test_cases=100,
        )
        def test_foo(tc):
            x = tc.draw(lists(integers(), min_size=1))
            assert sum(x) < 10000

        test_foo()
    lines = o.getvalue().splitlines()
    assert len([l for l in lines if "Running test case" in l]) > 2
    assert [l for l in lines if "AssertionError" in l]


@hegel.test(verbosity=hegel.Verbosity.quiet)
@hegel.explicit_test_case(0)
def test_no_indexerror_in_quiet_mode(tc):
    # Regression tests for https://github.com/HypothesisWorks/hypothesis/issues/2696
    # where quiet mode -> no fragments to report -> IndexError accessing first report
    tc.draw(gs.integers())


@fails
@hegel.test(verbosity=hegel.Verbosity.quiet, report_multiple_failures=True)
@hegel.explicit_test_case(0)
def test_no_indexerror_in_quiet_mode_report_multiple(tc):
    x = tc.draw(gs.integers())
    assert x


@fails
@hegel.test(verbosity=hegel.Verbosity.quiet, report_multiple_failures=False)
@hegel.explicit_test_case(0)
def test_no_indexerror_in_quiet_mode_report_one(tc):
    x = tc.draw(gs.integers())
    assert x
