"""Stateful testing: state machines driven by the engine, sequentially or
concurrently, and pools of values shared between their rules."""

from __future__ import annotations

import queue
import threading
from collections.abc import Callable
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, TypeVar

from hegel._labels import label_from_name
from hegel.errors import (
    AssumeFailed,
    InternalError,
    InvalidArgument,
    LoopDone,
    StopTest,
)
from hegel.generators.base import Generator

if TYPE_CHECKING:
    from hegel._native import StateMachine as NativeMachine
    from hegel.test_case import TestCase

__all__ = [
    "ANONYMOUS_GROUP",
    "DEFAULT_STEP_COUNT",
    "ConcurrentPool",
    "Invariant",
    "Machine",
    "Pool",
    "Rule",
    "StateMachine",
    "invariant",
    "rule",
    "run_state_machine",
    "state_machine_test",
]

T = TypeVar("T")

RULE_LABEL = label_from_name("hegel.stateful.rule")
VALUES_REUSABLE_LABEL = label_from_name("hegel.pool.values_reusable")
VALUES_CONSUMED_LABEL = label_from_name("hegel.pool.values_consumed")
DEFAULT_STEP_COUNT = 50
ANONYMOUS_GROUP = "<anonymous>"


@dataclass(frozen=True)
class Rule:
    """A rule of a state machine: ``apply(tc)`` performs one step."""

    name: str
    apply: Callable[[TestCase], Any]
    group: str = ANONYMOUS_GROUP
    weight: float = 1.0


@dataclass(frozen=True)
class Invariant:
    """An invariant of a state machine: ``apply(tc)`` checks it. An
    always-run invariant is checked after every step; otherwise the engine
    samples when to check it (it always runs on the initial and final
    state)."""

    name: str
    apply: Callable[[TestCase], Any]
    always_run: bool = False


def _check_weight(name: str, weight: Any) -> float:
    try:
        value = float(weight)
    except (TypeError, ValueError):
        raise InvalidArgument(
            f"rule {name!r}: weight={weight!r} must be a positive number"
        ) from None
    if not (value > 0) or value != value or value in (float("inf"),):
        raise InvalidArgument(f"rule {name!r}: weight={weight!r} must be finite and positive")
    return value


class Machine:
    """A state machine assembled from rule and invariant callables.

    ::

        machine = hegel.Machine(tc)
        machine.rule("push", lambda tc: stack.append(tc.draw(gs.integers())))
        machine.invariant("sorted", lambda tc: check(stack))
        machine.run()

    With ``max_concurrency`` above 1 the machine runs concurrently: the
    engine picks a concurrency level, and in each round every worker
    thread applies rules from the round's group to the shared model.
    """

    def __init__(self, tc: TestCase) -> None:
        self._tc = tc
        self._rules: list[Rule] = []
        self._invariants: list[Invariant] = []
        self._steps = DEFAULT_STEP_COUNT
        self._min_concurrency = 1
        self._max_concurrency = 1

    def rule(
        self,
        name: str,
        apply: Callable[[TestCase], Any] | None = None,
        *,
        group: str | None = None,
        weight: float = 1.0,
    ) -> Any:
        """Add a rule; usable as a decorator when ``apply`` is omitted."""

        def add(fn: Callable[[TestCase], Any]) -> Callable[[TestCase], Any]:
            if any(r.name == name for r in self._rules):
                raise InvalidArgument(f"duplicate rule name {name!r}")
            self._rules.append(
                Rule(
                    name,
                    fn,
                    ANONYMOUS_GROUP if group is None else str(group),
                    _check_weight(name, weight),
                )
            )
            return fn

        if apply is None:
            return add
        add(apply)
        return self

    def invariant(
        self,
        name: str,
        apply: Callable[[TestCase], Any] | None = None,
        *,
        always_run: bool = False,
    ) -> Any:
        """Add an invariant; usable as a decorator when ``apply`` is omitted."""

        def add(fn: Callable[[TestCase], Any]) -> Callable[[TestCase], Any]:
            if any(i.name == name for i in self._invariants):
                raise InvalidArgument(f"duplicate invariant name {name!r}")
            self._invariants.append(Invariant(name, fn, bool(always_run)))
            return fn

        if apply is None:
            return add
        add(apply)
        return self

    def steps(self, step_count: int) -> Machine:
        """The target number of steps per test case (default 50)."""
        if not isinstance(step_count, int) or step_count < 1:
            raise InvalidArgument(f"steps={step_count!r} must be a positive integer")
        self._steps = step_count
        return self

    def min_concurrency(self, n: int) -> Machine:
        if not isinstance(n, int) or n < 1:
            raise InvalidArgument(f"min_concurrency={n!r} must be a positive integer")
        self._min_concurrency = n
        return self

    def max_concurrency(self, n: int) -> Machine:
        if not isinstance(n, int) or n < 1:
            raise InvalidArgument(f"max_concurrency={n!r} must be a positive integer")
        self._max_concurrency = n
        return self

    def run(self) -> None:
        """Run the machine on this test case."""
        if not self._rules:
            raise InvalidArgument("a state machine needs at least one rule")
        if self._min_concurrency > self._max_concurrency:
            raise InvalidArgument("min_concurrency must not exceed max_concurrency")
        if self._max_concurrency > 1:
            _run_concurrent(
                self._tc,
                self._rules,
                self._invariants,
                self._min_concurrency,
                self._max_concurrency,
                self._steps,
            )
        else:
            _run_sequential(self._tc, self._rules, self._invariants, self._steps)


# -- the engine protocol ----------------------------------------------------


def _note_trailer(tc: TestCase, exception: BaseException, trailer: str) -> None:
    if not isinstance(exception, (AssumeFailed, StopTest)):
        tc.note(trailer)


def _check_invariants(
    tc: TestCase, invariants: list[Invariant], machine: NativeMachine | None
) -> None:
    for index, inv in enumerate(invariants):
        if machine is not None and not machine.should_check_invariant(tc._handle, index):
            continue
        inv_tc = tc.child(2)
        try:
            inv.apply(inv_tc)
        except BaseException as exception:
            _note_trailer(tc, exception, f"Invariant {inv.name} failed:")
            raise


def _run_sequential(
    tc: TestCase, rules: list[Rule], invariants: list[Invariant], step_count: int
) -> None:
    machine, _ = tc._handle.new_state_machine(
        [r.name for r in rules],
        [0] * len(rules),
        [r.weight for r in rules],
        [i.name for i in invariants],
        [i.always_run for i in invariants],
        1,
        1,
        step_count,
    )
    if invariants:
        tc.note("Checking invariants on the initial state.")
    _check_invariants(tc, invariants, None)
    steps_attempted = 0
    while True:
        tc.start_span(RULE_LABEL)
        if machine.next_group(tc._handle) is None:
            tc.stop_span(False)
            break
        round_rejected = False
        while (rule_index := machine.next_rule(tc._handle, 0)) is not None:
            if not 0 <= rule_index < len(rules):  # nocov
                raise InternalError(
                    f"state_machine_next_rule returned out-of-range rule index {rule_index}"
                )
            current = rules[rule_index]
            tc.note(f"Step {steps_attempted + 1}: {current.name} {{")
            rule_tc = tc.child(2)
            steps_attempted += 1
            try:
                current.apply(rule_tc)
            except AssumeFailed:
                machine.rule_rejected(tc._handle, 0)
                round_rejected = True
                tc.child(2).note("Rule stopped early due to violated assumption.")
                tc.note("}")
            except BaseException as exception:
                tc.note("}")
                _note_trailer(tc, exception, f"Rule {current.name} failed:")
                tc.stop_span(False)
                raise
            else:
                tc.note("}")
        tc.stop_span(round_rejected)
        _check_invariants(tc, invariants, machine)
    if invariants:
        tc.note("Checking invariants on the final state.")
    _check_invariants(tc, invariants, None)


class _Event:
    __slots__ = ("exception", "kind")

    def __init__(self, kind: str, exception: BaseException | None = None) -> None:
        self.kind = kind
        self.exception = exception


def _classify(exception: BaseException) -> _Event:
    if isinstance(exception, AssumeFailed):
        return _Event("invalid")
    if isinstance(exception, StopTest):
        return _Event("overrun")
    if isinstance(exception, (InvalidArgument, InternalError, LoopDone)):
        return _Event("control", exception)
    return _Event("panicked", exception)


def _worker_round(worker: int, tc: TestCase, rules: list[Rule], machine: NativeMachine) -> _Event:
    while True:
        try:
            rule_index = machine.next_rule(tc._handle, worker)
            if rule_index is not None and not 0 <= rule_index < len(rules):  # nocov
                raise InternalError(
                    f"state_machine_next_rule returned out-of-range rule index {rule_index}"
                )
        except BaseException as exception:  # nocov - the engine abandoned the case
            return _classify(exception)
        if rule_index is None:
            return _Event("done")
        current = rules[rule_index]
        tc.note(f"Rule: {current.name} {{")
        rule_tc = tc.child(2)
        try:
            current.apply(rule_tc)
        except BaseException as exception:
            event = _classify(exception)
            if event.kind == "invalid":
                try:
                    machine.rule_rejected(tc._handle, worker)
                except BaseException as rejected:  # nocov - the engine abandoned the case
                    tc.note("}")
                    return _classify(rejected)
                tc.child(2).note("Rule stopped early due to violated assumption.")
                tc.note("}")
                continue
            tc.note("}")
            if event.kind == "panicked":
                tc.note(f"Rule {current.name} failed:")
            return event
        tc.note("}")


def _worker_loop(
    worker: int,
    rules: list[Rule],
    machine: NativeMachine,
    rounds: queue.Queue[TestCase | None],
    events: queue.Queue[_Event],
) -> None:
    while True:
        tc = rounds.get()
        if tc is None:
            return
        try:
            event = _worker_round(worker, tc, rules, machine)
        except BaseException as exception:  # nocov
            event = _Event(
                "control", InternalError(f"worker {worker} failed unexpectedly: {exception!r}")
            )
        events.put(event)


def _resolve_round(events: list[_Event], tc: TestCase) -> None:
    control: BaseException | None = None
    saw_overrun = False
    saw_invalid = False
    panics: list[tuple[int, BaseException]] = []
    for worker, event in enumerate(events):
        if event.kind == "invalid":
            saw_invalid = True
        elif event.kind == "overrun":
            saw_overrun = True
        elif event.kind == "control":
            if control is None:
                control = event.exception
        elif event.kind == "panicked":
            panics.append((worker, event.exception))  # type: ignore[arg-type]
        elif event.kind == "died":
            if control is None:
                control = InternalError(
                    f"Internal error in hegel: concurrent stateful worker {worker} exited "
                    "without reporting an outcome. This is a bug in hegel itself; please "
                    "report it at https://github.com/hegeldev/hegel-python/issues"
                )

    def note_dropped(dropped: list[tuple[int, BaseException]]) -> None:
        for worker, exception in dropped:
            tc.note(
                f"Dropped concurrent exception from worker {worker}: "
                f"{type(exception).__name__}: {exception}"
            )

    if control is not None:
        raise control
    if saw_overrun or saw_invalid:
        note_dropped(panics)
        if saw_overrun:
            raise StopTest()
        raise AssumeFailed()
    if panics:
        note_dropped(panics[1:])
        raise panics[0][1]


def _run_concurrent(
    tc: TestCase,
    rules: list[Rule],
    invariants: list[Invariant],
    min_concurrency: int,
    max_concurrency: int,
    step_count: int,
) -> None:
    group_names: list[str] = []
    rule_groups: list[int] = []
    for r in rules:
        if r.group not in group_names:
            group_names.append(r.group)
        rule_groups.append(group_names.index(r.group))
    machine, concurrency = tc._handle.new_state_machine(
        [r.name for r in rules],
        rule_groups,
        [r.weight for r in rules],
        [i.name for i in invariants],
        [i.always_run for i in invariants],
        min_concurrency,
        max_concurrency,
        step_count,
    )
    tc.note(f"Concurrency level: {concurrency}")
    if invariants:
        tc.note("Checking invariants on the initial state.")
    _check_invariants(tc, invariants, None)

    round_queues: list[queue.Queue[TestCase | None]] = []
    event_queues: list[queue.Queue[_Event]] = []
    threads: list[threading.Thread] = []
    for worker in range(concurrency):
        rounds: queue.Queue[TestCase | None] = queue.Queue()
        outcomes: queue.Queue[_Event] = queue.Queue()
        round_queues.append(rounds)
        event_queues.append(outcomes)
        thread = threading.Thread(
            target=_worker_loop,
            args=(worker, rules, machine, rounds, outcomes),
            name=f"hegel-worker-{worker}",
            daemon=True,
        )
        threads.append(thread)
        thread.start()
    try:
        round_number = 0
        while (group := machine.next_group(tc._handle)) is not None:
            if not 0 <= group < len(group_names):  # nocov
                raise InternalError(f"state_machine_next_group returned unknown group id {group}")
            round_number += 1
            name = group_names[group]
            tc.note(f"---------------- Round {round_number}: group {name!r} ----------------")
            for worker, rounds in enumerate(round_queues):
                worker_tc = tc.clone()
                worker_tc._set_worker(worker)
                rounds.put(worker_tc)
            events: list[_Event] = []
            for worker, event_queue in enumerate(event_queues):
                if not threads[worker].is_alive() and event_queue.empty():  # nocov
                    events.append(_Event("died"))
                else:
                    events.append(event_queue.get())
            _resolve_round(events, tc)
            _check_invariants(tc, invariants, machine)
        if invariants:
            tc.note("Checking invariants on the final state.")
        _check_invariants(tc, invariants, None)
    finally:
        for rounds in round_queues:
            rounds.put(None)
        for thread in threads:
            thread.join()


# -- class-based machines ---------------------------------------------------


def rule(
    fn: Callable[..., Any] | None = None,
    *,
    group: str | None = None,
    weight: float = 1.0,
    name: str | None = None,
) -> Any:
    """Mark a :class:`StateMachine` method as a rule. The method takes the
    test case: ``def push(self, tc)``."""

    def decorate(method: Callable[..., Any]) -> Callable[..., Any]:
        method._hegel_rule = {  # type: ignore[attr-defined]
            "group": group,
            "weight": _check_weight(name or method.__name__, weight),
            "name": name or method.__name__,
        }
        return method

    return decorate if fn is None else decorate(fn)


def invariant(
    fn: Callable[..., Any] | None = None, *, always_run: bool = False, name: str | None = None
) -> Any:
    """Mark a :class:`StateMachine` method as an invariant."""

    def decorate(method: Callable[..., Any]) -> Callable[..., Any]:
        method._hegel_invariant = {  # type: ignore[attr-defined]
            "always_run": bool(always_run),
            "name": name or method.__name__,
        }
        return method

    return decorate if fn is None else decorate(fn)


class StateMachine:
    """Base class for class-based state machines.

    Rules and invariants are methods decorated with :func:`rule` and
    :func:`invariant`; each takes the test case for its step. The class
    attributes ``steps``, ``min_concurrency`` and ``max_concurrency``
    configure the run; with ``max_concurrency > 1`` the rules run on
    worker threads against the shared instance.

    ::

        class Counter(hegel.StateMachine):
            def __init__(self):
                self.count = 0

            @hegel.rule()
            def incr(self, tc):
                self.count += tc.draw(gs.integers(min_value=1, max_value=10))

            @hegel.invariant()
            def non_negative(self, tc):
                assert self.count >= 0

        test_counter = hegel.state_machine_test(Counter)
    """

    steps: int = DEFAULT_STEP_COUNT
    min_concurrency: int = 1
    max_concurrency: int = 1

    @classmethod
    def _rules(cls) -> list[tuple[str, dict[str, Any]]]:
        found: dict[str, dict[str, Any]] = {}
        for klass in reversed(cls.__mro__):
            for attr, value in vars(klass).items():
                spec = getattr(value, "_hegel_rule", None)
                if spec is not None:
                    found[attr] = spec
        return list(found.items())

    @classmethod
    def _invariants(cls) -> list[tuple[str, dict[str, Any]]]:
        found: dict[str, dict[str, Any]] = {}
        for klass in reversed(cls.__mro__):
            for attr, value in vars(klass).items():
                spec = getattr(value, "_hegel_invariant", None)
                if spec is not None:
                    found[attr] = spec
        return list(found.items())

    def build_machine(self, tc: TestCase) -> Machine:
        machine = Machine(tc)
        for attr, spec in self._rules():
            machine.rule(
                spec["name"], getattr(self, attr), group=spec["group"], weight=spec["weight"]
            )
        for attr, spec in self._invariants():
            machine.invariant(spec["name"], getattr(self, attr), always_run=spec["always_run"])
        machine.steps(self.steps)
        machine.min_concurrency(self.min_concurrency)
        machine.max_concurrency(self.max_concurrency)
        return machine

    def teardown(self) -> None:
        """Called after each test case, whether or not it failed."""


def run_state_machine(
    tc: TestCase, machine: type[StateMachine] | StateMachine | Machine, **overrides: Any
) -> None:
    """Run a state machine on ``tc``: a :class:`Machine`, a
    :class:`StateMachine` instance, or a class to instantiate (with ``tc``
    if its constructor accepts one). ``steps``, ``min_concurrency`` and
    ``max_concurrency`` may be overridden."""
    if isinstance(machine, Machine):
        built = machine
    else:
        instance = _instantiate(machine, tc) if isinstance(machine, type) else machine
        if not isinstance(instance, StateMachine):
            raise InvalidArgument(f"{machine!r} is not a StateMachine")
        try:
            built = instance.build_machine(tc)
            _apply_overrides(built, overrides)
            built.run()
        finally:
            instance.teardown()
        return
    _apply_overrides(built, overrides)
    built.run()


def _apply_overrides(machine: Machine, overrides: dict[str, Any]) -> None:
    for key, value in overrides.items():
        if key == "steps":
            machine.steps(value)
        elif key == "min_concurrency":
            machine.min_concurrency(value)
        elif key == "max_concurrency":
            machine.max_concurrency(value)
        else:
            raise InvalidArgument(f"unknown state machine option {key!r}")


def _instantiate(cls: type[StateMachine], tc: TestCase) -> StateMachine:
    import inspect

    try:
        takes_tc = bool(inspect.signature(cls).parameters)
    except (TypeError, ValueError):  # nocov
        takes_tc = False
    if takes_tc:
        return cls(tc)  # type: ignore[call-arg]
    return cls()


def state_machine_test(machine: type[StateMachine], **settings: Any) -> Callable[..., None]:
    """A test function (for pytest to collect) that runs ``machine``."""
    from hegel.runner import test

    def run_machine(tc: TestCase) -> None:
        run_state_machine(tc, machine)

    run_machine.__name__ = f"test_{machine.__name__}"
    run_machine.__qualname__ = run_machine.__name__
    run_machine.__module__ = machine.__module__
    return test(**settings)(run_machine) if settings else test(run_machine)


# -- pools ------------------------------------------------------------------


class _PoolValues(Generator[Any]):
    def __init__(self, pool: Pool, consume: bool) -> None:
        self.pool = pool
        self.consume = consume
        self.label = VALUES_CONSUMED_LABEL if consume else VALUES_REUSABLE_LABEL

    def do_draw(self, tc: TestCase) -> Any:
        return self.pool._draw(tc, self.consume)

    def __repr__(self) -> str:
        return f"{self.pool!r}.{'values_consumed' if self.consume else 'values_reusable'}()"


class Pool:
    """A pool of values a stateful test accumulates, with generators that
    draw from it: :meth:`values_reusable` leaves the value in the pool,
    :meth:`values_consumed` removes it. Drawing from an empty pool rejects
    the test case."""

    def __init__(self, tc: TestCase) -> None:
        self._tc = tc
        self._handle = tc._handle.new_pool()
        self._values: dict[int, Any] = {}

    def __len__(self) -> int:
        return len(self._values)

    @property
    def is_empty(self) -> bool:
        return not self._values

    def add(self, value: Any, tc: TestCase | None = None) -> None:
        """Add ``value`` to the pool."""
        handle = (tc or self._tc)._handle
        variable_id = self._handle.add(handle)
        if variable_id in self._values:  # nocov
            raise InternalError("unexpected variable id in pool")
        self._values[variable_id] = value

    def values_reusable(self) -> Generator[Any]:
        return _PoolValues(self, False)

    def values_consumed(self) -> Generator[Any]:
        return _PoolValues(self, True)

    def _draw(self, tc: TestCase, consume: bool) -> Any:
        tc.assume(self._values)
        variable_id = self._handle.generate(tc._handle, consume)
        if consume:
            return self._values.pop(variable_id)
        return self._values[variable_id]

    def __repr__(self) -> str:
        return f"Pool({len(self._values)} values)"


class ConcurrentPool(Pool):
    """A :class:`Pool` safe to share between the worker threads of a
    concurrent state machine. ``add`` must be given the worker's test
    case."""

    def __init__(self, tc: TestCase) -> None:
        super().__init__(tc)
        self._lock = threading.Lock()

    def __len__(self) -> int:
        with self._lock:
            return len(self._values)

    @property
    def is_empty(self) -> bool:
        with self._lock:
            return not self._values

    def add(self, value: Any, tc: TestCase | None = None) -> None:
        if tc is None:
            raise InvalidArgument("ConcurrentPool.add needs the worker's test case: add(value, tc)")
        with self._lock:
            variable_id = self._handle.add(tc._handle)
            if variable_id in self._values:  # nocov
                raise InternalError("unexpected variable id in pool")
            self._values[variable_id] = value

    def _draw(self, tc: TestCase, consume: bool) -> Any:
        with self._lock:
            if not self._values:
                tc.reject()
            variable_id = self._handle.generate(tc._handle, consume)
            if consume:
                return self._values.pop(variable_id)
            return self._values[variable_id]

    def __repr__(self) -> str:
        return f"ConcurrentPool({len(self)} values)"
