from __future__ import annotations

from collections.abc import Callable
from typing import Any

import pytest

import hegel


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
