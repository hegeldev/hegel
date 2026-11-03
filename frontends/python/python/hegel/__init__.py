"""Hegel: property-based testing for Python.

A test takes a :class:`TestCase` and draws values from generators::

    import hegel
    from hegel import generators as gs

    @hegel.test
    def test_sort(tc: hegel.TestCase):
        xs = tc.draw(gs.lists(gs.integers()))
        assert sorted(xs) == sorted(sorted(xs))
"""

import unicodedata as _unicodedata
from importlib import metadata as _metadata

from hegel import _native, generators
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
    NotPrintable,
    ResolutionFailed,
    RunError,
    StopTest,
    Unsatisfiable,
)
from hegel.explicit import ExplicitCase
from hegel.generators import Generator, composite
from hegel.pretty import Document, PrettyPrinter, Speculation, pretty_print, pretty_repr
from hegel.runner import (
    explicit_test_case,
    reproduce_failure,
    run,
    test,
)
from hegel.settings import (
    Backend,
    HealthCheck,
    NondeterminismStrictness,
    Phase,
    Settings,
    TestLocation,
    Verbosity,
)
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

_native.set_unicode_version(_unicodedata.unidata_version)

try:
    __version__ = _metadata.version("hegeltest")
except _metadata.PackageNotFoundError:  # nocov
    __version__ = "0+unknown"
__all__ = [
    "AssumeFailed",
    "Backend",
    "BackendError",
    "Collection",
    "ConcurrentPool",
    "DidNotReproduce",
    "Document",
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
    "NondeterminismStrictness",
    "NotPrintable",
    "Phase",
    "Pool",
    "PrettyPrinter",
    "ResolutionFailed",
    "RunError",
    "Settings",
    "Speculation",
    "StateMachine",
    "StopTest",
    "TestCase",
    "TestLocation",
    "Unsatisfiable",
    "Verbosity",
    "composite",
    "engine_version",
    "explicit_test_case",
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
]


def engine_version() -> str:
    """The version of the libhegel engine linked into this build."""
    return _version()
