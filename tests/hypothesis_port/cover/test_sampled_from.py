# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_transformed_just_strategy
# (JustStrategy internals); test_unsat_filtered_sampling_in_rejection_stage
# (SampledFromStrategy internal early-exit branch);
# TestErrorNoteBehavior3819 (the "sampled_from was given a collection of
# strategies" error note, which Hegel does not add);
# test_efficient_sets_of_samples_with_chained_transformations_slow_path
# (carrying .map()/.filter() through a sampled_from's element narrowing).

import collections
import enum

import pytest

import hegel
from hegel import generators as gs
from hegel.generators import sampled_from
from tests.hypothesis_port.common.debug import (
    assert_all_examples,
    assert_simple_property,
    check_can_generate_examples,
)
from tests.hypothesis_port.common.utils import fails_with

an_enum = enum.Enum("A", "a b c")
a_flag = enum.Flag("A", "a b c")
# named zero state is required for empty flags from around py3.11/3.12
an_empty_flag = enum.Flag("EmptyFlag", {"a": 0})

an_ordereddict = collections.OrderedDict([("a", 1), ("b", 2), ("c", 3)])


@fails_with(hegel.InvalidArgument)
def test_cannot_sample_sets():
    check_can_generate_examples(sampled_from(set("abc")))


def test_can_sample_sequence_without_warning():
    check_can_generate_examples(sampled_from([1, 2, 3]))


def test_can_sample_ordereddict_without_warning():
    check_can_generate_examples(sampled_from(an_ordereddict))


@pytest.mark.parametrize(
    "enum_class",
    [
        an_enum,
        a_flag,
        an_empty_flag,
    ],
)
def test_can_sample_enums(enum_class):
    assert_all_examples(sampled_from(enum_class), lambda x: isinstance(x, enum_class))


@fails_with(hegel.FailedHealthCheck)
@hegel.test(suppress_health_check=[])
def test_unsat_filtered_sampling(tc):
    tc.draw(sampled_from(range(10)).filter(lambda x: x < 0))
    raise AssertionError


def test_easy_filtered_sampling():
    assert_simple_property(sampled_from(range(100)).filter(lambda x: x == 0), lambda x: x == 0)


@hegel.test
def test_filtered_sampling_finds_rare_value(tc):
    x = tc.draw(sampled_from(range(100)).filter(lambda x: x == 99))
    assert x == 99


@hegel.test
def test_efficient_sets_of_samples(tc):
    x = tc.draw(gs.sets(gs.sampled_from(range(50)), min_size=50))
    assert x == set(range(50))


@hegel.test
def test_efficient_dicts_with_sampled_keys(tc):
    x = tc.draw(gs.dicts(gs.sampled_from(range(50)), gs.none(), min_size=50))
    assert set(x) == set(range(50))


@hegel.test
def test_does_not_include_duplicates_even_when_duplicated_in_collection(tc):
    ls = tc.draw(gs.lists(gs.sampled_from([0] * 100), unique=True))
    assert len(ls) <= 1


@hegel.test
def test_efficient_sets_of_samples_with_chained_transformations(tc):
    x = tc.draw(
        gs.sets(
            gs.sampled_from(range(50))
            .map(lambda x: x * 2)
            .filter(lambda x: x % 3)
            .map(lambda x: x // 2),
            min_size=33,
        )
    )
    assert x == {x for x in range(50) if (x * 2) % 3}


@pytest.mark.xfail(
    strict=True,
    reason="Engine: a sampled_from whose every element is filtered out fails FilterTooMuch; "
    "Unsatisfiable is only raised when the test draws no data",
    raises=hegel.FailedHealthCheck,
)
@fails_with(hegel.Unsatisfiable)
@hegel.test
def test_unsatisfiable_explicit_filteredstrategy_sampled(tc):
    tc.draw(gs.sampled_from([None, False, ""]).filter(bool))
    raise AssertionError("Unreachable because there are no valid examples")


@fails_with(hegel.Unsatisfiable)
@hegel.test
def test_unsatisfiable_explicit_filteredstrategy_just(tc):
    tc.draw(gs.none().filter(bool))
    raise AssertionError("Unreachable because there are no valid examples")


@hegel.test
def test_max_size_is_respected_with_unique_sampled_from(tc):
    ls = tc.draw(gs.lists(gs.sampled_from(range(100)), max_size=3, unique=True))
    assert len(ls) <= 3


@hegel.test
def test_issue_2247_regression(tc):
    ls = tc.draw(gs.lists(gs.sampled_from([0, 0.0]), unique=True, min_size=1))
    assert len(ls) == 1


@hegel.test
def test_mutability_1(tc):
    # See https://github.com/HypothesisWorks/hypothesis/issues/2507
    x = [1]
    s = gs.sampled_from(x)
    x.append(2)
    assert tc.draw(s) != 2


@hegel.test
def test_mutability_2(tc):
    x = [1]
    s = gs.sampled_from(x)
    assert tc.draw(s) != 2
    x.append(2)
    assert tc.draw(s) != 2


class AnnotationsInsteadOfElements(enum.Enum):
    a: "int"


def test_rejects_enum_with_annotations_instead_of_elements():
    # Hypothesis's message points at the annotations; hegel's says the
    # sequence is empty.
    with pytest.raises(hegel.InvalidArgument, match=r"Cannot sample"):
        check_can_generate_examples(gs.sampled_from(AnnotationsInsteadOfElements))
