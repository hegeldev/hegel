"""Rule-based state machines in Hypothesis's style, run by Hegel's
state-machine engine."""

from __future__ import annotations

import functools
import inspect
import unittest
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field
from typing import Any, ClassVar, Generic, TypeVar

from hegel import generators as gs
from hegel.generators.combinators import OneOf
from hegel.hypothesis._settings import HealthCheck
from hegel.hypothesis._settings import settings as Settings
from hegel.hypothesis.control import BuildContext, current_build_context, currently_in_test_context
from hegel.hypothesis.core import given
from hegel.hypothesis.errors import (
    FailedHealthCheck,
    InvalidArgument,
    InvalidDefinition,
    UnsatisfiedAssumption,
)
from hegel.hypothesis.strategies import SearchStrategy, check_strategy
from hegel.hypothesis.strategies import data as data_strategy
from hegel.pretty import pretty_repr
from hegel.stateful import Machine, Pool
from hegel.test_case import TestCase
from hegel.test_case import TestCase as HegelTestCase

__all__ = [
    "Bundle",
    "MultipleResults",
    "RuleBasedStateMachine",
    "consumes",
    "initialize",
    "invariant",
    "multiple",
    "precondition",
    "rule",
    "run_state_machine_as_test",
]

Ex = TypeVar("Ex")
T = TypeVar("T")

RULE_MARKER = "hypothesis_stateful_rule"
INITIALIZE_RULE_MARKER = "hypothesis_stateful_initialize_rule"
PRECONDITIONS_MARKER = "hypothesis_stateful_preconditions"
INVARIANT_MARKER = "hypothesis_stateful_invariant"


class TestCaseProperty:
    def __get__(self, obj: Any, typ: Any = None) -> Any:
        if obj is not None:
            typ = type(obj)
        return typ._to_test_case()

    def __set__(self, obj: Any, value: Any) -> None:
        raise AttributeError("Cannot set TestCase")

    def __delete__(self, obj: Any) -> None:
        raise AttributeError("Cannot delete TestCase")


def _is_singleton(obj: object) -> bool:
    return isinstance(obj, (bool, type(None))) or (isinstance(obj, int) and -5 <= obj <= 256)


@dataclass(slots=True, frozen=True)
class VarReference:
    name: str


@dataclass(slots=True, frozen=True)
class MultipleResults(Generic[Ex]):
    values: tuple[Ex, ...]

    def __iter__(self) -> Any:
        return iter(self.values)


def multiple(*args: T) -> MultipleResults[T]:
    """Return several values from a rule, each added to its targets."""
    return MultipleResults(args)


@dataclass(slots=True, frozen=False)
class Rule:
    targets: Any
    function: Any
    arguments: Any
    preconditions: Any
    bundles: tuple[Bundle, ...] = field(init=False)

    def __post_init__(self) -> None:
        bundles = []
        arguments = {}
        for k, v in self.arguments.items():
            if isinstance(v, Bundle):
                bundles.append(v)
                # A bundle whose only transformations are filters hands the
                # rule the stored value, so the step can print its name.
                draw_references = all(name == "filter" for name, _ in v._transformations)
                v = Bundle(
                    v.name,
                    consume=v.consume,
                    draw_references=draw_references,
                    transformations=v._transformations,
                )
            arguments[k] = v
        self.arguments = arguments
        self.bundles = tuple(bundles)

    def __repr__(self) -> str:
        return f"Rule(targets={self.targets!r}, function={self.function!r})"


@dataclass(slots=True, frozen=True)
class Invariant:
    function: Any
    preconditions: Any
    check_during_init: bool


class Bundle(SearchStrategy[Ex]):
    """A strategy whose values are the results rules have returned into
    it; drawing picks one (and removes it if ``consume`` is set).

    Each bundle is one of Hegel's engine-managed pools, so which value a
    rule gets is chosen and shrunk by the engine."""

    def __init__(
        self,
        name: str,
        *,
        consume: bool = False,
        draw_references: bool = False,
        transformations: tuple[tuple[str, Callable[..., Any]], ...] = (),
    ) -> None:
        if not isinstance(name, str):
            raise InvalidArgument(f"Bundle name must be a string, not {name!r}")
        self.name = name
        self.consume = consume
        self.draw_references = draw_references
        self._transformations = transformations
        self.label = gs.label_from_name(f"hegel.hypothesis.bundle.{name}")

    def _machine(self) -> RuleBasedStateMachine:
        if currently_in_test_context():
            runner = current_build_context().runner
            if isinstance(runner, RuleBasedStateMachine):
                return runner
        raise InvalidArgument(f"Cannot draw from Bundle({self.name!r}) outside a state machine")

    def do_draw(self, tc: TestCase) -> Any:
        machine = self._machine()
        pool = machine._pool(self.name, tc)
        if pool.is_empty:
            raise UnsatisfiedAssumption()
        values = pool.values_consumed() if self.consume else pool.values_reusable()
        attempts = _FILTER_ATTEMPTS if self._transformations else 1
        for _ in range(attempts):
            reference = tc.draw_silent(values)
            result = self._transform(machine.names_to_values[reference.name])
            if result is not _REJECTED:
                if self.consume:
                    machine._consumed_this_step.append((self.name, reference))
                return reference if self.draw_references else result
            if self.consume:
                # The filter rejected it, so it stays available.
                pool.add(reference, tc)
        raise UnsatisfiedAssumption()

    def _transform(self, value: Any) -> Any:
        for kind, f in self._transformations:
            if kind == "map":
                value = f(value)
            elif not f(value):
                return _REJECTED
        return value

    def __with_transform(self, name: str, f: Callable[..., Any]) -> Bundle[Any]:
        return Bundle(
            self.name,
            consume=self.consume,
            draw_references=self.draw_references,
            transformations=(*self._transformations, (name, f)),
        )

    def map(self, pack: Callable[[Ex], Any]) -> Bundle[Any]:  # type: ignore[override]
        return self.__with_transform("map", pack)

    def filter(self, condition: Callable[[Ex], Any]) -> Bundle[Ex]:  # type: ignore[override]
        return self.__with_transform("filter", condition)

    def _base_repr(self) -> str:
        consume = ", consume=True" if self.consume else ""
        return f"Bundle(name={self.name!r}{consume})"

    def __repr__(self) -> str:
        transforms = "".join(
            f".{name}({getattr(f, '__name__', repr(f))})" for name, f in self._transformations
        )
        return self._base_repr() + transforms

    def is_currently_empty(self, tc: TestCase | None = None) -> bool:
        return not self._machine().bundle(self.name)

    def __hash__(self) -> int:
        return hash(("Bundle", self.name))


_REJECTED = object()


_FILTER_ATTEMPTS = 3


class _AbortCase(BaseException):
    """The current test case is invalid (an assumption failed inside a
    step); raised through Hegel's machine runner, which only treats
    AssumeFailed as "skip this rule"."""


def consumes(bundle: Bundle[Ex]) -> SearchStrategy[Ex]:
    """Draw from ``bundle`` and remove the drawn value."""
    if not isinstance(bundle, Bundle):
        raise TypeError("Argument to be consumed must be a bundle.")
    return Bundle(bundle.name, consume=True, transformations=bundle._transformations)


def _convert_targets(targets: Any, target: Any) -> tuple[str, ...]:
    if target is not None:
        if targets:
            raise InvalidArgument(
                f"Passing both targets={targets!r} and target={target!r} is "
                f"redundant - pass targets={(*targets, target)!r} instead."
            )
        targets = (target,)
    converted = []
    for t in targets:
        if not isinstance(t, Bundle):
            msg = "Got invalid target %r of type %r, but all targets must be Bundles."
            if isinstance(t, OneOf):
                msg += (
                    "\nIt looks like you passed `one_of(a, b)` or `a | b` as "
                    "a target.  You should instead pass `targets=(a, b)` to "
                    "add the return value of this rule to both the `a` and "
                    "`b` bundles, or define a rule for each target if it "
                    "should be added to exactly one."
                )
            raise InvalidArgument(msg % (t, type(t)))
        converted.append(t.name)
    return tuple(converted)


def _rule_qualname(f: Any) -> str:
    return getattr(f, "__qualname__", repr(f))


def _check_kwargs(kwargs: dict[str, Any]) -> None:
    for k, v in kwargs.items():
        check_strategy(v, name=k)


def rule(
    *,
    targets: Sequence[Bundle[Any]] = (),
    target: Bundle[Any] | None = None,
    **kwargs: SearchStrategy[Any],
) -> Callable[[Any], Any]:
    """Mark a method as a rule: its keyword arguments are drawn from the
    given strategies, and its return value goes into ``target(s)``."""
    converted_targets = _convert_targets(targets, target)
    _check_kwargs(kwargs)

    def accept(f: Any) -> Any:
        if getattr(f, INVARIANT_MARKER, None):
            raise InvalidDefinition(
                f"{_rule_qualname(f)} is used with both @rule and @invariant, which is not allowed."
            )
        existing_rule = getattr(f, RULE_MARKER, None)
        existing_initialize_rule = getattr(f, INITIALIZE_RULE_MARKER, None)
        if existing_rule is not None:
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with @rule twice, which is not allowed."
            )
        if existing_initialize_rule is not None:
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with both @rule and "
                "@initialize, which is not allowed."
            )
        preconditions = getattr(f, PRECONDITIONS_MARKER, ())
        new_rule = Rule(
            targets=converted_targets, arguments=kwargs, function=f, preconditions=preconditions
        )

        @functools.wraps(f)
        def rule_wrapper(*args: Any, **kwargs: Any) -> Any:
            return f(*args, **kwargs)

        setattr(rule_wrapper, RULE_MARKER, new_rule)
        return rule_wrapper

    return accept


def initialize(
    *,
    targets: Sequence[Bundle[Any]] = (),
    target: Bundle[Any] | None = None,
    **kwargs: SearchStrategy[Any],
) -> Callable[[Any], Any]:
    """Mark a method as an initialisation rule, run once (in a random
    order with the others) before any ordinary rule."""
    converted_targets = _convert_targets(targets, target)
    _check_kwargs(kwargs)

    def accept(f: Any) -> Any:
        if getattr(f, INVARIANT_MARKER, None):
            raise InvalidDefinition(
                f"{_rule_qualname(f)} is used with both @initialize and "
                "@invariant, which is not allowed."
            )
        existing_rule = getattr(f, RULE_MARKER, None)
        existing_initialize_rule = getattr(f, INITIALIZE_RULE_MARKER, None)
        if existing_rule is not None:
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with both @rule and "
                "@initialize, which is not allowed."
            )
        if existing_initialize_rule is not None:
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with @initialize "
                "twice, which is not allowed."
            )
        preconditions = getattr(f, PRECONDITIONS_MARKER, ())
        if preconditions:
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with both @initialize and @precondition, "
                "which is not allowed. Initialize rules are always run, so a precondition would "
                "have no effect."
            )
        new_rule = Rule(
            targets=converted_targets, arguments=kwargs, function=f, preconditions=preconditions
        )

        @functools.wraps(f)
        def rule_wrapper(*args: Any, **kwargs: Any) -> Any:
            return f(*args, **kwargs)

        setattr(rule_wrapper, INITIALIZE_RULE_MARKER, new_rule)
        return rule_wrapper

    return accept


def precondition(precond: Callable[[Any], bool]) -> Callable[[Any], Any]:
    """Only run the decorated rule or invariant while ``precond(self)``."""
    if not callable(precond):
        raise InvalidArgument(f"precond={precond!r} must be a callable")

    def decorator(f: Any) -> Any:
        @functools.wraps(f)
        def precondition_wrapper(*args: Any, **kwargs: Any) -> Any:
            return f(*args, **kwargs)

        if getattr(f, INITIALIZE_RULE_MARKER, None):
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with both @initialize and @precondition, "
                "which is not allowed. Initialize rules are always run, so a precondition would "
                "have no effect."
            )
        existing_rule = getattr(f, RULE_MARKER, None)
        existing_invariant = getattr(f, INVARIANT_MARKER, None)
        if existing_rule is not None:
            new_rule = Rule(
                targets=existing_rule.targets,
                arguments=existing_rule.arguments,
                function=precondition_wrapper,
                preconditions=(*existing_rule.preconditions, precond),
            )
            setattr(precondition_wrapper, RULE_MARKER, new_rule)
        elif existing_invariant is not None:
            new_invariant = Invariant(
                function=precondition_wrapper,
                preconditions=(*existing_invariant.preconditions, precond),
                check_during_init=existing_invariant.check_during_init,
            )
            setattr(precondition_wrapper, INVARIANT_MARKER, new_invariant)
        else:
            setattr(
                precondition_wrapper,
                PRECONDITIONS_MARKER,
                (*getattr(f, PRECONDITIONS_MARKER, ()), precond),
            )
        return precondition_wrapper

    return decorator


def invariant(*, check_during_init: bool = False) -> Callable[[Any], Any]:
    """Mark a method as an invariant, checked after every rule."""
    if not isinstance(check_during_init, bool):
        raise InvalidArgument(f"check_during_init={check_during_init!r} must be a bool")

    def accept(f: Any) -> Any:
        if getattr(f, RULE_MARKER, None) or getattr(f, INITIALIZE_RULE_MARKER, None):
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with both @invariant and "
                "@rule, which is not allowed."
            )
        if getattr(f, INVARIANT_MARKER, None):
            raise InvalidDefinition(
                f"{_rule_qualname(f)} has been decorated with @invariant "
                "twice, which is not allowed."
            )
        preconditions = getattr(f, PRECONDITIONS_MARKER, ())
        invar = Invariant(
            function=f, preconditions=preconditions, check_during_init=check_during_init
        )

        @functools.wraps(f)
        def invariant_wrapper(*args: Any, **kwargs: Any) -> Any:
            return f(*args, **kwargs)

        setattr(invariant_wrapper, INVARIANT_MARKER, invar)
        return invariant_wrapper

    return accept


class StateMachineMeta(type):
    def __setattr__(cls, name: str, value: Any) -> None:
        if name == "settings" and isinstance(value, Settings):
            descr = f"settings({value.show_changed()})"
            raise AttributeError(
                f"Assigning {cls.__name__}.settings = {descr} does nothing. Assign "
                f"to {cls.__name__}.TestCase.settings, or use @{descr} as a decorator "
                f"on the {cls.__name__} class."
            )
        return super().__setattr__(name, value)


@dataclass(slots=True, frozen=True)
class _SetupState:
    rules: list[Rule]
    invariants: list[Invariant]
    initializers: list[Rule]


class RuleBasedStateMachine(metaclass=StateMachineMeta):
    """A state machine whose rules (methods decorated with ``@rule``) are
    run in an engine-chosen order, checking invariants between them."""

    _setup_state_per_class: ClassVar[dict[type, _SetupState]] = {}

    def __init__(self) -> None:
        setup_state = self.setup_state()
        if not setup_state.rules:
            raise InvalidDefinition(f"State machine {type(self).__name__} defines no rules")
        if isinstance(s := vars(type(self)).get("settings"), Settings):
            tname = type(self).__name__
            descr = f"settings({s.show_changed()})"
            raise InvalidDefinition(
                f"Assigning settings = {descr} as a class attribute does nothing. "
                f"Assign to {tname}.TestCase.settings, or use @{descr} as a decorator "
                f"on the {tname} class."
            )
        self.rules = setup_state.rules
        self.invariants = setup_state.invariants
        self._initialize_rules_to_run = setup_state.initializers.copy()
        self._pools: dict[str, Pool] = {}
        self._tc: HegelTestCase | None = None
        self._consumed_this_step: list[tuple[str, VarReference]] = []
        self.names_counters: dict[str, int] = {}
        self.names_list: list[str] = []
        self.names_to_values: dict[str, Any] = {}
        self._value_names: dict[int, str] = {}

    def _pretty_print(self, value: Any) -> str:
        if isinstance(value, VarReference):
            return value.name
        if (
            isinstance(value, list)
            and value
            and all(isinstance(item, VarReference) for item in value)
        ):
            return "[" + ", ".join(item.name for item in value) + "]"
        if not _is_singleton(value) and id(value) in self._value_names:
            return self._value_names[id(value)]
        if type(value) in (list, tuple, set, frozenset) and any(
            not _is_singleton(item) and id(item) in self._value_names for item in value
        ):
            parts = [self._pretty_print(item) for item in value]
            if type(value) is list:
                return "[" + ", ".join(parts) + "]"
            if type(value) is tuple:
                return "(" + ", ".join(parts) + ("," if len(parts) == 1 else "") + ")"
            if type(value) is set:
                return "{" + ", ".join(parts) + "}"
            return "frozenset({" + ", ".join(parts) + "})"
        if type(value) is dict and any(
            not _is_singleton(item) and id(item) in self._value_names for item in value.values()
        ):
            items = ", ".join(
                f"{self._pretty_print(k)}: {self._pretty_print(v)}" for k, v in value.items()
            )
            return "{" + items + "}"
        return pretty_repr(value)

    @property
    def bundles(self) -> dict[str, list[VarReference]]:
        """The references each bundle currently holds."""
        return {name: self.bundle(name) for name in self._pools}

    def __repr__(self) -> str:
        return f"{type(self).__name__}({self.bundles!r})"

    def _new_name(self, target: str) -> str:
        count = self.names_counters.get(target, 0)
        result = f"{target}_{count}"
        self.names_counters[target] = count + 1
        self.names_list.append(result)
        return result

    def _last_names(self, n: int) -> list[str]:
        return self.names_list[len(self.names_list) - n :]

    def bundle(self, name: str) -> list[VarReference]:
        pool = self._pools.get(name)
        return list(pool._values.values()) if pool is not None else []

    def _pool(self, name: str, tc: HegelTestCase) -> Pool:
        try:
            return self._pools[name]
        except KeyError:
            if self._tc is None:
                raise InvalidArgument(
                    f"Cannot use Bundle({name!r}) before the state machine runs"
                ) from None
            pool = self._pools[name] = Pool(self._tc)
            return pool

    @classmethod
    def setup_state(cls) -> _SetupState:
        try:
            return cls._setup_state_per_class[cls]
        except KeyError:
            pass
        rules: list[Rule] = []
        initializers: list[Rule] = []
        invariants: list[Invariant] = []
        for _name, f in inspect.getmembers(cls):
            r = getattr(f, RULE_MARKER, None)
            initializer = getattr(f, INITIALIZE_RULE_MARKER, None)
            inv = getattr(f, INVARIANT_MARKER, None)
            if r is not None:
                rules.append(r)
            if initializer is not None:
                initializers.append(initializer)
            if inv is not None:
                invariants.append(inv)
            if getattr(f, PRECONDITIONS_MARKER, None) is not None and r is None and inv is None:
                raise InvalidDefinition(
                    f"{_rule_qualname(f)} has been decorated with @precondition, "
                    "but not @rule (or @invariant), which is not allowed. A "
                    "precondition must be combined with a rule or an invariant, "
                    "since it has no effect alone."
                )
        state = _SetupState(rules=rules, initializers=initializers, invariants=invariants)
        cls._setup_state_per_class[cls] = state
        return state

    def _repr_step(self, r: Rule, data: dict[str, str], result: Any) -> str:
        output_assignment = ""
        extra_assignment_lines = []
        if r.targets:
            number_of_results = len(result.values) if isinstance(result, MultipleResults) else 1
            number_of_last_names = len(r.targets) * number_of_results
            last_names = self._last_names(number_of_last_names)
            if isinstance(result, MultipleResults):
                if len(result.values) == 1:
                    output_assignment = " = ".join(f"({name},)" for name in last_names) + " = "
                elif result.values:
                    names_per_target = [
                        last_names[i : i + number_of_results]
                        for i in range(0, len(last_names), number_of_results)
                    ]
                    first_target_output = ", ".join(names_per_target[0])
                    output_assignment = first_target_output + " = "
                    for other in names_per_target[1:]:
                        extra_assignment_lines.append(
                            ", ".join(other) + " = " + first_target_output
                        )
            else:
                output_assignment = " = ".join(last_names) + " = "
        args = ", ".join(f"{k}={v}" for k, v in data.items())
        output_line = f"{output_assignment}state.{r.function.__name__}({args})"
        return "\n".join([output_line, *extra_assignment_lines])

    def _add_results_to_targets(
        self, tc: HegelTestCase, targets: Iterable[str], results: Iterable[Any]
    ) -> None:
        for target in targets:
            for result in results:
                name = self._new_name(target)
                if not _is_singleton(result):
                    self._value_names.setdefault(id(result), name)
                self.names_to_values[name] = result
                self._pool(target, tc).add(VarReference(name), tc)

    def check_invariants(self, settings: Settings, output: Callable[[str], None]) -> None:
        for invar in self.invariants:
            if self._initialize_rules_to_run and not invar.check_during_init:
                continue
            if not all(precond(self) for precond in invar.preconditions):
                continue
            name = invar.function.__name__
            output(f"state.{name}()")
            result = invar.function(self)
            if (
                result is not None
                and HealthCheck.return_value not in settings.suppress_health_check
            ):
                raise FailedHealthCheck(
                    "The return value of an @invariant is always ignored, but "
                    f"{invar.function.__qualname__} returned {result!r} instead of None"
                )

    def teardown(self) -> None:
        """Called after a run has finished executing to clean up any
        necessary state. Does nothing by default."""

    TestCase = TestCaseProperty()

    @classmethod
    @functools.lru_cache
    def _to_test_case(cls) -> type[unittest.TestCase]:
        class StateMachineTestCase(unittest.TestCase):
            settings = Settings(deadline=None, suppress_health_check=list(HealthCheck))

            def runTest(self) -> None:
                run_state_machine_as_test(cls, settings=self.settings)

            runTest.is_hypothesis_test = True  # type: ignore[attr-defined]
            runTest._hypothesis_state_machine_class = cls  # type: ignore[attr-defined]

        StateMachineTestCase.__name__ = cls.__name__ + ".TestCase"
        StateMachineTestCase.__qualname__ = cls.__qualname__ + ".TestCase"
        return StateMachineTestCase

    # -- execution -------------------------------------------------------------

    def _rule_is_valid(self, r: Rule) -> bool:
        return all(precond(self) for precond in r.preconditions) and all(
            self.bundle(b.name) for b in r.bundles
        )

    def _draw_arguments(self, tc: HegelTestCase, r: Rule) -> dict[str, Any]:
        self._consumed_this_step = []
        try:
            return {k: tc.draw_silent(v) for k, v in r.arguments.items()}
        except UnsatisfiedAssumption:
            # The rule is skipped (Hegel's machine picks another), so values
            # its arguments consumed go back into their bundles.
            for name, reference in self._consumed_this_step:
                self._pool(name, tc).add(reference, tc)
            raise

    def _execute_rule(
        self,
        tc: HegelTestCase,
        r: Rule,
        data: dict[str, Any],
        settings: Settings,
        output: Callable[[str], None],
    ) -> None:
        data_to_print = {k: self._pretty_print(v) for k, v in data.items()}
        result: Any = multiple()
        try:
            resolved = {}
            for k, v in data.items():
                if isinstance(v, VarReference):
                    resolved[k] = self.names_to_values[v.name]
                elif (
                    isinstance(v, list) and v and all(isinstance(item, VarReference) for item in v)
                ):
                    resolved[k] = [self.names_to_values[item.name] for item in v]
                else:
                    resolved[k] = v
            try:
                result = r.function(self, **resolved)
            except UnsatisfiedAssumption:
                # assume() inside a rule abandons the whole test case, as in
                # Hypothesis, rather than skipping just this rule.
                raise _AbortCase() from None
            if r.targets:
                if isinstance(result, MultipleResults):
                    self._add_results_to_targets(tc, r.targets, result.values)
                else:
                    self._add_results_to_targets(tc, r.targets, [result])
            elif (
                result is not None
                and HealthCheck.return_value not in settings.suppress_health_check
            ):
                raise FailedHealthCheck(
                    "Rules should return None if they have no target bundle, "
                    f"but {r.function.__qualname__} returned {result!r}"
                )
        finally:
            output(self._repr_step(r, data_to_print, result))

    def _run(self, tc: HegelTestCase, settings: Settings) -> None:
        try:
            self._run_steps(tc, settings)
        except _AbortCase:
            raise UnsatisfiedAssumption() from None

    def _run_steps(self, tc: HegelTestCase, settings: Settings) -> None:
        output = tc.note
        self._tc = tc
        with BuildContext(tc, is_final=tc.is_final, runner=self):
            try:
                output(f"state = {type(self).__name__}()")
                self.check_invariants(settings, output)
                while self._initialize_rules_to_run:
                    r = tc.draw_silent(gs.sampled_from(self._initialize_rules_to_run))
                    data = self._draw_arguments(tc, r)
                    self._initialize_rules_to_run.remove(r)
                    self._execute_rule(tc, r, data, settings, output)
                    self.check_invariants(settings, output)
                machine = Machine(tc)
                machine._print_steps = False
                names: dict[str, int] = {}
                for r in self.rules:
                    # Hegel needs distinct rule names; one function may back
                    # several Hypothesis rules.
                    base = r.function.__name__
                    names[base] = names.get(base, 0) + 1
                    name = base if names[base] == 1 else f"{base}#{names[base]}"
                    machine.rule(name, self._rule_runner(r, settings, output))
                # Hegel checks invariants on the initial state itself; the
                # ones after initialize rules were already checked above.
                initial = [True]

                def check(inv_tc: HegelTestCase) -> None:
                    if initial:
                        initial.pop()
                        return
                    self.check_invariants(settings, output)

                machine.invariant("invariants", check, always_run=True)
                machine.steps(settings.stateful_step_count)
                machine.run()
            finally:
                output("state.teardown()")
                self.teardown()

    def _rule_runner(
        self, r: Rule, settings: Settings, output: Callable[[str], None]
    ) -> Callable[[HegelTestCase], None]:
        def apply(tc: HegelTestCase) -> None:
            if not self._rule_is_valid(r):
                if not any(self._rule_is_valid(other) for other in self.rules):
                    rules = ", ".join(other.function.__name__ for other in self.rules)
                    raise InvalidDefinition(
                        f"No progress can be made from state {self!r}, because no "
                        f"available rule had a True precondition. rules: {rules}"
                    )
                raise UnsatisfiedAssumption()
            data = self._draw_arguments(tc, r)
            self._execute_rule(tc, r, data, settings, output)

        return apply


def get_state_machine_test(
    state_machine_factory: Any, *, settings: Settings | None = None, _min_steps: int = 0
) -> Callable[[], None]:
    if settings is None:
        try:
            settings = state_machine_factory.TestCase.settings
            if not isinstance(settings, Settings):
                raise InvalidArgument(
                    "Expected settings but got "
                    f"state_machine_factory.TestCase.settings={settings!r}"
                )
        except AttributeError:
            settings = Settings(deadline=None, suppress_health_check=list(HealthCheck))
    if not isinstance(settings, Settings):
        raise InvalidArgument(f"Expected settings but got settings={settings!r}")
    if not isinstance(_min_steps, int) or _min_steps < 0:
        raise InvalidArgument(f"_min_steps={_min_steps} must be non-negative.")
    resolved = settings

    @resolved
    @given(data_strategy())
    def run_state_machine(data: Any) -> None:
        machine = state_machine_factory()
        if not isinstance(machine, RuleBasedStateMachine):
            raise InvalidArgument(
                f"Expected RuleBasedStateMachine but got state_machine_factory()={machine!r}"
            )
        machine._run(data.conjecture_data, resolved)

    run_state_machine.hypothesis.inner_test.__name__ = getattr(
        state_machine_factory, "__name__", "run_state_machine"
    )  # type: ignore[attr-defined]
    run_state_machine.hypothesis.inner_test.__qualname__ = getattr(  # type: ignore[attr-defined]
        state_machine_factory, "__qualname__", "run_state_machine"
    )
    run_state_machine.hypothesis.inner_test.__module__ = getattr(  # type: ignore[attr-defined]
        state_machine_factory, "__module__", __name__
    )
    run_state_machine._hypothesis_internal_print_given_args = False  # type: ignore[attr-defined]
    run_state_machine._hypothesis_internal_use_seed = getattr(  # type: ignore[attr-defined]
        state_machine_factory, "_hypothesis_internal_use_seed", None
    )
    run_state_machine._hypothesis_internal_use_reproduce_failure = getattr(  # type: ignore[attr-defined]
        state_machine_factory, "_hypothesis_internal_use_reproduce_failure", None
    )
    return run_state_machine


def run_state_machine_as_test(
    state_machine_factory: Any, *, settings: Settings | None = None, _min_steps: int = 0
) -> None:
    """Run a state machine definition as a test, raising on the minimal
    failing sequence of rules it finds."""
    get_state_machine_test(state_machine_factory, settings=settings, _min_steps=_min_steps)()
