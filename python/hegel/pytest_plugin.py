"""pytest integration, registered automatically when hegel is installed.

Hegel tests are marked ``hegel``, and these options adjust the default
settings profile for the run:

``--hegel-profile NAME``
    Start from the named settings profile.
``--hegel-seed SEED``
    Fix the seed.
``--hegel-verbosity {quiet,normal,verbose,debug}``
    Set the verbosity.
``--hegel-derandomize``
    Derive each test's seed from the test itself.
``--hegel-database PATH``
    Keep the example database in ``PATH``; ``--hegel-database=`` (empty)
    disables it.
"""

from __future__ import annotations

from typing import Any

import pytest


def pytest_addoption(parser: Any) -> None:
    group = parser.getgroup("hegel")
    group.addoption(
        "--hegel-profile", action="store", default=None, help="Hegel settings profile to use"
    )
    group.addoption(
        "--hegel-seed", action="store", default=None, help="Seed for Hegel's generation"
    )
    group.addoption(
        "--hegel-verbosity",
        action="store",
        default=None,
        choices=["quiet", "normal", "verbose", "debug"],
        help="Hegel output verbosity",
    )
    group.addoption(
        "--hegel-derandomize",
        action="store_true",
        default=None,
        help="Derive each Hegel test's seed from the test itself",
    )
    group.addoption(
        "--hegel-database",
        action="store",
        default=None,
        help="Directory for Hegel's example database (empty to disable it)",
    )


_OPTIONS = {
    "seed": ("--hegel-seed", int),
    "verbosity": ("--hegel-verbosity", str),
    "derandomize": ("--hegel-derandomize", bool),
    "database": ("--hegel-database", str),
}


def pytest_configure(config: Any) -> None:
    config.addinivalue_line("markers", "hegel: a Hegel property-based test")
    profile = config.getoption("--hegel-profile")
    overrides = {
        field: convert(value)
        for field, (option, convert) in _OPTIONS.items()
        if (value := config.getoption(option)) is not None
    }
    if profile is None and not overrides:
        return
    from hegel.settings import Settings

    base = Settings() if profile is None else Settings.from_profile(profile)
    Settings.register_profile("pytest", base, **overrides)
    Settings.set_default_profile("pytest")


def pytest_itemcollected(item: Any) -> None:
    test = getattr(item, "obj", None)
    if getattr(test, "is_hegel_test", False):
        item.add_marker(pytest.mark.hegel)
