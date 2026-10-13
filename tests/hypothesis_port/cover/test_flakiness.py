# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_flaky_with_context_when_fails_only_under_tracing
# (patches the internal explain-phase Tracer and StateForActualGivenExecution);
# test_flaky_stateful_reports_steps (Hegel reports nondeterministic generation in a state
# machine without the steps leading up to it).

import random
import sys

import pytest

import hegel
from hegel import Flaky, HealthCheck, Unsatisfiable, Verbosity
from hegel import generators as gs
from tests.hypothesis_port.common.utils import no_shrink, notes

# Hegel confirms a failure by replaying it this many times; a failure that
# never recurs is reported as unconfirmed, and under
# nondeterminism_strictness="error" as Flaky.
REPLAYS = 10


class Nope(Exception):
    pass


def test_fails_only_once_is_flaky():
    first_call = True

    @hegel.test(nondeterminism_strictness="error")
    def rude(tc):
        nonlocal first_call
        tc.draw(gs.integers())
        if first_call:
            first_call = False
            raise Nope

    with pytest.raises(Flaky):
        rude()


def test_fails_only_once_is_reported_as_unconfirmed_by_default():
    first_call = True

    @hegel.test
    def rude(tc):
        nonlocal first_call
        tc.draw(gs.integers())
        if first_call:
            first_call = False
            raise Nope

    with pytest.raises(Nope) as e:
        rude()
    assert any(note.startswith("unconfirmed failure") for note in notes(e.value))


def test_fails_differently_is_flaky():
    call_count = 0

    class DifferentNope(Exception):
        pass

    @hegel.test(database="", nondeterminism_strictness="error")
    def rude(tc):
        nonlocal call_count
        x = tc.draw(gs.integers())
        if x == 0:
            call_count += 1
            if call_count > 1:
                raise Nope
            else:
                raise DifferentNope

    with pytest.raises(Flaky):
        rude()


@pytest.mark.skipif(sys.version_info < (3, 11), reason="except* syntax")
def test_exceptiongroup_wrapped_naked_exception_is_flaky():
    # Defer parsing until runtime, as "except*" is syntax error pre 3.11
    rude_def = """
first_call = True
def rude_fn(tc):
    global first_call
    tc.draw(gs.integers())
    if first_call:
        first_call = False
        try:
            raise Nope
        except* Nope:
            raise
    """
    exec(rude_def, globals())
    rude = hegel.test(nondeterminism_strictness="error")(rude_fn)  # noqa: F821 # defined by exec()

    with pytest.raises(Flaky):
        rude()


def test_gives_flaky_error_if_assumption_is_flaky():
    seen = set()

    @hegel.test(verbosity=Verbosity.quiet, database="", nondeterminism_strictness="error")
    def oops(tc):
        s = tc.draw(gs.integers())
        tc.assume(s not in seen)
        seen.add(s)
        raise AssertionError

    with pytest.raises(Flaky):
        oops()


def test_does_not_attempt_to_shrink_flaky_errors():
    values = []

    @hegel.test(database="")
    def test(tc):
        x = tc.draw(gs.integers())
        values.append(x)
        assert len(values) != 1

    with pytest.raises(AssertionError) as e:
        test()
    assert any(note.startswith("unconfirmed failure") for note in notes(e.value))
    # The flaky example is replayed to confirm it, and nothing else is tried:
    # no shrinking, and no further generation.
    assert 1 < len(values) <= REPLAYS + 1
    assert set(values) == {values[0]}


class SatisfyMe(Exception):
    pass


@hegel.composite
def single_bool_lists(tc):
    n = tc.draw(gs.integers(0, 20))
    result = [False] * (n + 1)
    result[n] = True
    return result


def _random_sequences(count, seed=0):
    """Sequences of the shape Hypothesis drew from
    ``lists(booleans()) | single_bool_lists()`` and ``lists(integers(1, 3))``:
    the inner run below is a Hegel test, which cannot run inside another."""
    rnd = random.Random(seed)
    for _ in range(count):
        if rnd.random() < 0.5:
            building = [rnd.random() < 0.5 for _ in range(rnd.randint(0, 10))]
        else:
            n = rnd.randint(0, 20)
            building = [False] * (n + 1)
            building[n] = True
        testing = [rnd.randint(1, 3) for _ in range(rnd.randint(0, 10))]
        yield building, testing


@pytest.mark.parametrize(
    "building, testing",
    [
        ([True, False, False, False], [3]),
        ([False, True, False, False], [3]),
        ([False, False, True, False], [3]),
        ([False, False, False, True], [3]),
        *_random_sequences(10),
    ],
)
def test_failure_sequence_inducing(building, testing):
    buildit = iter(building)
    testit = iter(testing)

    @hegel.composite
    def build(tc):
        x = tc.draw(gs.integers())
        try:
            tc.assume(not next(buildit))
        except StopIteration:
            pass
        return x

    @hegel.test(
        verbosity=Verbosity.quiet,
        database="",
        suppress_health_check=list(HealthCheck),
        phases=no_shrink,
        test_cases=10,
    )
    def test(tc):
        tc.draw(build())
        try:
            i = next(testit)
        except StopIteration:
            return
        if i == 1:
            return
        elif i == 2:
            tc.reject()
        else:
            raise Nope

    try:
        test()
    except (Nope, Flaky, Unsatisfiable):
        pass
    except hegel.AssumeFailed:
        raise SatisfyMe from None
