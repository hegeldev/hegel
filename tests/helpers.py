from __future__ import annotations

from collections.abc import Callable
from typing import Any

import pytest

import hegel
from hegel.generators.combinators import as_generator


def run_test(fn: Callable[..., Any], **settings: Any) -> None:
    hegel.run(fn, **settings)


def failing(
    fn: Callable[..., Any], exception: type[BaseException] = AssertionError, **settings: Any
) -> BaseException:
    """Run ``fn`` expecting it to fail with ``exception``; returns it."""
    with pytest.raises(exception) as info:
        hegel.run(fn, **settings)
    return info.value


def notes(exception: BaseException) -> list[str]:
    return list(getattr(exception, "__notes__", []))


class _Found(Exception):
    pass


def find_example(generator: Any, condition: Callable[[Any], bool] | None = None) -> Any:
    """The minimal value from ``generator`` satisfying ``condition`` (any
    value when no condition is given). The search runs as a test whose
    failure is finding the value, so the result is shrunk."""
    generator = as_generator(generator)
    found: list[Any] = []

    def body(tc: hegel.TestCase) -> None:
        value = tc.draw_silent(generator)
        if condition is not None and not condition(value):
            tc.reject()
        found.append(value)
        raise _Found()

    try:
        with hegel.with_output_override(lambda line: None):
            hegel.run(
                body,
                hegel.Settings(
                    database="",
                    verbosity=hegel.Verbosity.quiet,
                    phases=[hegel.Phase.generate, hegel.Phase.shrink],
                    test_cases=1000,
                    print_blob=False,
                    suppress_health_check=hegel.HealthCheck.all(),
                ),
            )
    except _Found:
        return found[-1]
    raise hegel.InvalidArgument("Could not find an example satisfying the condition")
