# Dropped from the Hypothesis original: test_is_not_normally_default,
# test_settings_are_default_in_given (the settings.default descriptor);
# test_non_executed_tests_raise_skipped (SkipTest for a run with no phase to run).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.
# test_validates_strategies_for_test_method (lists(nothing(), min_size=1) is
# not an error in Hegel: nothing() only rejects when drawn).

import pytest
from _pytest.outcomes import Failed, Skipped

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import minimal


def test_stops_after_max_examples_if_satisfying():
    count = 0

    def track(x):
        nonlocal count
        count += 1
        return False

    test_cases = 100

    with pytest.raises(hegel.Unsatisfiable):
        minimal(gs.integers(0, 10000), track, test_cases=test_cases)

    assert count == test_cases


def test_stops_after_ten_times_max_examples_if_not_satisfying():
    count = 0

    def track(tc):
        nonlocal count
        count += 1
        tc.draw(gs.integers(0, 10000))
        tc.reject()

    test_cases = 100
    with pytest.raises(hegel.FailedHealthCheck):
        hegel.run(track, test_cases=test_cases)

    # Very occasionally we can generate overflows in generation, which also
    # count towards our example budget, which means that we don't hit the
    # maximum.
    assert count <= 10 * test_cases


def test_given_shrinks_pytest_helper_errors():
    final_value = None

    @hegel.test(derandomize=True, test_cases=100)
    def inner(tc):
        nonlocal final_value
        x = tc.draw(gs.integers())
        final_value = x
        if x > 100:
            pytest.fail(f"{x=} is too big!")

    with pytest.raises(Failed):
        inner()
    assert final_value == 101


def test_pytest_skip_skips_shrinking():
    seen_large = False

    @hegel.test(derandomize=True, test_cases=100)
    def inner(tc):
        nonlocal seen_large
        x = tc.draw(gs.integers())
        if x > 100:
            if seen_large:
                raise Exception("Should never replay a skipped test!")
            seen_large = True
            pytest.skip(f"{x=} is too big!")

    with pytest.raises(Skipped):
        inner()


def test_can_find_with_db_eq_none():
    minimal(gs.integers(), bool, database="", test_cases=100)


def test_no_such_example():
    with pytest.raises(hegel.Unsatisfiable):
        minimal(gs.none(), bool)


@pytest.mark.parametrize(
    "codec, max_codepoint, exclude_categories, categories",
    [
        ("ascii", None, None, None),
        ("ascii", 128, None, None),
        ("ascii", 100, None, None),
        ("utf-8", None, None, None),
        ("utf-8", None, ["Cs"], None),
        ("utf-8", None, ["N"], None),
        ("utf-8", None, None, ["N"]),
    ],
)
@hegel.test
def test_characters_codec(tc, codec, max_codepoint, exclude_categories, categories):
    generator = gs.characters(
        codec=codec,
        max_codepoint=max_codepoint,
        exclude_categories=exclude_categories,
        categories=categories,
    )
    example = tc.draw(generator)
    assert example.encode(encoding=codec).decode(encoding=codec) == example
