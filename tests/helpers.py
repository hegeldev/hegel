from __future__ import annotations

import unicodedata
from collections.abc import Callable
from typing import Any

import pytest

import hegel
from hegel.generators.combinators import as_generator
from hegel.runner import with_output_override


def run_test(fn: Callable[..., Any], **settings: Any) -> None:
    hegel.run(fn, **settings)


# libhegel's Unicode tables are 17.0, so the categories its text generators
# admit, its `\w` and its case folding follow assignments and
# reclassifications this Python's unicodedata, and so its re module, do not
# know yet (see docs/engine-issues.md). Whether a run meets one of those
# characters is down to chance, so the tests are skipped rather than xfailed
# until this Python catches up.
UNICODE_NEWER_THAN_PYTHON = pytest.mark.skipif(
    unicodedata.unidata_version < "17.0.0",
    reason="Engine: libhegel's Unicode 17.0 tables are newer than this Python's "
    f"({unicodedata.unidata_version}), so it draws characters Python's re and "
    "unicodedata classify differently",
)


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


def find_example(
    generator: Any, condition: Callable[[Any], bool] | None = None, seed: int | None = None
) -> Any:
    """The minimal value from ``generator`` satisfying ``condition`` (any
    value when no condition is given). The search runs as a test whose
    failure is finding the value, so the result is shrunk, and it raises
    ``Unsatisfiable`` when every case is rejected. A ``seed`` fixes the
    search."""
    generator = as_generator(generator)
    found: list[Any] = []

    def body(tc: hegel.TestCase) -> None:
        value = tc.draw_silent(generator)
        if condition is not None and not condition(value):
            tc.reject()
        found.append(value)
        raise _Found()

    try:
        with with_output_override(lambda line: None):
            hegel.run(
                body,
                hegel.Settings(
                    database="",
                    verbosity=hegel.Verbosity.quiet,
                    phases=[hegel.Phase.generate, hegel.Phase.shrink],
                    test_cases=1000,
                    print_blob=False,
                    suppress_health_check=hegel.HealthCheck.all(),
                    seed=seed,
                ),
            )
    except _Found:
        return found[-1]
    raise AssertionError("unreachable: the search either finds a value or rejects every case")
