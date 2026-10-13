"""Searching a generator for a value: the helpers Hypothesis's tests use
to find a minimal example, any example, or to check every example."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any, TypeVar

import hegel
from hegel.generators.combinators import as_generator
from hegel.runner import with_output_override

T = TypeVar("T")

DEFAULT_SEARCH_CASES = 500


class Found(Exception):
    """Raised by a search's test body once the value is found: the run
    treats it as a failure, shrinks it, and re-raises it last."""


def _discard(line: str) -> None:
    pass


def _search_settings(settings: hegel.Settings | None, overrides: dict[str, Any]) -> hegel.Settings:
    """Settings for a search: quiet, seeded by the search itself, every
    health check off, and no database."""
    base = hegel.Settings(settings, **overrides) if (settings or overrides) else hegel.Settings()
    return hegel.Settings(
        base,
        database="",
        derandomize=True,
        suppress_health_check=hegel.HealthCheck.all(),
        report_multiple_failures=False,
        print_blob=False,
        verbosity=hegel.Verbosity.quiet,
    )


def minimal(
    generator: Any,
    condition: Callable[[Any], bool] = lambda x: True,
    settings: hegel.Settings | None = None,
    **overrides: Any,
) -> Any:
    """The minimal value from ``generator`` satisfying ``condition``: the
    search runs as a test whose failure is finding the value, so the result
    is shrunk. Raises :class:`hegel.Unsatisfiable` when nothing is found.
    ``settings`` and ``overrides`` are Hegel settings for the search
    (``test_cases``, ``phases``, ...)."""
    generator = as_generator(generator)
    found: list[Any] = []

    def body(tc: hegel.TestCase) -> None:
        value = tc.draw_silent(generator)
        if condition(value):
            found.append(value)
            raise Found

    if settings is None and "test_cases" not in overrides:
        overrides["test_cases"] = DEFAULT_SEARCH_CASES
    if settings is None and "phases" not in overrides:
        overrides["phases"] = (hegel.Phase.generate, hegel.Phase.shrink)
    try:
        with with_output_override(_discard):
            hegel.run(body, _search_settings(settings, overrides))
    except Found:
        return found[-1]
    raise hegel.Unsatisfiable(
        f"Could not find any examples from {generator!r} that satisfied "
        f"{getattr(condition, '__name__', repr(condition))}"
    )


def find_any(
    generator: Any,
    condition: Callable[[Any], bool] = lambda _: True,
    settings: hegel.Settings | None = None,
    *,
    tc: hegel.TestCase | None = None,
    **overrides: Any,
) -> Any:
    """Any value from ``generator`` satisfying ``condition``, unshrunk.
    Inside a test, pass its test case as ``tc`` to draw from it instead of
    starting a search (the test case is rejected if nothing turns up)."""
    if tc is not None:
        generator = as_generator(generator)
        for _ in range(1000):
            if condition(value := tc.draw_silent(generator)):
                return value
        tc.reject()
    base = hegel.Settings(settings, **overrides) if (settings or overrides) else hegel.Settings()
    return minimal(
        generator,
        condition,
        settings=hegel.Settings(
            base,
            phases=frozenset(base.phases) - {hegel.Phase.shrink},
            test_cases=max(1000, base.test_cases),
        ),
    )


def assert_no_examples(generator: Any, condition: Callable[[Any], bool] = lambda _: True) -> None:
    try:
        assert_all_examples(generator, lambda val: not condition(val))
    except hegel.Unsatisfiable:
        pass


def assert_all_examples(
    generator: Any,
    predicate: Callable[[Any], bool],
    settings: hegel.Settings | None = None,
    *,
    tc: hegel.TestCase | None = None,
    **overrides: Any,
) -> None:
    """Asserts that all examples of the given generator match the predicate.
    Inside a test, pass its test case as ``tc`` to check a few draws from it
    instead of starting a run."""
    generator = as_generator(generator)
    if tc is not None:
        for _ in range(20):
            value = tc.draw_silent(generator)
            assert predicate(value), (
                f"Found {value!r} using generator {generator} which does not match"
            )
        return

    def body(tc: hegel.TestCase) -> None:
        value = tc.draw_silent(generator)
        assert predicate(value), f"Found {value!r} using generator {generator} which does not match"

    base = hegel.Settings(settings, **overrides) if (settings or overrides) else hegel.Settings()
    with with_output_override(_discard):
        hegel.run(body, hegel.Settings(base, database="", print_blob=False))


def assert_simple_property(
    generator: Any,
    predicate: Callable[[Any], bool],
    settings: hegel.Settings | None = None,
    **overrides: Any,
) -> None:
    """Like assert_all_examples, for simple constant properties that a few
    examples verify adequately."""
    base = hegel.Settings(settings, **overrides) if (settings or overrides) else hegel.Settings()
    assert_all_examples(
        generator,
        predicate,
        hegel.Settings(base, test_cases=15, suppress_health_check=hegel.HealthCheck.all()),
    )


def check_can_generate_examples(
    generator: Any, settings: hegel.Settings | None = None, **overrides: Any
) -> None:
    """Generates a few examples from the generator, checking only that no
    error is raised."""
    base = hegel.Settings(settings, **overrides) if (settings or overrides) else hegel.Settings()
    assert_simple_property(
        generator,
        lambda _: True,
        settings=hegel.Settings(base, phases=(hegel.Phase.generate,)),
    )
