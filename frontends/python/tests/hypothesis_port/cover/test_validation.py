# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_errors_when_given_varargs,
# test_varargs_without_positional_arguments_allowed,
# test_errors_when_given_varargs_and_kwargs_with_positional_arguments,
# test_varargs_and_kwargs_without_positional_arguments_allowed,
# test_bare_given_errors, test_errors_on_unwanted_kwargs,
# test_errors_on_too_many_positional_args, test_errors_on_any_varargs,
# test_can_put_arguments_in_the_middle, test_stuff_keyword,
# test_stuff_positional, test_too_many_positional,
# test_given_warns_on_use_of_non_strategies,
# test_given_warns_when_mixing_positional_with_keyword (@given argument
# binding); test_cannot_find_non_strategies (find());
# test_check_type_with_tuple_of_length_two, test_check_type_suggests_check_strategy,
# test_check_strategy_might_suggest_sampled_from (check_type/check_strategy
# internals); test_warn_on_strings_matching_common_codecs (HypothesisWarning).
# test_validation_happens_on_draw (lists(nothing(), min_size=1) is not an
# error in Hegel).

import functools

import pytest

from hegel import generators as gs
from hegel.errors import InvalidArgument
from hegel.generators import (
    binary,
    dicts,
    floats,
    frozensets,
    integers,
    lists,
    recursive,
    sets,
    text,
)
from tests.hypothesis_port.common.debug import check_can_generate_examples, find_any


def test_float_ranges():
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(floats(float("nan"), 0))
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(floats(1, -1))


def test_float_range_and_allow_nan_cannot_both_be_enabled():
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(floats(min_value=1, allow_nan=True))
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(floats(max_value=1, allow_nan=True))


def test_float_finite_range_and_allow_infinity_cannot_both_be_enabled():
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(floats(0, 1, allow_infinity=True))


def test_does_not_error_if_min_size_is_bigger_than_default_size():
    find_any(lists(integers(), min_size=50))
    find_any(sets(integers(), min_size=50))
    find_any(frozensets(integers(), min_size=50))
    find_any(lists(integers(), min_size=50, unique=True))


def test_list_unique_and_unique_by_cannot_both_be_enabled():
    # Hegel rejects the combination when the generator is built.
    with pytest.raises(InvalidArgument) as e:
        lists(integers(), unique=True, unique_by=lambda x: x)
    assert "unique " in e.value.args[0]
    assert "unique_by" in e.value.args[0]


def test_min_before_max():
    with pytest.raises(InvalidArgument):
        integers(min_value=1, max_value=0)


def test_filter_validates():
    with pytest.raises(InvalidArgument):
        integers(min_value=1, max_value=0).filter(bool)


def test_recursion_validates_base_case():
    with pytest.raises(InvalidArgument):
        recursive(integers(min_value=1, max_value=0), lists)


def test_recursion_validates_recursive_step():
    # The branch function is only called when a value is drawn, so the
    # error surfaces then rather than when the generator is built.
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(
            recursive(integers(), lambda x: lists(x, min_size=3, max_size=1))
        )


@pytest.mark.parametrize(
    "generator",
    [
        functools.partial(lists, elements=integers()),
        functools.partial(dicts, keys=integers(), values=integers()),
        text,
        binary,
    ],
)
@pytest.mark.parametrize("min_size,max_size", [(0, "10"), ("0", 10)])
def test_valid_sizes(generator, min_size, max_size):
    # Hegel validates sizes when the generator is built, not when it is drawn.
    with pytest.raises(InvalidArgument):
        generator(min_size=min_size, max_size=max_size)


def test_text_alphabet_must_be_collection_or_generator():
    with pytest.raises(InvalidArgument, match="alphabet"):
        gs.text(alphabet=123)
