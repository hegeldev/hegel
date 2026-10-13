# Dropped from the Hypothesis original: test_notes_high_overrun_rates_in_unsatisfiable_error
# (relies on the internal buffer_size_limit of the conjecture engine);
# TestCases.test_abs_non_negative_varargs, test_abs_non_negative_varargs_kwargs,
# test_abs_non_negative_varargs_kwargs_only, test_can_test_kwargs_only_methods,
# test_can_call_an_argument_f (@given's argument binding).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import functools
import threading
from collections import namedtuple

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.utils import (
    assert_failing_output,
    capture_out,
    fails,
    fails_with,
    no_shrink,
    notes,
    skipif_emscripten,
)


@hegel.test
def test_int_addition_is_commutative(tc):
    x = tc.draw(gs.integers())
    y = tc.draw(gs.integers())
    assert x + y == y + x


@fails
@hegel.test
def test_str_addition_is_commutative(tc):
    x = tc.draw(gs.text(min_size=1))
    y = tc.draw(gs.text(min_size=1))
    assert x + y == y + x


@fails
@hegel.test
def test_bytes_addition_is_commutative(tc):
    # We enforce min_size=1 here to avoid a rare flakiness, where the
    # test passes if x and/or y are b"" for every generated example.
    # (the internal implementation makes this less rare for bytes)
    x = tc.draw(gs.binary(min_size=1))
    y = tc.draw(gs.binary(min_size=1))
    assert x + y == y + x


@hegel.test
def test_int_addition_is_associative(tc):
    x = tc.draw(gs.integers())
    y = tc.draw(gs.integers())
    z = tc.draw(gs.integers())
    assert x + (y + z) == (x + y) + z


@fails
@hegel.test(test_cases=2000)
def test_float_addition_is_associative(tc):
    x = tc.draw(gs.floats())
    y = tc.draw(gs.floats())
    z = tc.draw(gs.floats())
    assert x + (y + z) == (x + y) + z


@hegel.test
def test_reversing_preserves_integer_addition(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    assert sum(xs) == sum(reversed(xs))


def test_still_minimizes_on_non_assertion_failures():
    @hegel.test(test_cases=50)
    def is_not_too_large(tc):
        x = tc.draw(gs.integers())
        if x >= 10:
            raise ValueError(f"No, {x} is just too large. Sorry")

    with pytest.raises(ValueError, match=" 10 "):
        is_not_too_large()


@hegel.test
def test_integer_division_shrinks_positive_integers(tc):
    n = tc.draw(gs.integers())
    tc.assume(n > 0)
    assert n / 2 < n


class TestCases:
    @hegel.test
    def test_abs_non_negative(self, tc):
        x = tc.draw(gs.integers())
        assert abs(x) >= 0
        assert isinstance(self, TestCases)

    @fails
    @hegel.test
    def test_int_is_always_negative(self, tc):
        x = tc.draw(gs.integers())
        assert x < 0

    @fails
    @hegel.test
    def test_float_addition_cancels(self, tc):
        x = tc.draw(gs.floats())
        y = tc.draw(gs.floats())
        assert x + (y - x) == y


@fails
@hegel.test
def test_can_be_given_keyword_args(tc):
    x = tc.draw(gs.integers(min_value=0, max_value=3))
    name = tc.draw(gs.text())
    tc.assume(x > 0)
    assert len(name) < x


@fails
@hegel.test
def test_one_of_produces_different_values(tc):
    x = tc.draw(gs.one_of(gs.floats(), gs.booleans()))
    y = tc.draw(gs.one_of(gs.floats(), gs.booleans()))
    assert type(x) is type(y)


@hegel.test
def test_is_the_answer(tc):
    x = tc.draw(gs.just(42))
    assert x == 42


@hegel.test
def test_integers_are_in_range(tc):
    x = tc.draw(gs.integers(1, 10))
    assert 1 <= x <= 10


@hegel.test
def test_integers_from_are_from(tc):
    x = tc.draw(gs.integers(min_value=100))
    assert x >= 100


def test_does_not_catch_interrupt_during_falsify():
    called = False

    @hegel.test
    def flaky_base_exception(tc):
        nonlocal called
        tc.draw(gs.integers())
        if not called:
            called = True
            raise KeyboardInterrupt

    with pytest.raises(KeyboardInterrupt):
        flaky_base_exception()


@hegel.test
def test_removing_an_element_from_a_unique_list(tc):
    xs = tc.draw(gs.lists(gs.integers(), unique=True))
    y = tc.draw(gs.integers())
    tc.assume(len(set(xs)) == len(xs))

    try:
        xs.remove(y)
    except ValueError:
        pass

    assert y not in xs


@fails
@hegel.test
def test_removing_an_element_from_a_non_unique_list(tc):
    xs = tc.draw(gs.lists(gs.integers(), min_size=2))
    y = tc.draw(gs.sampled_from(xs))
    xs.remove(y)
    assert y not in xs


@hegel.test
def test_can_test_sets_sampled_from(tc):
    xs = tc.draw(gs.sets(gs.sampled_from(list(range(10)))))
    assert all(isinstance(x, int) for x in xs)
    assert all(0 <= x < 10 for x in xs)


mix = gs.one_of(gs.sampled_from([1, 2, 3]), gs.text())


@fails
@hegel.test
def test_can_mix_sampling_with_generating(tc):
    x = tc.draw(mix)
    y = tc.draw(mix)
    assert type(x) is type(y)


@fails
@hegel.test
def test_can_find_large_sum_frozenset(tc):
    xs = tc.draw(gs.frozensets(gs.integers()))
    assert sum(xs) < 100


def test_prints_on_failure_by_default():
    @hegel.test(test_cases=1000)
    def test_ints_are_sorted(tc):
        balthazar = tc.draw(gs.integers())
        evans = tc.draw(gs.integers())
        tc.assume(evans >= 0)
        assert balthazar <= evans

    assert_failing_output(test_ints_are_sorted, balthazar=1, evans=0)


def test_does_not_print_on_success():
    @hegel.test(verbosity=hegel.Verbosity.normal)
    def test_is_an_int(tc):
        tc.draw(gs.integers())
        return

    with capture_out() as out:
        test_is_an_int()
    out = out.getvalue()
    lines = [l.strip() for l in out.split("\n")]
    assert all(not l for l in lines), lines


@hegel.test
def test_can_sample_from_single_element(tc):
    x = tc.draw(gs.sampled_from([1]))
    assert x == 1


@fails
@hegel.test
def test_list_is_sorted(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    assert sorted(xs) == xs


@fails
@hegel.test
def test_is_an_endpoint(tc):
    x = tc.draw(gs.floats(1.0, 2.0))
    assert x in {1.0, 2.0}


def test_breaks_bounds():
    @fails
    @hegel.test(derandomize=True, test_cases=10_000)
    def test_is_bounded(tc, t):
        x = tc.draw(gs.integers())
        assert x < t

    for t in [1, 10, 100, 1000]:
        test_is_bounded(t)


@fails_with(UnicodeEncodeError)
@hegel.test(test_cases=100)
def test_is_ascii(tc):
    x = tc.draw(gs.text())
    x.encode("ascii")


@fails
@hegel.test
def test_is_not_ascii(tc):
    x = tc.draw(gs.text())
    try:
        x.encode("ascii")
        raise AssertionError
    except UnicodeEncodeError:
        pass


@fails
@hegel.test(test_cases=100, derandomize=True)
def test_can_find_string_with_duplicates(tc):
    s = tc.draw(gs.text(min_size=2))
    assert len(set(s)) == len(s)


@fails
@hegel.test(derandomize=True)
def test_has_ascii(tc):
    x = tc.draw(gs.text(min_size=1))
    if not x:
        return
    ascii_characters = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ \t\n"
    assert any(c in ascii_characters for c in x)


def test_can_derandomize():
    values = []

    @fails
    @hegel.test(derandomize=True, database="")
    def test_blah(tc):
        x = tc.draw(gs.integers())
        values.append(x)
        assert x > 0

    test_blah()
    assert values
    v1 = values
    values = []
    test_blah()
    assert v1 == values


def test_can_run_without_database():
    @hegel.test(database="")
    def test_blah(tc):
        tc.draw(gs.integers())
        raise AssertionError

    with pytest.raises(AssertionError):
        test_blah()


@skipif_emscripten
def test_can_run_with_database_in_thread():
    results = []

    @hegel.test
    def test_blah(tc):
        tc.draw(gs.integers())
        raise ValueError

    def run_test():
        try:
            test_blah()
        except ValueError:
            results.append("success")

    # Run once in the main thread and once in another thread. Execution is
    # strictly serial, so no need for locking.
    run_test()
    assert results == ["success"]
    thread = threading.Thread(target=run_test)
    thread.start()
    thread.join()
    assert results == ["success", "success"]


Litter = namedtuple("Litter", ("kitten1", "kitten2"))


@hegel.test
def test_named_tuples_are_of_right_type(tc):
    litter = tc.draw(gs.builds(Litter, gs.integers(), gs.integers()))
    assert isinstance(litter, Litter)


@fails_with(AttributeError)
@hegel.test(suppress_health_check=hegel.HealthCheck.all())
def test_fails_in_reify(tc):
    tc.draw(gs.integers().map(lambda x: x.nope))


@hegel.test
def test_a_text(tc):
    x = tc.draw(gs.text("a"))
    assert set(x).issubset(set("a"))


@hegel.test
def test_empty_text(tc):
    x = tc.draw(gs.text(""))
    assert not x


@hegel.test
def test_mixed_text(tc):
    x = tc.draw(gs.text("abcdefg"))
    assert set(x).issubset(set("abcdefg"))


def test_when_set_to_no_simplifies_runs_failing_example_twice():
    failing = []

    @hegel.test(
        phases=no_shrink,
        test_cases=100,
        verbosity=hegel.Verbosity.normal,
        report_multiple_failures=False,
    )
    def foo(tc):
        x = tc.draw(gs.integers())
        if x > 11:
            tc.note("Lo")
            failing.append(x)
            raise AssertionError

    with pytest.raises(AssertionError) as err:
        foo()
    # Hegel confirms a failure by replaying it before the final replay, so
    # the example runs more than Hypothesis's twice; it is still never shrunk.
    assert len(failing) >= 2
    assert len(set(failing)) == 1
    assert "Falsifying example" in "\n".join(notes(err.value))
    assert "Lo" in "\n".join(notes(err.value))


@hegel.test
def test_filtered_values_satisfy_condition(tc):
    i = tc.draw(gs.integers().filter(lambda x: x % 4 == 0))
    assert i % 4 == 0


def nameless_const(x):
    def f(u, v):
        return u

    return functools.partial(f, x)


@hegel.test
def test_can_map_nameless(tc):
    x = tc.draw(gs.sets(gs.booleans()).map(nameless_const(2)))
    assert x == 2


@hegel.test
def test_can_flatmap_nameless(tc):
    x = tc.draw(gs.integers(0, 10).flat_map(nameless_const(gs.just(3))))
    assert x == 3


def test_can_be_used_with_none_module():
    def test_is_cool(tc):
        tc.draw(gs.integers())

    test_is_cool.__module__ = None
    test_is_cool = hegel.test(test_is_cool)
    test_is_cool()


def test_does_not_print_notes_if_all_succeed():
    @hegel.test(verbosity=hegel.Verbosity.normal)
    def test(tc):
        tc.draw(gs.integers())
        tc.note("Hi there")

    with capture_out() as out:
        test()
    assert not out.getvalue()


def test_prints_notes_once_on_failure():
    @hegel.test(database="", verbosity=hegel.Verbosity.normal)
    def test(tc):
        xs = tc.draw(gs.lists(gs.integers()))
        tc.note("Hi there")
        if sum(xs) <= 100:
            raise ValueError

    with pytest.raises(ValueError) as err:
        test()
    assert "\n".join(notes(err.value)).count("Hi there") == 1


@hegel.test
def test_empty_lists(tc):
    xs = tc.draw(gs.lists(gs.integers(), max_size=0))
    assert xs == []


def test_given_usable_inline_on_lambdas():
    xs = []
    hegel.test(lambda tc: xs.append(tc.draw(gs.booleans())))()
    assert set(xs) == {False, True}


@pytest.mark.xfail(
    strict=True,
    reason="engine: with the FilterTooMuch health check suppressed, a run whose every "
    "case is rejected ends quietly as passed instead of raising Unsatisfiable",
)
def test_notes_high_filter_rates_in_unsatisfiable_error():
    @hegel.test(suppress_health_check=[hegel.HealthCheck.filter_too_much])
    def f(tc):
        tc.draw(gs.integers())
        tc.assume(False)

    with pytest.raises(hegel.Unsatisfiable):
        f()


@pytest.mark.xfail(
    strict=True,
    reason="engine: a run whose every case is rejected fails the FilterTooMuch "
    "health check rather than raising Unsatisfiable",
)
def test_rejecting_every_input_is_unsatisfiable():
    @hegel.test
    def f(tc):
        tc.draw(gs.booleans())
        tc.assume(False)

    with pytest.raises(hegel.Unsatisfiable):
        f()
