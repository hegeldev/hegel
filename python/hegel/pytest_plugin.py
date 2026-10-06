"""pytest integration: ``--hegel-profile`` selects the settings profile,
``--hegel-seed`` fixes the seed, and tests are marked ``hegel``."""

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


def pytest_configure(config: Any) -> None:
    config.addinivalue_line("markers", "hegel: a Hegel property-based test")
    profile = config.getoption("--hegel-profile")
    seed = config.getoption("--hegel-seed")
    verbosity = config.getoption("--hegel-verbosity")
    if profile is None and seed is None and verbosity is None:
        return
    from hegel.settings import Settings, Verbosity

    base = Settings() if profile is None else Settings.from_profile(profile)
    overrides: dict[str, Any] = {}
    if seed is not None:
        overrides["seed"] = int(seed)
    if verbosity is not None:
        overrides["verbosity"] = Verbosity[verbosity]
    Settings.register_profile("pytest", base, **overrides)
    Settings.set_default_profile("pytest")


@pytest.hookimpl(hookwrapper=True)
def pytest_collection_modifyitems(items: list[Any]) -> Any:
    yield
    for item in items:
        if getattr(getattr(item, "obj", None), "is_hegel_test", False):
            item.add_marker(pytest.mark.hegel)
