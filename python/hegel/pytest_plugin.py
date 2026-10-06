"""pytest integration: ``--hegel-profile`` selects the settings profile,
``--hegel-seed`` fixes the seed, and tests are marked ``hegel``."""

from __future__ import annotations

from typing import Any

import pytest


def pytest_addoption(parser: Any) -> None:
    group = parser.getgroup("hegel")
    group.addoption(
        "--hypothesis-profile",
        action="store",
        default=None,
        help="hegel.hypothesis settings profile to load",
    )
    group.addoption(
        "--hypothesis-seed",
        action="store",
        default=None,
        help="Seed for hegel.hypothesis tests",
    )
    group.addoption(
        "--hypothesis-verbosity",
        action="store",
        default=None,
        choices=["quiet", "normal", "verbose", "debug"],
        help="Verbosity of hegel.hypothesis tests",
    )
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
    config.addinivalue_line("markers", "hypothesis: a hegel.hypothesis property-based test")
    _configure_hypothesis(config)
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


def pytest_itemcollected(item: Any) -> None:
    test = getattr(item, "obj", None)
    if getattr(test, "is_hegel_test", False):
        item.add_marker(pytest.mark.hegel)
    if getattr(test, "is_hypothesis_test", False):
        item.add_marker(pytest.mark.hypothesis)


def _configure_hypothesis(config: Any) -> None:
    profile = config.getoption("--hypothesis-profile")
    seed = config.getoption("--hypothesis-seed")
    verbosity = config.getoption("--hypothesis-verbosity")
    if profile is None and seed is None and verbosity is None:
        return
    from hegel.hypothesis import Verbosity, settings

    if profile is not None:
        settings.load_profile(profile)
    overrides: dict[str, Any] = {}
    if verbosity is not None:
        overrides["verbosity"] = Verbosity[verbosity]
    if seed is not None:
        overrides["database"] = None
        overrides["derandomize"] = False
    if overrides:
        settings.register_profile("hegel-pytest", settings.default, **overrides)
        settings.load_profile("hegel-pytest")
    if seed is not None:
        config._hegel_hypothesis_seed = int(seed)


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_call(item: Any) -> Any:
    seed = getattr(item.config, "_hegel_hypothesis_seed", None)
    test = getattr(item, "obj", None)
    if seed is not None and getattr(test, "is_hypothesis_test", False):
        from hegel.hypothesis import seed as seed_test

        if getattr(test, "_hypothesis_internal_use_seed", None) is None:
            seed_test(seed)(test)
    yield
