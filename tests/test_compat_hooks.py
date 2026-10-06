"""The small hooks the core keeps for the hegel.hypothesis layer."""

from __future__ import annotations

import inspect

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import find_example, notes, run_test


def test_generator_compat_methods():
    g = gs.integers()
    g.validate()
    assert g.is_empty is False
    assert gs.nothing().is_empty is True
    assert gs.Generator.flatmap is gs.Generator.flat_map


def test_is_final_is_set_on_the_final_replay_and_explicit_cases():
    seen: list[bool] = []

    def body(tc: hegel.TestCase) -> None:
        seen.append(tc.is_final)
        x = tc.draw(gs.integers(0, 10))
        assert x < 5

    exception = None
    try:
        run_test(body, database="")
    except AssertionError as error:
        exception = error
    assert exception is not None
    assert seen[-1] is True
    assert seen[0] is False

    seen.clear()
    hegel.run(
        body,
        database="",
        explicit_cases=[hegel.ExplicitCase((), {"x": 1})],
        phases=[hegel.Phase.explicit],
    )
    assert seen == [True]


def test_machine_without_step_output():
    def body(tc: hegel.TestCase) -> None:
        machine = hegel.Machine(tc)
        machine._print_steps = False
        count = [0]

        def step(tc: hegel.TestCase) -> None:
            count[0] += 1
            tc.assume(tc.draw(gs.booleans()))
            tc.note(f"step {count[0]}")
            assert count[0] < 3

        machine.rule("step", step)
        machine.invariant("inv", lambda tc: None)
        machine.steps(5)
        machine.run()

    try:
        run_test(body, database="")
    except AssertionError as error:
        text = "\n".join(notes(error))
    else:
        raise AssertionError("expected a failure")
    assert "Step 1" not in text
    assert "Checking invariants" not in text
    assert "step 3" in text


def test_machine_without_step_output_reports_invariant_failures():
    def body(tc: hegel.TestCase) -> None:
        machine = hegel.Machine(tc)
        machine._print_steps = False
        machine.rule("step", lambda tc: None)
        machine.invariant("inv", lambda tc: (_ for _ in ()).throw(ValueError("bad")))
        machine.run()

    try:
        run_test(body, database="")
    except ValueError as error:
        assert "Invariant inv failed" not in "\n".join(notes(error))
    else:
        raise AssertionError("expected a failure")


class _EmptyNow(gs.Generator[int]):
    """A generator that can report having no values right now, as a
    stateful bundle does."""

    label = gs.integers().label

    def __init__(self, empty: bool) -> None:
        self.empty = empty

    def is_currently_empty(self, tc: hegel.TestCase) -> bool:
        return self.empty

    def do_draw(self, tc: hegel.TestCase) -> int:
        return tc.draw_silent(gs.integers(0, 3))


def test_lists_of_a_currently_empty_generator():
    assert find_example(gs.lists(_EmptyNow(True))) == []
    assert len(find_example(gs.lists(_EmptyNow(False), min_size=1))) >= 1
    with pytest.raises(hegel.Unsatisfiable):
        find_example(gs.lists(_EmptyNow(True), min_size=1))


def test_from_type_honours_an_explicit_signature_without_self():
    class Model:
        __signature__ = inspect.Signature(
            [
                inspect.Parameter("self", inspect.Parameter.POSITIONAL_OR_KEYWORD),
                inspect.Parameter("n", inspect.Parameter.KEYWORD_ONLY, annotation=int),
            ]
        )

        def __init__(self, **kwargs: int) -> None:
            self.n = kwargs["n"]

    value = find_example(gs.from_type(Model))
    assert isinstance(value.n, int)


def test_from_type_ignores_variadic_parameters():
    class Variadic:
        def __init__(self, x: int, *args: str, **kwargs: str) -> None:
            self.x = x

    assert isinstance(find_example(gs.from_type(Variadic)).x, int)


def test_from_type_keeps_an_explicit_signature_without_self():
    class Model:
        __signature__ = inspect.Signature(
            [inspect.Parameter("n", inspect.Parameter.KEYWORD_ONLY, annotation=int)]
        )

        def __init__(self, **kwargs: int) -> None:
            self.n = kwargs["n"]

    assert isinstance(find_example(gs.from_type(Model)).n, int)


def test_from_type_requires_annotations_for_required_arguments():
    class Needs:
        def __init__(self, x, *args, **kwargs):
            self.x = x

    with pytest.raises(hegel.InvalidArgument, match="x"):
        gs.from_type(Needs)


def test_builds_skips_non_type_annotations_on_required_arguments():
    def target(x: 1):  # type: ignore[valid-type]
        return x

    assert find_example(gs.builds(target, x=gs.just(2))) == 2
    with pytest.raises(TypeError):
        find_example(gs.builds(target))


def test_builds_skips_non_type_annotations():
    def target(x: 1 = 0):  # type: ignore[valid-type]
        return x

    assert find_example(gs.builds(target)) == 0
