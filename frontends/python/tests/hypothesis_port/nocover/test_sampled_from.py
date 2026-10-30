# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_unsat_sets_of_samples (InvalidArgument
# for a set larger than its sampled element count); test_repr_truncates_with_many_elements
# (repr truncation).

import enum
import functools
import itertools
import operator

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import find_any, minimal


# 2**25 is omitted: hegel's sampled_from materialises the range (~50s).
@pytest.mark.parametrize("size", [100, 10**5, 10**6])
@hegel.test
def test_filter_large_lists(tc, size):
    n_calls = 0

    def cond(x):
        nonlocal n_calls
        n_calls += 1
        return x % 2 != 0

    s = tc.draw(gs.sampled_from(range(size)).filter(cond))

    assert s % 2 != 0
    assert n_calls <= 100  # Hypothesis's SampledFromStrategy._MAX_FILTER_CALLS


def rare_value_strategy(n, target):
    def forbid(s, forbidden):
        """Helper function to avoid Python variable scoping issues."""
        return s.filter(lambda x: x != forbidden)

    s = gs.sampled_from(range(n))
    for i in range(n):
        if i != target:
            s = forbid(s, i)

    return s


@hegel.test
def test_chained_filters_find_rare_value(tc):
    x = tc.draw(rare_value_strategy(n=128, target=80))
    assert x == 80


@hegel.test
def test_efficient_sets_of_samples(tc):
    x = tc.draw(gs.sets(gs.sampled_from(range(50)), min_size=50))
    assert x == set(range(50))


class AnEnum(enum.Enum):
    a = enum.auto()
    b = enum.auto()


def test_enum_repr_names_the_class():
    # The repr should have enough detail to find the class again.
    assert "AnEnum" in repr(gs.sampled_from(AnEnum))


class AFlag(enum.Flag):
    a = enum.auto()
    b = enum.auto()
    c = enum.auto()


LargeFlag = enum.Flag("LargeFlag", {f"bit{i}": enum.auto() for i in range(64)})


class UnnamedFlag(enum.Flag):
    # Would fail under EnumCheck.NAMED_FLAGS
    a = 0
    b = 7


def test_flag_enum_repr_names_the_class():
    assert "AFlag" in repr(gs.sampled_from(AFlag))


def test_exhaustive_flags():
    # Generate powerset of flag combinations. There are only 2^3 of them, so
    # we can reasonably expect that they are all are found. Each multi-bit
    # combination is drawn about one case in twenty, so 100 cases miss one
    # about once in 500 runs; 1000 cases never do.
    unseen_flags = {
        functools.reduce(operator.or_, flaglist, AFlag(0))
        for r in range(len(AFlag) + 1)
        for flaglist in itertools.combinations(AFlag, r)
    }

    @hegel.test(test_cases=1000)
    def accept(tc):
        flag = tc.draw(gs.sampled_from(AFlag))
        unseen_flags.discard(flag)

    accept()

    assert not unseen_flags


def test_flags_minimize_to_first_named_flag():
    assert minimal(gs.sampled_from(LargeFlag)) == LargeFlag.bit0


def test_flags_minimizes_bit_count():
    assert (
        minimal(gs.sampled_from(LargeFlag), lambda f: f.value.bit_count() > 1)
        == LargeFlag.bit0 | LargeFlag.bit1
    )


def test_flags_finds_all_bits_set():
    assert find_any(gs.sampled_from(LargeFlag), lambda f: f == ~LargeFlag(0))


def test_sample_unnamed_alias():
    assert find_any(gs.sampled_from(UnnamedFlag), lambda f: f == UnnamedFlag.b)


def test_shrink_to_named_empty():
    assert minimal(gs.sampled_from(UnnamedFlag)) == UnnamedFlag(0)
