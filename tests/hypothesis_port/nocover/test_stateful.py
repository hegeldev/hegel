# Dropped from the Hypothesis original: test_multiple_precondition_bug (@precondition);
# test_unrelated_rule_does_not_use_var_reference_repr, test_doesnt_replace_when_different_id
# (Hypothesis's printed program with bundle variable references).
# Rewritten: TestMyStatefulMachine as test_MyStatefulMachine (no MachineClass.TestCase);
# test_replaces_when_same_id as test_shrinks_source_same_as_target_to_two_steps (Hegel's step
# format instead of variable references).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import inspect
from collections import namedtuple

import pytest

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.utils import no_shrink, notes


def run_to_notes(machine):
    try:
        hegel.state_machine_test(machine, test_cases=500)()
    except AssertionError as err:
        # Hegel attaches the whole falsifying example as one note.
        return [line for note in notes(err) for line in note.split("\n")]

    raise RuntimeError("Expected an assertion error")


def assert_runs_to_output(machine, output):
    # remove the first line, which is always "Falsfying example:"
    actual = "\n".join(run_to_notes(machine)[1:])
    assert actual == inspect.cleandoc(output.strip())


Leaf = namedtuple("Leaf", ("label",))
Split = namedtuple("Split", ("left", "right"))


class BalancedTrees(hegel.StateMachine):
    def __init__(self, tc):
        self.trees = hegel.Pool(tc)

    @hegel.rule()
    def leaf(self, tc):
        x = tc.draw(gs.booleans())
        self.trees.add(Leaf(x))

    @hegel.rule()
    def split(self, tc):
        left = tc.draw(self.trees.values_reusable())
        right = tc.draw(self.trees.values_reusable())
        self.trees.add(Split(left, right))

    @hegel.rule()
    def test_is_balanced(self, tc):
        tree = tc.draw(self.trees.values_reusable())
        self.check_is_balanced(tree)

    def check_is_balanced(self, tree):
        if isinstance(tree, Leaf):
            return

        assert abs(self.size(tree.left) - self.size(tree.right)) <= 1
        self.check_is_balanced(tree.left)
        self.check_is_balanced(tree.right)

    def size(self, tree):
        if isinstance(tree, Leaf):
            return 1
        else:
            return 1 + self.size(tree.left) + self.size(tree.right)


class DepthCharge:
    def __init__(self, value):
        if value is None:
            self.depth = 0
        else:
            self.depth = value.depth + 1


class DepthMachine(hegel.StateMachine):
    def __init__(self, tc):
        self.charges = hegel.Pool(tc)

    @hegel.rule()
    def charge(self, tc):
        child = tc.draw(self.charges.values_reusable())
        self.charges.add(DepthCharge(child))

    @hegel.rule()
    def none_charge(self, tc):
        self.charges.add(DepthCharge(None))

    @hegel.rule()
    def is_not_too_deep(self, tc):
        check = tc.draw(self.charges.values_reusable())
        assert check.depth < 3


class RoseTreeStateMachine(hegel.StateMachine):
    def __init__(self, tc):
        self.nodes = hegel.Pool(tc)

    @hegel.rule()
    def bunch(self, tc):
        source = tc.draw(gs.lists(self.nodes.values_reusable()))
        self.nodes.add(source)

    @hegel.rule()
    def shallow(self, tc):
        source = tc.draw(self.nodes.values_reusable())

        def depth(ls):
            return 0 if not ls else 1 + max(map(depth, ls))

        assert depth(source) <= 5


class NotTheLastMachine(hegel.StateMachine):
    def __init__(self, tc):
        self.stuff = hegel.Pool(tc)
        self.last = None
        self.bye_called = False

    @hegel.rule()
    def hi(self, tc):
        result = object()
        self.last = result
        self.stuff.add(result)

    @hegel.rule()
    def bye(self, tc):
        tc.assume(not self.bye_called)
        v = tc.draw(self.stuff.values_reusable())
        assert v == self.last
        self.bye_called = True


class PopulateMultipleTargets(hegel.StateMachine):
    def __init__(self, tc):
        self.b1 = hegel.Pool(tc)
        self.b2 = hegel.Pool(tc)

    @hegel.rule()
    def populate(self, tc):
        self.b1.add(1)
        self.b2.add(1)

    @hegel.rule()
    def fail(self, tc):
        tc.draw(self.b1.values_reusable())
        tc.draw(self.b2.values_reusable())
        raise AssertionError


class CanSwarm(hegel.StateMachine):
    """This test will essentially never pass if you choose rules uniformly at
    random, because every time the snake rule fires we return to the beginning,
    so we will tend to undo progress well before we make enough progress for
    the test to fail.

    This tests our swarm testing functionality in stateful testing by ensuring
    that we can sometimes generate long runs of steps which exclude a
    particular rule.
    """

    def __init__(self):
        self.seen = set()

    @hegel.rule()
    def ladder(self, tc):
        move = tc.draw(gs.integers())
        self.seen.add(move)
        assert len(self.seen) <= 15

    @hegel.rule()
    def snake(self, tc):
        self.seen.clear()


bad_machines = (
    BalancedTrees,
    DepthMachine,
    RoseTreeStateMachine,
    NotTheLastMachine,
    PopulateMultipleTargets,
    CanSwarm,
)


@pytest.mark.parametrize("machine", bad_machines, ids=[t.__name__ for t in bad_machines])
def test_bad_machines_fail(machine):
    with pytest.raises(AssertionError):
        hegel.state_machine_test(
            machine,
            test_cases=1000,
            database="",
            phases=no_shrink,
            suppress_health_check=[hegel.HealthCheck.filter_too_much],
        )()


class MyStatefulMachine(hegel.StateMachine):
    steps = 5

    def __init__(self):
        self.n_steps = 0

    @hegel.rule()
    def step(self, tc):
        self.n_steps += 1
        assert self.n_steps <= 10


test_MyStatefulMachine = hegel.state_machine_test(MyStatefulMachine, derandomize=True)


class SourceSameAsTarget(hegel.StateMachine):
    def __init__(self, tc):
        self.values = hegel.Pool(tc)

    @hegel.rule()
    def f(self, tc):
        value = tc.draw(gs.lists(self.values.values_reusable()))
        assert len(value) == 0
        self.values.add(value)


def test_shrinks_source_same_as_target_to_two_steps():
    assert_runs_to_output(
        SourceSameAsTarget,
        """
        Step 1: f {
          value = []
        }
        Step 2: f {
          value = [[]]
        }
        Rule f failed:
        """,
    )
