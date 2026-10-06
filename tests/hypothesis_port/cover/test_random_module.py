# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import gc
import random
import sys

# Dropped (Hegel's register_random does not manage registered PRNG state, and there is no deterministic_PRNG or entropy module): test_registering_a_Random_is_idempotent, test_manages_registered_Random_instance, test_registered_Random_is_seeded_by_random_module_strategy, test_given_does_not_pollute_state, test_find_does_not_pollute_state, test_evil_prng_registration_nonsense, test_passing_unreferenced_instance_raises, test_passing_unreferenced_instance_within_function_scope_raises, test_passing_referenced_instance_within_function_scope_raises, test_register_random_within_nested_function_scope.


import pytest

from hegel.hypothesis import (
    Phase,
    core,
    find,
    given,
    register_random,
    settings,
    strategies as st,
)
from hegel.hypothesis.errors import HypothesisWarning, InvalidArgument
from tests.hypothesis_port.common.compat import GRAALPY, PYPY

from tests.hypothesis_port.common.utils import skipif_threading, xfail_if_gil_disabled


def gc_collect():
    # CPython uses reference counting, so objects (without circular refs)
    # are collected immediately on `del`, breaking weak references.
    # Python implementations with other garbage collection strategies may
    # or may not, so we use this function in tests before counting the
    # surviving references to ensure that they're deterministic.
    if PYPY or GRAALPY:
        gc.collect()


def test_can_seed_random():
    @settings(phases=(Phase.generate, Phase.shrink))
    @given(st.random_module())
    def test(r):
        raise AssertionError

    with pytest.raises(AssertionError) as err:
        test()
    assert "RandomSeeder(0)" in "\n".join(err.value.__notes__)


@given(st.random_module(), st.random_module())
@pytest.mark.xfail(
    strict=True,
    reason="Hegel's random_module() draws its seed from the test case, so two draws can coincide",
)
def test_seed_random_twice(r, r2):
    assert repr(r) == repr(r2)


# ideally we would actually raise the global random warning here, but random_module
# calls seed_all and restore_all inside the deprecate_random_in_strategy context
# manager, which never sees the global random interference.
#
# But it can sometimes see it under multithreading depending on timing. Until
# we fix this to also warn in this case, just skip on threading.
@skipif_threading
@given(st.random_module())
def test_does_not_fail_health_check_if_randomness_is_used(r):
    random.getrandbits(128)


def test_cannot_register_non_Random():
    with pytest.raises(InvalidArgument):
        register_random("not a Random instance")


@given(st.random_module())
@skipif_threading  # writing to global random state
def test_will_actually_use_the_random_seed(rnd):
    a = random.randint(0, 100)
    b = random.randint(0, 100)
    random.seed(rnd.seed)
    assert a == random.randint(0, 100)
    assert b == random.randint(0, 100)
