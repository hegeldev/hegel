# Dropped from the Hypothesis original:
# test_baseexception_in_strategy_no_rerun_no_flaky[SystemExit], test_explanations[SystemExit]
# (Hegel lets SystemExit propagate rather than treating it as a failure).
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
from hegel.generators import integers, none

pytest_plugins = "pytester"


@pytest.mark.parametrize("e", [KeyboardInterrupt, SystemExit, GeneratorExit, ValueError])
def test_exception_propagates_fine(e):
    @hegel.test
    def test_raise(tc):
        tc.draw(integers())
        raise e

    with pytest.raises(e):
        test_raise()


@pytest.mark.parametrize("e", [KeyboardInterrupt, SystemExit, GeneratorExit, ValueError])
def test_exception_propagates_fine_from_strategy(e):
    @hegel.composite
    def interrupt_eventually(tc):
        raise e
        # this line will not be executed, but must be here
        # to pass draw function static reference check
        return tc.draw(none())

    @hegel.test
    def test_do_nothing(tc):
        tc.draw(interrupt_eventually())

    with pytest.raises(e):
        test_do_nothing()


@pytest.mark.parametrize("e", [KeyboardInterrupt, ValueError])
def test_baseexception_no_rerun_no_flaky(e):
    runs = 0
    interrupt = 3

    # A failure that does not replay is Flaky only under the strict
    # nondeterminism setting; by default Hegel reports it with a caveat.
    @hegel.test(nondeterminism_strictness="error")
    def test_raise_baseexception(tc):
        nonlocal runs
        tc.draw(integers())
        runs += 1
        if runs == interrupt:
            raise e

    if issubclass(e, (KeyboardInterrupt, SystemExit, GeneratorExit)):
        # Here SystemExit and GeneratorExit are passed through
        with pytest.raises(e):
            test_raise_baseexception()

        assert runs == interrupt
    else:
        with pytest.raises(hegel.Flaky):
            test_raise_baseexception()


@pytest.mark.parametrize("e", [KeyboardInterrupt, GeneratorExit, ValueError])
def test_baseexception_in_strategy_no_rerun_no_flaky(e):
    runs = 0
    interrupt = 3

    @hegel.composite
    def interrupt_eventually(tc):
        nonlocal runs
        runs += 1
        if runs == interrupt:
            raise e
        return tc.draw(integers())

    @hegel.test(nondeterminism_strictness="error")
    def test_do_nothing(tc):
        tc.draw(interrupt_eventually())

    if issubclass(e, KeyboardInterrupt):
        with pytest.raises(e):
            test_do_nothing()

        assert runs == interrupt

    else:
        # Now GeneratorExit is caught like other exceptions
        with pytest.raises(hegel.Flaky):
            test_do_nothing()


TEMPLATE = """
import hegel
from hegel import generators as gs

@hegel.composite
def things(tc):
    raise {exception}
    # this line will not be executed, but must be here
    # to pass draw function static reference check
    return tc.draw(gs.none())


@hegel.test
def test(tc):
    x = tc.draw(gs.integers())
    if x > 100:
        tc.draw({strategy})
        raise {exception}
"""


@pytest.mark.parametrize("exc_name", ["GeneratorExit"])
@pytest.mark.parametrize("use_composite", [True, False])
def test_explanations(testdir, exc_name, use_composite):
    code = TEMPLATE.format(
        exception=exc_name, strategy="things()" if use_composite else "gs.none()"
    )
    test_file = str(testdir.makepyfile(code))
    pytest_stdout = str(testdir.runpytest_inprocess(test_file, "--tb=native").stdout)
    assert "x = 101" in pytest_stdout
    assert exc_name in pytest_stdout
