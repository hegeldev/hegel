from __future__ import annotations

import re
import threading
import time

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import AssumeFailed, InternalError, InvalidArgument, LoopDone, StopTest
from hegel.stateful import (
    ANONYMOUS_GROUP,
    DEFAULT_STEP_COUNT,
    ConcurrentPool,
    Machine,
    Pool,
    _Event,
    _resolve_round,
)
from tests.helpers import failing, notes, run_test


class Counter(hegel.StateMachine):
    def __init__(self):
        self.count = 0

    @hegel.rule()
    def incr(self, tc):
        step = tc.draw(gs.integers(1, 10))
        self.count += step

    @hegel.rule(weight=0.5, group="decr")
    def decr(self, tc):
        n = tc.draw(gs.integers(1, 10))
        tc.assume(n <= self.count)
        self.count -= n

    @hegel.invariant()
    def non_negative(self, tc):
        assert self.count >= 0

    @hegel.invariant(always_run=True)
    def is_int(self, tc):
        assert isinstance(self.count, int)


def test_sequential_machine_runs_and_shrinks():
    instances = []

    class Small(Counter):
        steps = 20

        def __init__(self):
            super().__init__()
            instances.append(self)

        @hegel.invariant()
        def small(self, tc):
            assert self.count < 15

    test = hegel.state_machine_test(Small, test_cases=50)
    assert test.__name__ == "test_Small"
    assert test.is_hegel_test
    with pytest.raises(AssertionError) as info:
        test()
    text = notes(info.value)[0]
    assert "Checking invariants on the initial state." in text
    assert "Step 1: incr {" in text
    assert "  step = " in text
    assert "Invariant small failed:" in text
    increments = [int(line.split("= ")[1]) for line in text.splitlines() if "step = " in line]
    assert sum(increments) == 15  # shrunk to exactly the boundary


def test_passing_machine_and_final_invariants(output):
    class Fine(Counter):
        steps = 5

    hegel.state_machine_test(Fine, test_cases=10, verbosity=hegel.Verbosity.verbose)()
    assert "Checking invariants on the final state." in output.lines
    assert any("Rule stopped early due to violated assumption." in line for line in output.lines)
    assert any(line.startswith("Step ") for line in output.lines)


def test_rule_failure_is_reported():
    class Boom(hegel.StateMachine):
        @hegel.rule()
        def boom(self, tc):
            x = tc.draw(gs.integers(0, 5))
            if x > 2:
                raise ValueError(x)

    with pytest.raises(ValueError) as info:
        hegel.state_machine_test(Boom)()
    text = notes(info.value)[0]
    assert "Rule boom failed:" in text
    assert text.endswith("Rule boom failed:")
    assert "x = 3" in text


def test_class_based_machine_discovery_and_overrides():
    assert [name for name, _ in Counter._rules()] == ["incr", "decr"]
    assert [spec["group"] for _, spec in Counter._rules()] == [None, "decr"]
    assert [name for name, _ in Counter._invariants()] == ["non_negative", "is_int"]
    assert Counter._invariants()[1][1]["always_run"] is True

    class Renamed(hegel.StateMachine):
        @hegel.rule(name="custom")
        def method(self, tc):
            pass

        @hegel.invariant(name="inv")
        def check(self, tc):
            pass

    assert Renamed._rules()[0][1]["name"] == "custom"
    assert Renamed._invariants()[0][1]["name"] == "inv"

    def body(tc):
        hegel.run_state_machine(tc, Counter, steps=3)
        hegel.run_state_machine(tc, Counter(), steps=3, min_concurrency=1, max_concurrency=1)
        machine = Counter().build_machine(tc)
        assert machine._steps == DEFAULT_STEP_COUNT
        hegel.run_state_machine(tc, machine, steps=2)
        with pytest.raises(InvalidArgument):
            hegel.run_state_machine(tc, Counter, bogus=1)
        with pytest.raises(InvalidArgument):
            hegel.run_state_machine(tc, 5)  # type: ignore[arg-type]
        with pytest.raises(InvalidArgument):
            hegel.run_state_machine(tc, Counter, steps=0)
        with pytest.raises(InvalidArgument):
            hegel.run_state_machine(tc, Counter, min_concurrency=0)
        with pytest.raises(InvalidArgument):
            hegel.run_state_machine(tc, Counter, max_concurrency="x")
        with pytest.raises(InvalidArgument):
            hegel.run_state_machine(tc, Counter, min_concurrency=3, max_concurrency=2)

    run_test(body, test_cases=3)


def test_machine_constructed_with_test_case():
    class TakesTc(hegel.StateMachine):
        def __init__(self, tc):
            self.tc = tc

        @hegel.rule()
        def check(self, tc):
            assert self.tc is not None

    hegel.state_machine_test(TakesTc, test_cases=3)()


def test_machine_builder_api():
    log = []

    def body(tc):
        machine = Machine(tc)
        assert machine.rule("push", lambda tc: log.append("push")) is machine

        @machine.rule("pop", group="g", weight=2)
        def pop(tc):
            log.append("pop")

        @machine.invariant("inv", always_run=True)
        def inv(tc):
            log.append("inv")

        assert machine.invariant("inv2", lambda tc: None) is machine
        assert machine.steps(4).min_concurrency(1).max_concurrency(1) is machine
        with pytest.raises(InvalidArgument, match="duplicate rule"):
            machine.rule("push", lambda tc: None)
        with pytest.raises(InvalidArgument, match="duplicate invariant"):
            machine.invariant("inv", lambda tc: None)
        with pytest.raises(InvalidArgument, match="weight"):
            machine.rule("w", lambda tc: None, weight=0)
        with pytest.raises(InvalidArgument, match="weight"):
            machine.rule("w", lambda tc: None, weight="heavy")
        with pytest.raises(InvalidArgument, match="weight"):
            machine.rule("w", lambda tc: None, weight=float("inf"))
        machine.run()
        assert machine._rules[1].group == "g"
        assert machine._rules[0].group == ANONYMOUS_GROUP

    run_test(body, test_cases=3)
    assert "inv" in log and ("push" in log or "pop" in log)

    def no_rules(tc):
        Machine(tc).run()

    failing(no_rules, InvalidArgument, test_cases=1)


def test_invariant_failure_is_reported():
    class Bad(hegel.StateMachine):
        @hegel.rule()
        def step(self, tc):
            pass

        @hegel.invariant()
        def fails(self, tc):
            raise KeyError("inv")

    with pytest.raises(KeyError) as info:
        hegel.state_machine_test(Bad)()
    assert notes(info.value)[0].endswith("Invariant fails failed:")


def test_invariant_may_reject_test_case():
    class Picky(hegel.StateMachine):
        def __init__(self):
            self.n = 0

        @hegel.rule()
        def step(self, tc):
            self.n += 1

        @hegel.invariant()
        def few(self, tc):
            tc.assume(self.n < 3)

    hegel.state_machine_test(Picky, test_cases=5, suppress_health_check=hegel.HealthCheck.all())()


def test_rule_raising_control_signal_propagates():
    class Loop(hegel.StateMachine):
        @hegel.rule()
        def step(self, tc):
            raise LoopDone()

    hegel.state_machine_test(Loop, test_cases=2)()

    class Stop(hegel.StateMachine):
        @hegel.rule()
        def step(self, tc):
            if tc.draw(gs.booleans()):
                raise StopTest()

    hegel.state_machine_test(Stop, test_cases=2, suppress_health_check=hegel.HealthCheck.all())()

    class AlwaysStop(hegel.StateMachine):
        @hegel.rule()
        def step(self, tc):
            raise StopTest()

    with pytest.raises(hegel.Unsatisfiable):
        hegel.state_machine_test(
            AlwaysStop, test_cases=2, suppress_health_check=hegel.HealthCheck.all()
        )()


def test_pools():
    class Stack(hegel.StateMachine):
        def __init__(self, tc):
            self.pool = Pool(tc)
            self.consumed = []

        @hegel.rule()
        def add(self, tc):
            self.pool.add(tc.draw(gs.integers()))
            assert not self.pool.is_empty
            assert len(self.pool) > 0
            assert repr(self.pool) == f"Pool({len(self.pool)} values)"

        @hegel.rule()
        def reuse(self, tc):
            value = tc.draw(self.pool.values_reusable())
            assert isinstance(value, int)

        @hegel.rule()
        def consume(self, tc):
            before = len(self.pool)
            value = tc.draw(self.pool.values_consumed())
            self.consumed.append(value)
            assert len(self.pool) == before - 1

    hegel.state_machine_test(Stack, test_cases=30)()

    def reprs(tc):
        pool = Pool(tc)
        assert repr(pool.values_reusable()) == "Pool(0 values).values_reusable()"
        assert repr(pool.values_consumed()) == "Pool(0 values).values_consumed()"
        pool.add(1, tc)
        assert len(pool) == 1

    run_test(reprs, test_cases=1)


def test_pool_draw_from_empty_rejects():
    class Empty(hegel.StateMachine):
        def __init__(self, tc):
            self.pool = Pool(tc)

        @hegel.rule()
        def take(self, tc):
            tc.draw(self.pool.values_reusable())

    hegel.state_machine_test(Empty, test_cases=3, suppress_health_check=hegel.HealthCheck.all())()


def test_pool_filter_consumes_only_the_accepted_value():
    """A filtered consuming draw takes exactly the value it yields; the
    values its attempts rejected stay in the pool and can be drawn later."""

    class Evens(hegel.StateMachine):
        def __init__(self, tc):
            self.pool = Pool(tc)
            self.held = []

        @hegel.rule()
        def add(self, tc):
            value = tc.draw(gs.integers(0, 20))
            self.pool.add(value)
            self.held.append(value)

        @hegel.rule()
        def take_even(self, tc):
            value = tc.draw(self.pool.values_consumed().filter(lambda x: x % 2 == 0))
            assert value % 2 == 0
            self.held.remove(value)

        @hegel.rule()
        def take_big_even(self, tc):
            even = self.pool.values_consumed().filter(lambda x: x % 2 == 0)
            value = tc.draw(even.filter(lambda x: x > 4))
            assert value % 2 == 0 and value > 4
            self.held.remove(value)

        @hegel.rule()
        def take_any(self, tc):
            self.held.remove(tc.draw(self.pool.values_consumed()))

        @hegel.invariant()
        def pool_holds_what_was_added_and_not_taken(self, tc):
            assert sorted(self.pool._values.values()) == sorted(self.held)

    hegel.state_machine_test(Evens, test_cases=50, suppress_health_check=hegel.HealthCheck.all())()


def test_pool_filter_rejects_the_case_and_restores_when_no_value_is_accepted():
    """With every value rejected (here fewer values than attempts), the
    draw rejects the test case and every value is back in the pool, in the
    engine as well as in the binding's map."""

    class Odds(hegel.StateMachine):
        def __init__(self, tc):
            self.pool = Pool(tc)
            self.pool.add(1)
            self.pool.add(3)

        @hegel.rule()
        def take_even(self, tc):
            tc.draw(self.pool.values_consumed().filter(lambda x: x % 2 == 0))
            raise AssertionError("an odd value passed the filter")

        @hegel.rule()
        def predicate_rejects(self, tc):
            calls = []

            def first_false_then_reject(value):
                calls.append(value)
                if len(calls) == 1:
                    return False
                tc.reject()

            tc.draw(self.pool.values_consumed().filter(first_false_then_reject))
            raise AssertionError("unreachable")

        @hegel.rule()
        def take_both(self, tc):
            both = {tc.draw(self.pool.values_consumed()) for _ in range(2)}
            assert both == {1, 3}
            self.pool.add(1)
            self.pool.add(3)

        @hegel.invariant()
        def both_values_present(self, tc):
            assert len(self.pool) == 2

    hegel.state_machine_test(Odds, test_cases=20, suppress_health_check=hegel.HealthCheck.all())()


def test_pool_filter_reprs():
    def even(x):
        return x % 2 == 0

    def reprs(tc):
        pool = Pool(tc)
        consumed = pool.values_consumed().filter(even)
        assert repr(consumed) == "Pool(0 values).values_consumed().filter(even)"
        assert repr(consumed.filter(bool)) == (
            "Pool(0 values).values_consumed().filter(even).filter(bool)"
        )
        assert repr(pool.values_reusable().filter(even)) == (
            "Pool(0 values).values_reusable().filter(even)"
        )
        pool.add(1, tc)
        pool.add(2, tc)
        assert tc.draw(pool.values_reusable().filter(even)) == 2
        assert len(pool) == 2
        assert tc.draw(consumed) == 2
        assert len(pool) == 1

    run_test(reprs, test_cases=1)


def test_pool_values_shrink_toward_earliest():
    class Shrinks(hegel.StateMachine):
        def __init__(self, tc):
            self.pool = Pool(tc)

        @hegel.rule()
        def add(self, tc):
            self.pool.add(tc.draw(gs.integers(0, 100)))

        @hegel.rule()
        def check(self, tc):
            v = tc.draw(self.pool.values_reusable())
            assert v < 5

    with pytest.raises(AssertionError) as info:
        hegel.state_machine_test(Shrinks)()
    text = notes(info.value)[0]
    assert "Step 1: add {" in text and "v = 5" in text


# -- concurrent -------------------------------------------------------------


class Bank(hegel.StateMachine):
    max_concurrency = 4
    steps = 30

    def __init__(self):
        self.balance = 0
        self.lock = threading.Lock()

    @hegel.rule(group="deposit")
    def deposit(self, tc):
        amount = tc.draw(gs.integers(1, 100))
        with self.lock:
            self.balance += amount

    @hegel.rule(group="withdraw")
    def withdraw(self, tc):
        amount = tc.draw(gs.integers(1, 100))
        if self.balance >= amount:  # racy check-then-act
            time.sleep(0.0001)
            self.balance -= amount

    @hegel.invariant()
    def non_negative(self, tc):
        assert self.balance >= 0, self.balance


def test_concurrent_machine_finds_race(output):
    with pytest.raises(AssertionError) as info:
        hegel.state_machine_test(Bank, test_cases=300)()
    text = notes(info.value)[0]
    assert text.startswith("Falsifying example:\nConcurrency level: ")
    assert re.search(
        r'^---------------- Round 1: group "(deposit|withdraw)" ----------------$', text, re.M
    )
    assert "[worker " in text
    assert "Invariant non_negative failed:" in text


def test_concurrent_machine_passes_when_safe():
    class Safe(Bank):
        steps = 10
        min_concurrency = 2
        max_concurrency = 3

        @hegel.rule(group="withdraw")
        def withdraw(self, tc):
            amount = tc.draw(gs.integers(1, 100))
            with self.lock:
                if self.balance >= amount:
                    self.balance -= amount

    hegel.state_machine_test(Safe, test_cases=20)()


def test_concurrent_rules_may_reject_and_use_pools():
    class Pooled(hegel.StateMachine):
        max_concurrency = 3
        steps = 15

        def __init__(self, tc):
            self.pool = ConcurrentPool(tc)
            self.pool.add(0, tc)

        @hegel.rule()
        def add(self, tc):
            x = tc.draw(gs.integers())
            tc.assume(x % 3 != 0)
            self.pool.add(x, tc)
            assert not self.pool.is_empty
            assert len(self.pool) >= 1
            assert repr(self.pool).startswith("ConcurrentPool(")

        @hegel.rule()
        def take(self, tc):
            tc.draw(self.pool.values_consumed())

        @hegel.rule()
        def look(self, tc):
            tc.draw(self.pool.values_reusable())

    hegel.state_machine_test(Pooled, test_cases=20, suppress_health_check=hegel.HealthCheck.all())()

    def needs_tc(tc):
        pool = ConcurrentPool(tc)
        with pytest.raises(InvalidArgument):
            pool.add(1)

    run_test(needs_tc, test_cases=1)


def test_concurrent_pool_filter_consumes_only_the_accepted_value():
    class Evens(hegel.StateMachine):
        max_concurrency = 3
        steps = 15

        def __init__(self, tc):
            self.pool = ConcurrentPool(tc)
            self.lock = threading.Lock()
            self.balance = 0

        @hegel.rule()
        def add(self, tc):
            value = tc.draw(gs.integers(0, 20))
            with self.lock:
                self.pool.add(value, tc)
                self.balance += 1

        @hegel.rule()
        def take_even(self, tc):
            value = tc.draw(self.pool.values_consumed().filter(lambda x: x % 2 == 0))
            assert value % 2 == 0
            with self.lock:
                self.balance -= 1

        @hegel.invariant()
        def pool_holds_what_was_added_and_not_taken(self, tc):
            assert len(self.pool) == self.balance

    hegel.state_machine_test(Evens, test_cases=20, suppress_health_check=hegel.HealthCheck.all())()


def test_concurrent_pool_empty_rejects():
    class Empty(hegel.StateMachine):
        max_concurrency = 2
        steps = 4

        def __init__(self, tc):
            self.pool = ConcurrentPool(tc)

        @hegel.rule()
        def take(self, tc):
            tc.draw(self.pool.values_reusable())

    hegel.state_machine_test(Empty, test_cases=3, suppress_health_check=hegel.HealthCheck.all())()


def test_concurrent_rule_failure_is_reported():
    class Boom(hegel.StateMachine):
        max_concurrency = 2
        steps = 10

        @hegel.rule()
        def boom(self, tc):
            x = tc.draw(gs.integers(0, 3))
            if x == 3:
                raise ValueError(x)

    with pytest.raises(ValueError) as info:
        hegel.state_machine_test(Boom, test_cases=100)()
    text = notes(info.value)[0]
    assert "Rule boom failed:" in text


def test_concurrent_control_errors_propagate():
    class Bad(hegel.StateMachine):
        max_concurrency = 2
        steps = 4

        @hegel.rule()
        def bad(self, tc):
            raise InvalidArgument("nope")

    with pytest.raises(InvalidArgument, match="nope"):
        hegel.state_machine_test(Bad, test_cases=5)()

    class Internal(hegel.StateMachine):
        max_concurrency = 2
        steps = 4

        @hegel.rule()
        def bad(self, tc):
            raise InternalError("bug")

    with pytest.raises(InternalError):
        hegel.state_machine_test(Internal, test_cases=5)()


def test_concurrent_invariant_failure():
    class Bad(hegel.StateMachine):
        max_concurrency = 2
        steps = 4

        @hegel.rule()
        def step(self, tc):
            pass

        @hegel.invariant(always_run=True)
        def fails(self, tc):
            raise KeyError

    with pytest.raises(KeyError):
        hegel.state_machine_test(Bad, test_cases=5)()


def test_concurrent_rule_overrun_and_stop():
    class Stops(hegel.StateMachine):
        max_concurrency = 2
        steps = 4

        @hegel.rule()
        def stop(self, tc):
            if tc.draw(gs.booleans()):
                raise StopTest()

    hegel.state_machine_test(Stops, test_cases=3, suppress_health_check=hegel.HealthCheck.all())()


def test_resolve_round_precedence():
    noted = []

    class FakeTc:
        def note(self, text):
            noted.append(text)

    tc = FakeTc()
    _resolve_round([_Event("done"), _Event("done")], tc)
    with pytest.raises(InvalidArgument):
        _resolve_round([_Event("panicked", ValueError()), _Event("control", InvalidArgument())], tc)
    with pytest.raises(StopTest):
        _resolve_round([_Event("panicked", ValueError("p")), _Event("overrun")], tc)
    assert noted[-1].startswith("Dropped concurrent exception from worker 0: ValueError: p")
    with pytest.raises(AssumeFailed):
        _resolve_round([_Event("invalid"), _Event("done")], tc)
    first, second = ValueError("first"), ValueError("second")
    with pytest.raises(ValueError, match="first"):
        _resolve_round([_Event("done"), _Event("panicked", first), _Event("panicked", second)], tc)
    assert "worker 2" in noted[-1]
    with pytest.raises(InternalError, match="exited without reporting"):
        _resolve_round([_Event("died")], tc)
    with pytest.raises(InternalError, match="exited without reporting"):
        _resolve_round([_Event("died"), _Event("control", InvalidArgument())], tc)
    with pytest.raises(InvalidArgument):
        _resolve_round([_Event("control", InvalidArgument()), _Event("died")], tc)


def test_concurrent_failure_in_verbose_mode(output):
    class Boom(hegel.StateMachine):
        max_concurrency = 2
        steps = 5

        @hegel.rule()
        def boom(self, tc):
            raise ValueError("concurrent boom")

    with pytest.raises(ValueError):
        hegel.state_machine_test(Boom, test_cases=5, verbosity=hegel.Verbosity.verbose)()
    assert output.text.count("ValueError: concurrent boom") >= 1
    assert any(line.startswith("Test case failed: ValueError") for line in output.lines)


def test_concurrent_failure_in_quiet_mode(output):
    with pytest.raises(AssertionError) as info:
        hegel.state_machine_test(Bank, test_cases=300, verbosity=hegel.Verbosity.quiet)()
    assert notes(info.value) == []
    assert output.lines == []


def test_rules_and_invariants_must_take_the_test_case():
    with pytest.raises(InvalidArgument, match=r"rule r must take .*def r\(self, tc\)"):

        @hegel.rule()
        def r(self):
            pass

    with pytest.raises(InvalidArgument, match="invariant i must take"):

        @hegel.invariant()
        def i(self, tc, extra):
            pass

    with pytest.raises(InvalidArgument, match="rule a is async"):

        @hegel.rule()
        async def a(self, tc):
            pass

    with pytest.raises(InvalidArgument, match="must decorate a method"):
        hegel.rule()(5)


def test_machine_constructor_takes_nothing_or_the_test_case():
    class TakesTwo(hegel.StateMachine):
        def __init__(self, a, b):
            pass

    with pytest.raises(InvalidArgument, match="must take no arguments, or only the test case"):
        hegel.run(lambda tc: hegel.run_state_machine(tc, TakesTwo), database="")
