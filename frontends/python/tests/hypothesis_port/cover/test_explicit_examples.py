# Dropped from the Hypothesis original: TestInstanceMethods.test_hi_3,
# test_no_args_and_kwargs, test_no_empty_examples (@given's argument binding; Hegel
# accepts mixed and empty explicit cases);
# test (deadline); test_simplifies_multiple_examples_with_same_error,
# test_shows_all_examples_at_verbose, test_different_errors_not_simplified
# (Hypothesis's grouping of failing explicit examples); test_example_decorator_accepts_strategies,
# test_helpful_message_when_example_fails_because_it_was_passed_a_strategy
# (Hypothesis's warning about a strategy passed to @example);
# test_stop_silently_dropping_examples_when_decorator_is_applied_to_itself
# (Hypothesis's guard against @example applied to @example);
# test_prints_verbose_output_for_explicit_examples (a verbose trace of explicit cases);
# the ExceptionGroup case of test_multiple_example_reporting (MultipleFailures for
# explicit cases).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

from unittest import TestCase

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.utils import assert_failing_output, capture_out, notes


class TestInstanceMethods(TestCase):
    @hegel.test
    @hegel.explicit_test_case(1)
    def test_hi_1(self, tc):
        x = tc.draw(gs.integers())
        assert isinstance(x, int)

    @hegel.test
    @hegel.explicit_test_case(x=1)
    def test_hi_2(self, tc):
        x = tc.draw(gs.integers())
        assert isinstance(x, int)


def test_kwarg_example_on_testcase():
    class Stuff(TestCase):
        @hegel.test
        @hegel.explicit_test_case(x=1)
        def test_hi(self, tc):
            x = tc.draw(gs.integers())
            assert isinstance(x, int)

    Stuff("test_hi").test_hi()


def test_errors_when_run_with_not_enough_args():
    @hegel.test
    @hegel.explicit_test_case(1)
    def foo(tc):
        tc.draw(gs.integers())
        tc.draw(gs.integers())

    with pytest.raises(hegel.InvalidArgument):
        foo()


def test_errors_when_run_with_not_enough_kwargs():
    @hegel.test
    @hegel.explicit_test_case(x=1)
    def foo(tc):
        x = tc.draw(gs.integers())
        y = tc.draw(gs.integers())
        assert isinstance((x, y), tuple)

    with pytest.raises(hegel.InvalidArgument):
        foo()


def test_must_agree_with_number_of_arguments():
    @hegel.test
    @hegel.explicit_test_case(1, 2)
    def test(tc):
        tc.draw(gs.integers())

    with pytest.raises(hegel.InvalidArgument):
        test()


def test_can_use_examples_after_given():
    long_str = "This is a very long string that you've no chance of hitting"

    @hegel.explicit_test_case(long_str)
    @hegel.test
    def test_not_long_str(tc):
        x = tc.draw(gs.text())
        assert x != long_str

    with pytest.raises(AssertionError):
        test_not_long_str()


def test_can_use_examples_before_given():
    long_str = "This is a very long string that you've no chance of hitting"

    @hegel.test
    @hegel.explicit_test_case(long_str)
    def test_not_long_str(tc):
        x = tc.draw(gs.text())
        assert x != long_str

    with pytest.raises(AssertionError):
        test_not_long_str()


def test_can_use_examples_around_given():
    long_str = "This is a very long string that you've no chance of hitting"
    short_str = "Still no chance"

    seen = []

    @hegel.explicit_test_case(short_str)
    @hegel.test
    @hegel.explicit_test_case(long_str)
    def test_not_long_str(tc):
        x = tc.draw(gs.text())
        seen.append(x)

    test_not_long_str()
    assert set(seen[:2]) == {long_str, short_str}


@pytest.mark.parametrize(("x", "y"), [(1, False), (2, True)])
@hegel.test
@hegel.explicit_test_case(z=10)
def test_is_a_thing(tc, x, y):
    z = tc.draw(gs.integers())
    assert isinstance(z, int)


def test_does_not_print_on_explicit_examples_if_no_failure():
    @hegel.test
    @hegel.explicit_test_case(1)
    def test_positive(tc):
        x = tc.draw(gs.integers())
        assert x > 0

    with pytest.raises(AssertionError), capture_out() as out:
        test_positive()
    out = out.getvalue()
    assert "x = 1" not in out


def test_prints_output_for_explicit_examples():
    @hegel.test
    @hegel.explicit_test_case(-1)
    def test_positive(tc):
        x = tc.draw(gs.integers())
        assert x > 0

    assert_failing_output(test_positive, "Falsifying example", x=-1)


def test_captures_original_repr_of_example():
    @hegel.test
    @hegel.explicit_test_case(x=[])
    def test_mutation(tc):
        x = tc.draw(gs.lists(gs.integers()))
        x.append(1)
        assert not x

    assert_failing_output(test_mutation, "Falsifying example", x=[])


def test_examples_are_tried_in_order():
    @hegel.explicit_test_case(x=1)
    @hegel.explicit_test_case(x=2)
    @hegel.test(phases=[hegel.Phase.explicit])
    @hegel.explicit_test_case(x=3)
    def test(tc):
        x = tc.draw(gs.integers())
        print(f"x -> {x}")

    with capture_out() as out:
        test()
    ls = out.getvalue().splitlines()
    assert ls == ["x -> 1", "x -> 2", "x -> 3"]


def test_prints_note_in_failing_example():
    @hegel.test
    @hegel.explicit_test_case(x=42)
    @hegel.explicit_test_case(x=43)
    def test(tc):
        x = tc.draw(gs.integers())
        tc.note(f"x -> {x}")
        assert x == 42

    with pytest.raises(AssertionError) as err:
        test()
    assert "x -> 43" in "\n".join(notes(err.value))
    assert "x -> 42" not in "\n".join(notes(err.value))


@pytest.mark.parametrize("threshold", [0.5, 1])
def test_unsatisfied_assumption_during_explicit_example(threshold):
    # Regression test for https://github.com/HypothesisWorks/hypothesis/issues/2125.
    # Hegel treats an assumption that an explicit test case violates as a
    # mistake in the explicit case, rather than skipping it.
    @hegel.test
    @hegel.explicit_test_case(value=0.56789)
    def test(tc):
        value = tc.draw(gs.floats(0, 1))
        tc.assume(value < threshold)

    if threshold > 0.56789:
        test()
    else:
        with pytest.raises(hegel.InvalidArgument, match="Explicit test cases must not be rejected"):
            test()


def test_multiple_example_reporting():
    @hegel.test(phases=[hegel.Phase.explicit])
    @hegel.explicit_test_case(1)
    @hegel.explicit_test_case(2)
    def inner_test_multiple_failing_examples(tc):
        x = tc.draw(gs.integers())
        assert x < 2
        assert x < 1

    with pytest.raises(AssertionError):
        inner_test_multiple_failing_examples()
