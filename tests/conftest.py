from __future__ import annotations

from collections.abc import Iterator

import pytest

import hegel
from hegel.runner import with_output_override

pytest_plugins = ["pytester"]


class Output:
    """Collects every line Hegel prints during a block."""

    def __init__(self) -> None:
        self.lines: list[str] = []

    def __call__(self, line: str) -> None:
        self.lines.append(line)

    @property
    def text(self) -> str:
        return "\n".join(self.lines)


@pytest.fixture
def output() -> Iterator[Output]:
    out = Output()
    with with_output_override(out):
        yield out


@pytest.fixture(autouse=True)
def _quiet_default_profile() -> Iterator[None]:
    """Keep the test-suite's own runs away from the example database and
    the engine's environment-dependent default profile."""
    hegel.Settings.register_profile("hegel-tests", database="", test_cases=30, print_blob=False)
    hegel.Settings.set_default_profile("hegel-tests")
    yield
