# Dropped from the Hypothesis original: test_no_missed_custom_init_exceptions
# (the exception list is Hegel's, checked by test_exceptions_are_picklable).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import pickle
import random
from unittest.mock import Mock

import pytest

import hegel
from hegel import errors
from hegel import generators as gs


def strat():
    return gs.builds(dict, one=strat_one())


@hegel.composite
def strat_one(tc):
    return tc.draw(gs.builds(dict, val=gs.builds(dict, two=strat_two())))


@hegel.composite
def strat_two(tc):
    return tc.draw(gs.builds(dict, some_text=gs.text(min_size=1)))


@hegel.test
def test_issue751(tc):
    tc.draw(strat())


def test_can_find_non_zero():
    # This future proofs against a possible failure mode where the depth bound
    # is triggered but we've fixed the behaviour of min_size so that it can
    # handle that: We want to make sure that we're really not depth bounding
    # the text in the leaf nodes.

    @hegel.test(verbosity=hegel.Verbosity.quiet)
    def test(tc):
        v = tc.draw(strat())
        assert "0" in v["one"]["val"]["two"]["some_text"]

    with pytest.raises(AssertionError):
        test()


def test_mock_injection():
    """Ensure that it's possible for mechanisms like `pytest.fixture` and
    `patch` to inject mocks into hegel test functions without side
    effects.

    (covers https://github.com/HypothesisWorks/hypothesis-
    python/issues/491)
    """

    class Bar:
        pass

    @hegel.test
    def test_foo_spec(tc, bar):
        tc.draw(gs.integers())

    test_foo_spec(Bar())
    test_foo_spec(Mock(Bar))
    test_foo_spec(Mock())


def test_regression_issue_1230():
    generator = gs.builds(
        lambda x, y: dict(list(x.items()) + list(y.items())),
        gs.fixed_dicts({"0": gs.text()}),
        gs.builds(
            lambda dictionary, keys: {key: dictionary[key] for key in keys},
            gs.fixed_dicts({"1": gs.lists(gs.sampled_from(["a"]))}),
            gs.sets(gs.sampled_from(["1"])),
        ),
    )

    @hegel.test(database="", seed=81581571036124932593143257836081491416)
    def test_false_is_false(tc):
        params = tc.draw(generator)
        tc.assume(params.get("0") not in ("", "\x00"))
        raise ValueError

    with pytest.raises(ValueError):
        test_false_is_false()


@hegel.test
def random_func(tc):
    tc.draw(gs.integers())
    random.random()


def test_prng_state_unpolluted_by_given_issue_1266():
    # Checks that a hegel test doesn't leave the global PRNG in a particular
    # modified state; there may be no effect or random effect but not
    # a consistent end-state.
    first = random.getstate()
    random_func()
    second = random.getstate()
    random_func()
    third = random.getstate()
    if first == second:
        assert second == third
    else:
        assert second != third


exc_instances = [
    errors.AssumeFailed("reason for unsatisfied"),
    errors.BackendError("backend"),
    errors.DidNotReproduce("did not reproduce"),
    errors.FailedHealthCheck("FailedHealthCheck: FilterTooMuch"),
    errors.Flaky("flaky"),
    errors.HegelError("hegel"),
    errors.InternalError("internal"),
    errors.InvalidArgument("invalid"),
    errors.MultipleFailures("check with Exception", [Exception()]),
    errors.MultipleFailures("check with BaseException", [KeyboardInterrupt()]),
    errors.ResolutionFailed("resolution"),
    errors.RunError("run"),
    errors.StopTest("stop"),
    errors.Unsatisfiable("unsatisfiable"),
]


@pytest.mark.parametrize("exc", exc_instances, ids=repr)
def test_exceptions_are_picklable(exc):
    # See https://github.com/HypothesisWorks/hypothesis/issues/3426
    pickle.loads(pickle.dumps(exc))


def test_every_public_exception_is_tested_for_pickling():
    untested = {
        et
        for et in vars(errors).values()
        if isinstance(et, type) and issubclass(et, BaseException) and et.__name__ in errors.__all__
    } - {type(exc) for exc in exc_instances}
    assert not untested
