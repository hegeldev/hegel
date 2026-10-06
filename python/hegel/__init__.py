"""Hegel: property-based testing for Python.

A test takes a :class:`TestCase` and draws values from generators::

    import hegel
    from hegel import generators as gs

    @hegel.test
    def test_sort(tc: hegel.TestCase):
        xs = tc.draw(gs.lists(gs.integers()))
        assert sorted(xs) == sorted(sorted(xs))
"""

from hegel import generators
from hegel._native import version as _version
from hegel.errors import (
    AssumeFailed,
    BackendError,
    DidNotReproduce,
    FailedHealthCheck,
    Flaky,
    HegelError,
    InternalError,
    InvalidArgument,
    MultipleFailures,
    RunError,
    StopTest,
    Unsatisfiable,
)
from hegel.explicit import ExplicitCase
from hegel.generators import Generator, composite
from hegel.pretty import PrettyPrinter, pretty_print, pretty_repr
from hegel.runner import (
    explicit_test_case,
    find_example,
    reproduce_failure,
    run,
    test,
    with_output_override,
)
from hegel.settings import Backend, HealthCheck, Phase, Settings, TestLocation, Verbosity
from hegel.stateful import (
    ConcurrentPool,
    Machine,
    Pool,
    StateMachine,
    invariant,
    rule,
    run_state_machine,
    state_machine_test,
)
from hegel.test_case import Collection, TestCase

__version__ = "0.1.0"
__all__ = [
    "AssumeFailed",
    "Backend",
    "BackendError",
    "Collection",
    "ConcurrentPool",
    "DidNotReproduce",
    "ExplicitCase",
    "FailedHealthCheck",
    "Flaky",
    "Generator",
    "HealthCheck",
    "HegelError",
    "InternalError",
    "InvalidArgument",
    "Machine",
    "MultipleFailures",
    "Phase",
    "Pool",
    "PrettyPrinter",
    "RunError",
    "Settings",
    "StateMachine",
    "StopTest",
    "TestCase",
    "TestLocation",
    "Verbosity",
    "composite",
    "engine_version",
    "explicit_test_case",
    "find_example",
    "generators",
    "invariant",
    "pretty_print",
    "pretty_repr",
    "reproduce_failure",
    "rule",
    "run",
    "run_state_machine",
    "state_machine_test",
    "test",
    "Unsatisfiable",
    "with_output_override",
]


def engine_version() -> str:
    """The version of the libhegel engine linked into this build."""
    return _version()
