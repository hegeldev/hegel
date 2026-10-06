"""Engine behaviour recorded in docs/engine-issues.md, pinned with a seed so
each reproduces every run. Each test is a strict xfail: when the engine is
fixed it passes, and the xfail (and the docs entry) should go."""

from __future__ import annotations

import pytest

import hegel
from hegel import generators as gs
from tests.test_generators_from_type import Node

ENGINE_ISSUE = "engine issue: see docs/engine-issues.md"


class _Found(Exception):
    pass


def _first_found(generator, condition, seed):
    """The value a seeded search for one satisfying ``condition`` shrinks
    to, or None if the run ends without finding one."""
    found = []

    def body(tc):
        value = tc.draw_silent(generator)
        tc.assume(condition(value))
        found.append(value)
        raise _Found

    try:
        hegel.run(
            body,
            database="",
            seed=seed,
            verbosity=hegel.Verbosity.quiet,
            phases=[hegel.Phase.generate, hegel.Phase.shrink],
            test_cases=1000,
            print_blob=False,
            suppress_health_check=hegel.HealthCheck.all(),
        )
    except _Found:
        return found[-1]
    return None


@pytest.mark.xfail(strict=True, reason=ENGINE_ISSUE)
def test_an_integer_shrinks_to_the_smallest_positive_multiple_of_six():
    value = _first_found(gs.integers(0, 100), lambda x: x > 0 and x % 6 == 0, seed=6)
    assert value == 6  # the engine stops at 78


@pytest.mark.xfail(strict=True, reason=ENGINE_ISSUE)
def test_a_run_whose_first_cases_are_rejected_does_not_pass_quietly():
    value = _first_found(gs.from_type(Node), lambda n: n.child is not None, seed=3)
    assert value is not None  # the run ends, passed, having found nothing


@pytest.mark.xfail(strict=True, reason=ENGINE_ISSUE)
def test_a_small_finite_space_is_covered_by_a_hundred_cases():
    generator = gs.one_of(
        gs.sampled_from(["a", "b", "c", "empty"]),
        gs.integers(1, 3)
        .flat_map(lambda r: gs.sets(gs.sampled_from([1, 2, 4]), min_size=r, max_size=r))
        .map(sum),
    )
    seen = set()
    hegel.run(lambda tc: seen.add(tc.draw_silent(generator)), database="", seed=27)
    assert "empty" in seen
