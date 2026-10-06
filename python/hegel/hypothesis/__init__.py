"""Hypothesis's API, reimplemented on top of Hegel.

``from hegel.hypothesis import given, strategies as st`` works as the
Hypothesis import would, for the parts of Hypothesis this package covers.
Tests run on Hegel's engine: generation, shrinking, the example database
and output all come from Hegel.
"""

from __future__ import annotations

from hegel.hypothesis._settings import HealthCheck, Phase, Verbosity, settings
from hegel.hypothesis.control import (
    assume,
    currently_in_test_context,
    event,
    note,
    reject,
    target,
)
from hegel.hypothesis.core import (
    example,
    find,
    given,
    infer,
    is_hypothesis_test,
    reproduce_failure,
    seed,
)

__version__ = "6.168.5+hegel"
__version_info__ = (6, 168, 5)

__all__ = [
    "HealthCheck",
    "Phase",
    "Verbosity",
    "__version__",
    "__version_info__",
    "assume",
    "currently_in_test_context",
    "event",
    "example",
    "find",
    "given",
    "infer",
    "is_hypothesis_test",
    "note",
    "register_random",
    "reject",
    "reproduce_failure",
    "seed",
    "settings",
    "target",
]


def register_random(r: object) -> None:
    """Accepted for compatibility: Hegel does not manage the state of
    random number generators, so this does nothing."""
