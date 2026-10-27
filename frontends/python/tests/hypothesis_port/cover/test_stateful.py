# Dropped from the Hypothesis original:
# test_unique_lists_of_bundle_values_use_the_generic_unique_strategy (inspects Hypothesis's
# internal strategy classes);
# test_picks_up_settings_at_first_use_of_testcase, test_can_get_test_case_off_machine_instance,
# test_settings_attribute_is_validated, test_fails_on_settings_class_attribute
# (MachineClass.TestCase and its settings attribute);
# test_flaky_draw_less_raises_flaky, test_flaky_raises_flaky, test_flaky_precondition_error_message,
# test_flaky_draw_in_rule_no_precondition_note (current_build_context);
# test_get_state_machine_test_is_importable (Hypothesis internal API);
# test_multiple, test_multiple_variables_printed, test_multiple_variables_printed_single_element,
# test_no_variables_printed, test_single_target_multiple, test_targets_repr, test_multiple_targets,
# test_multiple_common_targets (multiple() and rule targets);
# test_consumes_typecheck, test_deprecated_target_consumes_bundle,
# test_bundle_repr_includes_transformations, test_rule_deprecation_targets_and_target,
# test_rule_deprecation_bundle_by_name, test_rule_non_bundle_target,
# test_rule_non_bundle_target_oneof (the Bundle/consumes API);
# test_settings_argument_is_validated, test_stateful_double_rule_is_forbidden,
# test_no_double_invariant, test_invariant_and_rule_are_incompatible, test_invalid_rule_argument,
# test_invalid_initialize_argument (Hypothesis's definition-time validation);
# test_can_explicitly_call_functions_when_precondition_not_satisfied, test_invariant_precondition,
# test_explicit_invariant_call_with_precondition, test_precondition_cannot_be_used_without_rule
# (@precondition);
# test_initialize_rule, test_initialize_rule_populate_bundle,
# test_initialize_rule_dont_mix_with_precondition, test_initialize_rule_dont_mix_with_regular_rule,
# test_initialize_rule_cannot_be_double_applied, test_can_manually_call_initialize_rule,
# test_invariants_are_checked_after_init_steps, test_invariants_can_be_checked_during_init_steps,
# test_check_during_init_must_be_boolean (@initialize and check_during_init);
# test_arguments_do_not_use_names_of_return_values,
# test_prints_variable_names_for_filtered_bundle_arguments,
# test_prints_variable_names_for_consumed_filtered_bundle_arguments,
# test_prints_mapped_bundle_values_directly (Hypothesis's printed program with bundle variable
# names);
# test_uses_seed (@seed);
# test_min_steps_argument (minimum step count);
# test_minimizes_errors_in_teardown (teardown).
# Rewritten: test_initialize_rule_in_state_machine_with_inheritance as
# test_rules_in_state_machine_with_inheritance (rules instead of @initialize);
# test_prints_equal_values_with_correct_variable_name as
# test_shrinks_moves_between_pools_to_one_step_each (step counts instead of variable names).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import os
import re
from collections import defaultdict
from typing import ClassVar

import pytest
from _pytest.outcomes import Failed, Skipped
from pytest import raises

import hegel
from hegel import generators as gs
from hegel.errors import DidNotReproduce, FailedHealthCheck, Flaky, InvalidArgument
from tests.hypothesis_port.common.utils import capture_out, notes
from tests.hypothesis_port.nocover.test_stateful import DepthMachine


def _run(machine, **settings):
    """Run ``machine`` as a Hegel state machine test."""
    hegel.state_machine_test(machine, **settings)()


def _failure_text(err):
    """The falsifying example Hegel attached to ``err``, as one string."""
    return "\n".join(notes(err)).strip()


class MultipleRulesSameFuncMachine(hegel.StateMachine):
    def myfunc(self, data):
        print(data)

    rule1 = hegel.rule(name="rule1")(lambda self, tc: self.myfunc("rule1data"))
    rule2 = hegel.rule(name="rule2")(lambda self, tc: self.myfunc("rule2data"))


class PreconditionMachine(hegel.StateMachine):
    num = 0

    @hegel.rule()
    def add_one(self, tc):
        self.num += 1

    @hegel.rule()
    def set_to_zero(self, tc):
        self.num = 0

    @hegel.rule()
    def div_when_nonzero(self, tc):
        tc.assume(self.num != 0)
        num = tc.draw(gs.integers())
        self.num = num / self.num


test_PreconditionMachine = hegel.state_machine_test(PreconditionMachine, test_cases=10)


def test_multiple_rules_same_func():
    with capture_out() as o:
        _run(MultipleRulesSameFuncMachine, test_cases=10)
    output = o.getvalue()
    assert "rule1data" in output
    assert "rule2data" in output


def test_result_is_added_to_target():
    class TargetStateMachine(hegel.StateMachine):
        def __init__(self, tc):
            self.nodes = hegel.Pool(tc)

        @hegel.rule()
        def bunch(self, tc):
            source = tc.draw(gs.lists(self.nodes.values_reusable()))
            assert len(source) == 0
            self.nodes.add(source)

    with raises(AssertionError) as err:
        _run(TargetStateMachine)
    # The value added by the first step is what the failing step draws.
    assert re.search(r"Step \d+: bunch \{\n  source = \[\[\]\]\n\}", _failure_text(err.value))


class FlakyRatchettingMachine(hegel.StateMachine):
    ratchet = 0

    @hegel.rule()
    def action(self, tc):
        FlakyRatchettingMachine.ratchet += 1
        n = FlakyRatchettingMachine.ratchet
        tc.draw(gs.lists(gs.integers(), min_size=n, max_size=n))
        raise AssertionError


class MachineWithConsumingRule(hegel.StateMachine):
    steps = 10

    def __init__(self, tc):
        self.b1 = hegel.Pool(tc)
        self.b2 = hegel.Pool(tc)
        self.created_counter = 0
        self.consumed_counter = 0

    @hegel.invariant()
    def bundle_length(self, tc):
        assert len(self.b1) == self.created_counter - self.consumed_counter

    @hegel.rule()
    def populate_b1(self, tc):
        self.created_counter += 1
        self.b1.add(self.created_counter)

    @hegel.rule()
    def depopulate_b1(self, tc):
        consumed = tc.draw(self.b1.values_consumed())
        self.consumed_counter += 1
        self.b2.add(consumed)

    @hegel.rule()
    def depopulate_b1_multiple(self, tc):
        n = tc.draw(gs.integers(0, 3))
        tc.assume(n <= len(self.b1))
        for _ in range(n):
            tc.draw(self.b1.values_consumed())
        self.consumed_counter += n

    @hegel.rule()
    def check(self, tc):
        value1 = tc.draw(self.b1.values_reusable())
        value2 = tc.draw(self.b2.values_reusable())
        assert value1 != value2


test_MachineWithConsumingRule = hegel.state_machine_test(
    MachineWithConsumingRule,
    test_cases=30,
    suppress_health_check=[hegel.HealthCheck.filter_too_much],
)


class MachineUsingMultiple(hegel.StateMachine):
    def __init__(self, tc):
        self.b = hegel.Pool(tc)
        self.expected_bundle_length = 0

    @hegel.invariant()
    def bundle_length(self, tc):
        assert len(self.b) == self.expected_bundle_length

    @hegel.rule()
    def populate_bundle(self, tc):
        items = tc.draw(gs.lists(gs.integers(), max_size=10))
        self.expected_bundle_length += len(items)
        for item in items:
            self.b.add(item)

    @hegel.rule()
    def do_not_populate(self, tc):
        pass


test_MachineUsingMultiple = hegel.state_machine_test(MachineUsingMultiple)


def test_ratchetting_raises_flaky():
    with raises(Flaky):
        _run(FlakyRatchettingMachine, nondeterminism_strictness="error")


def test_empty_machine_is_invalid():
    class EmptyMachine(hegel.StateMachine):
        pass

    with raises(InvalidArgument):
        _run(EmptyMachine)


def test_machine_with_no_terminals_fails_health_check():
    # Hypothesis rejects this machine at definition time; Hegel keeps
    # rejecting the only rule (its pool is never filled) until the filter
    # health check fires.
    class NonTerminalMachine(hegel.StateMachine):
        def __init__(self, tc):
            self.hi = hegel.Pool(tc)

        @hegel.rule()
        def bye(self, tc):
            tc.draw(self.hi.values_reusable())

    with raises(FailedHealthCheck, match="FilterTooMuch"):
        _run(NonTerminalMachine)


class RequiresInit(hegel.StateMachine):
    def __init__(self, threshold):
        self.threshold = threshold

    @hegel.rule()
    def action(self, tc):
        value = tc.draw(gs.integers())
        if value > self.threshold:
            raise ValueError(f"{value} is too high")


def test_can_use_factory_for_tests():
    with raises(ValueError):
        hegel.run(lambda tc: hegel.run_state_machine(tc, RequiresInit(42)), test_cases=100)


class FailsEventually(hegel.StateMachine):
    steps = 5

    def __init__(self):
        self.counter = 0

    @hegel.rule()
    def increment(self, tc):
        self.counter += 1
        assert self.counter < 10


def test_can_explicitly_pass_settings():
    _run(FailsEventually)
    with raises(AssertionError):
        hegel.run(lambda tc: hegel.run_state_machine(tc, FailsEventually, steps=15))
    hegel.run(lambda tc: hegel.run_state_machine(tc, FailsEventually, steps=2))


def test_runner_that_checks_factory_produced_a_machine():
    with pytest.raises(InvalidArgument):
        hegel.run(lambda tc: hegel.run_state_machine(tc, object))


def test_saves_failing_example_in_database(tmp_path):
    with raises(AssertionError):
        _run(
            DepthMachine,
            database=str(tmp_path),
            test_cases=1000,
            suppress_health_check=hegel.HealthCheck.all(),
        )
    # Hegel's database is a directory; a saved failure leaves files in it.
    assert any(files for _, _, files in os.walk(tmp_path))


def test_can_run_with_no_db():
    with raises(AssertionError):
        _run(DepthMachine, database="", test_cases=10_000)


def test_invariant():
    """If an invariant raise an exception, the exception is propagated."""

    class Invariant(hegel.StateMachine):
        @hegel.invariant()
        def test_blah(self, tc):
            raise ValueError

        @hegel.rule()
        def do_stuff(self, tc):
            pass

    with pytest.raises(ValueError):
        _run(Invariant)


def test_multiple_invariants():
    """If multiple invariants are present, they all get run."""

    class Invariant(hegel.StateMachine):
        def __init__(self):
            self.first_invariant_ran = False

        @hegel.invariant()
        def invariant_1(self, tc):
            self.first_invariant_ran = True

        @hegel.invariant()
        def invariant_2(self, tc):
            if self.first_invariant_ran:
                raise ValueError

        @hegel.rule()
        def do_stuff(self, tc):
            pass

    with pytest.raises(ValueError):
        _run(Invariant)


def test_invariant_checks_initial_state():
    """Invariants are checked before any rules run."""

    class BadPrecondition(hegel.StateMachine):
        def __init__(self):
            self.num = 0

        @hegel.invariant()
        def test_blah(self, tc):
            if self.num == 0:
                raise ValueError

        @hegel.rule()
        def test_foo(self, tc):
            self.num += 1

    with pytest.raises(ValueError):
        _run(BadPrecondition)


def test_invariant_failling_present_in_falsifying_example():
    class BadInvariant(hegel.StateMachine):
        @hegel.invariant()
        def invariant_1(self, tc):
            raise ValueError

        @hegel.rule()
        def rule_1(self, tc):
            pass

    with pytest.raises(ValueError) as err:
        _run(BadInvariant)

    assert (
        _failure_text(err.value)
        == """
Falsifying example:
Checking invariants on the initial state.
Invariant invariant_1 failed:
""".strip()
    )


def test_invariant_present_in_falsifying_example():
    class BadRuleWithGoodInvariants(hegel.StateMachine):
        def __init__(self):
            self.num = 0

        @hegel.invariant()
        def invariant_1(self, tc):
            pass

        @hegel.invariant(always_run=True)
        def invariant_2(self, tc):
            pass

        @hegel.invariant()
        def invariant_3(self, tc):
            assert self.num >= 0

        @hegel.rule()
        def rule_1(self, tc):
            self.num += 1
            if self.num == 2:
                raise ValueError

    with pytest.raises(ValueError) as err:
        _run(BadRuleWithGoodInvariants, phases=set(hegel.Phase) - {hegel.Phase.shrink})

    # Passing invariants are not part of the printed run; the steps are.
    expected = """
Falsifying example:
Checking invariants on the initial state.
Step 1: rule_1 {
}
Step 2: rule_1 {
}
Rule rule_1 failed:
""".strip()
    assert _failure_text(err.value) == expected


def test_always_runs_at_least_one_step():
    counts = []

    class CountSteps(hegel.StateMachine):
        def __init__(self):
            self.count = 0
            counts.append(self)

        @hegel.rule()
        def do_something(self, tc):
            self.count += 1

    _run(CountSteps)
    assert counts
    assert all(machine.count > 0 for machine in counts)


def test_removes_needless_steps():
    """Regression test from an example based on
    tests/nocover/test_database_agreement.py, but without the expensive bits.
    Comparing two database implementations in which deletion is broken, so as
    soon as a key/value pair is successfully deleted the test will now fail if
    you ever check that key.

    The main interesting feature of this is that it has a lot of
    opportunities to generate keys and values before it actually fails,
    but will still fail with very high probability.
    """

    class IncorrectDeletion(hegel.StateMachine):
        def __init__(self, tc):
            self.__saved = defaultdict(set)
            self.__deleted = defaultdict(set)
            self.keys = hegel.Pool(tc)
            self.values = hegel.Pool(tc)

        @hegel.rule()
        def k(self, tc):
            self.keys.add(tc.draw(gs.binary()))

        @hegel.rule()
        def v(self, tc):
            self.values.add(tc.draw(gs.binary()))

        @hegel.rule()
        def save(self, tc):
            k = tc.draw(self.keys.values_reusable())
            v = tc.draw(self.values.values_reusable())
            self.__saved[k].add(v)

        @hegel.rule()
        def delete(self, tc):
            k = tc.draw(self.keys.values_reusable())
            v = tc.draw(self.values.values_reusable())
            if v in self.__saved[k]:
                self.__deleted[k].add(v)

        @hegel.rule()
        def values_agree(self, tc):
            k = tc.draw(self.keys.values_reusable())
            assert not self.__deleted[k]

    with pytest.raises(AssertionError) as err:
        _run(IncorrectDeletion, derandomize=True, test_cases=1000)

    result = _failure_text(err.value)
    assert len(re.findall(r"Step \d+: k \{", result)) == 1
    assert len(re.findall(r"Step \d+: v \{", result)) == 1


def test_shrinks_moves_between_pools_to_one_step_each():
    class MovesBetweenBundles(hegel.StateMachine):
        def __init__(self, tc):
            self.b1 = hegel.Pool(tc)
            self.b2 = hegel.Pool(tc)

        @hegel.rule()
        def create(self, tc):
            self.b1.add([])

        @hegel.rule()
        def transfer(self, tc):
            source = tc.draw(self.b1.values_reusable())
            self.b2.add(source)

        @hegel.rule()
        def fail(self, tc):
            tc.draw(self.b2.values_reusable())
            raise AssertionError

    with pytest.raises(AssertionError) as err:
        _run(MovesBetweenBundles, test_cases=100, suppress_health_check=hegel.HealthCheck.all())

    result = _failure_text(err.value)
    for m in ["create", "transfer", "fail"]:
        assert len(re.findall(rf"Step \d+: {m} \{{", result)) == 1
    assert "Step 1: create {" in result
    assert "Rule fail failed:" in result


def test_rules_in_state_machine_with_inheritance():
    class ParentStateMachine(hegel.StateMachine):
        seen: ClassVar = set()

        @hegel.rule()
        def rule_a(self, tc):
            self.seen.add("a")

    class ChildStateMachine(ParentStateMachine):
        @hegel.rule()
        def rule_b(self, tc):
            self.seen.add("b")

        @hegel.rule()
        def fail_when_both_ran(self, tc):
            assert self.seen != {"a", "b"}

    with pytest.raises(AssertionError):
        _run(ChildStateMachine)

    assert ChildStateMachine.seen == {"a", "b"}


def test_one_of_with_a_single_bundle_branch():
    class SingleBundleBranch(hegel.StateMachine):
        def __init__(self, tc):
            self.b = hegel.Pool(tc)

        @hegel.rule()
        def create(self, tc):
            self.b.add(1)

        @hegel.rule()
        def consume(self, tc):
            value = tc.draw(gs.one_of(self.b.values_reusable(), gs.nothing()))
            assert value == 1

    _run(SingleBundleBranch)


def test_steps_printed_despite_pytest_fail():
    # Test for https://github.com/HypothesisWorks/hypothesis/issues/1372
    class RaisesProblem(hegel.StateMachine):
        @hegel.rule()
        def oops(self, tc):
            pytest.fail("note that this raises a BaseException")

    with pytest.raises(Failed) as err:
        _run(RaisesProblem)
    assert (
        _failure_text(err.value)
        == """
Falsifying example:
Step 1: oops {
}
Rule oops failed:""".strip()
    )


def test_steps_not_printed_with_pytest_skip():
    class RaisesProblem(hegel.StateMachine):
        @hegel.rule()
        def skip_whole_test(self, tc):
            pytest.skip()

    with capture_out() as out, pytest.raises(Skipped):
        _run(RaisesProblem)
    assert "Step" not in out.getvalue()
    assert "Falsifying" not in out.getvalue()


def _failing_blob():
    # Hegel's reproduction blobs are engine-specific, so take one from a
    # real failure instead of encoding a choice sequence.
    class TrivialMachine(hegel.StateMachine):
        @hegel.rule()
        def oops(self, tc):
            raise AssertionError

    with pytest.raises(AssertionError) as err:
        _run(TrivialMachine, print_blob=True, database="")
    text = "\n".join(notes(err.value))
    match = re.search(r'reproduce_failure\("([^"]+)"\)', text)
    assert match, text
    return match.group(1)


def test_reproduce_failure_works():
    class TrivialMachine(hegel.StateMachine):
        @hegel.rule()
        def oops(self, tc):
            raise AssertionError

    test = hegel.reproduce_failure(_failing_blob())(
        hegel.state_machine_test(TrivialMachine, print_blob=True)
    )
    with pytest.raises(AssertionError):
        test()


def test_reproduce_failure_fails_if_no_error():
    class TrivialMachine(hegel.StateMachine):
        @hegel.rule()
        def ok(self, tc):
            pass

    test = hegel.reproduce_failure(_failing_blob())(
        hegel.state_machine_test(TrivialMachine, print_blob=True)
    )
    with pytest.raises(DidNotReproduce):
        test()


def test_cannot_have_zero_steps():
    class ZeroSteps(hegel.StateMachine):
        steps = 0

        @hegel.rule()
        def step(self, tc):
            pass

    with pytest.raises(InvalidArgument):
        _run(ZeroSteps)
    with pytest.raises(InvalidArgument):
        hegel.run(lambda tc: hegel.run_state_machine(tc, FailsEventually, steps=0))


class LotsOfEntropyPerStepMachine(hegel.StateMachine):
    # Regression tests for https://github.com/HypothesisWorks/hypothesis/issues/3618
    @hegel.rule()
    def rule1(self, tc):
        data = tc.draw(gs.binary(min_size=512, max_size=512))
        assert data


def test_lots_of_entropy():
    _run(LotsOfEntropyPerStepMachine)


def test_flatmap():
    class Machine(hegel.StateMachine):
        steps = 5

        def __init__(self, tc):
            self.buns = hegel.Pool(tc)
            self.buns.add(0)

        @hegel.rule()
        def use_flatmap(self, tc):
            bun = tc.draw(self.buns.values_reusable().flat_map(lambda x: gs.just(x + 1)))
            assert isinstance(bun, int)
            self.buns.add(bun)

        @hegel.rule()
        def use_directly(self, tc):
            bun = tc.draw(self.buns.values_reusable())
            assert isinstance(bun, int)

    _run(Machine, test_cases=10)


def test_consumes_flatmap():
    class Machine(hegel.StateMachine):
        steps = 5

        def __init__(self, tc):
            self.my_bundle = hegel.Pool(tc)

        @hegel.rule()
        def f(self, tc):
            self.my_bundle.add("abcde")

        @hegel.rule()
        def g(self, tc):
            value = tc.draw(self.my_bundle.values_consumed().flat_map(lambda v: gs.just(None)))
            assert value is None

    _run(Machine, test_cases=10)


def test_use_bundle_within_other_strategies():
    class Class:
        def __init__(self, value):
            self.value = value

    class Machine(hegel.StateMachine):
        steps = 5

        def __init__(self, tc):
            self.my_bundle = hegel.Pool(tc)
            self.my_bundle.add("sample text")

        @hegel.rule()
        def check(self, tc):
            instance = tc.draw(gs.builds(Class, self.my_bundle.values_reusable()))
            assert isinstance(instance, Class)
            assert isinstance(instance.value, str)

    _run(Machine, test_cases=10)


def _filled_pool(tc, values):
    pool = hegel.Pool(tc)
    for value in values:
        pool.add(value)
    return pool


def test_can_filter_bundle():
    class Machine(hegel.StateMachine):
        steps = 5

        def __init__(self, tc):
            self.values = _filled_pool(tc, range(10))

        @hegel.rule()
        def use(self, tc):
            value = tc.draw(self.values.values_reusable().filter(lambda x: x % 2 == 0))
            assert value % 2 == 0

    _run(Machine, test_cases=10)


def test_can_map_bundle():
    class Machine(hegel.StateMachine):
        steps = 5

        def __init__(self, tc):
            self.values = _filled_pool(tc, (1, 2, 3))

        @hegel.rule()
        def use(self, tc):
            value = tc.draw(self.values.values_reusable().map(lambda x: x * 10))
            assert value in (10, 20, 30)

    _run(Machine, test_cases=10)


def test_can_filter_mapped_bundle():
    class Machine(hegel.StateMachine):
        steps = 5

        def __init__(self, tc):
            self.values = _filled_pool(tc, range(10))

        @hegel.rule()
        def use(self, tc):
            value = tc.draw(
                self.values.values_reusable().filter(lambda x: x % 2 == 0).map(lambda x: x + 1)
            )
            assert value % 2 == 1

    _run(Machine, test_cases=10)


def test_filtering_consumed_bundle_consumes_only_the_drawn_value():
    class Machine(hegel.StateMachine):
        steps = 5

        def __init__(self, tc):
            self.values = _filled_pool(tc, range(10))

        @hegel.rule()
        def pop_even(self, tc):
            value = tc.draw(self.values.values_consumed().filter(lambda x: x % 2 == 0))
            assert value % 2 == 0
            remaining = set(self.values._values.values())
            # rejected values are not consumed from the pool
            assert {1, 3, 5, 7, 9} <= remaining

    _run(Machine, test_cases=20)


def test_can_consume_filtered_bundle():
    class Machine(hegel.StateMachine):
        steps = 1

        def __init__(self, tc):
            self.values = _filled_pool(tc, range(4))

        @hegel.rule()
        def pop(self, tc):
            value = tc.draw(self.values.values_consumed().filter(lambda x: x == 3))
            assert value == 3

    _run(Machine, test_cases=10)


def test_can_consume_mapped_bundle():
    class Machine(hegel.StateMachine):
        steps = 2

        def __init__(self, tc):
            self.values = _filled_pool(tc, (1, 2, 3))

        @hegel.rule()
        def pop(self, tc):
            value = tc.draw(self.values.values_consumed().map(lambda x: x * 10))
            assert value in (10, 20, 30)

    _run(Machine, test_cases=10)


def test_unsatisfiable_bundle_filter_fails_health_check():
    class Machine(hegel.StateMachine):
        steps = 2

        def __init__(self, tc):
            self.values = _filled_pool(tc, [0])

        @hegel.rule()
        def use(self, tc):
            tc.draw(self.values.values_reusable().filter(lambda x: False))
            raise AssertionError("should be unreachable")

    with pytest.raises(FailedHealthCheck, match="FilterTooMuch"):
        _run(Machine, test_cases=2)


def test_can_generate_unique_lists_of_bundle_values():
    class Machine(hegel.StateMachine):
        steps = 3

        def __init__(self, tc):
            self.values = _filled_pool(tc, range(5))

        @hegel.rule()
        def use(self, tc):
            vs = tc.draw(gs.lists(self.values.values_reusable(), unique=True, min_size=2))
            assert len(set(vs)) == len(vs)

    _run(Machine, test_cases=10)


def test_can_generate_unique_lists_of_filtered_bundle_values():
    class Machine(hegel.StateMachine):
        steps = 3

        def __init__(self, tc):
            self.values = _filled_pool(tc, range(10))

        @hegel.rule()
        def use(self, tc):
            vs = tc.draw(
                gs.lists(
                    self.values.values_reusable().filter(lambda x: x % 2 == 0),
                    unique=True,
                    min_size=2,
                )
            )
            assert len(set(vs)) == len(vs)
            assert all(v % 2 == 0 for v in vs)

    _run(Machine, test_cases=10)
