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
        config._hegel_hypothesis_seed = int(seed)
    if overrides:
        settings.register_profile("hegel-pytest", settings.default, **overrides)
        settings.load_profile("hegel-pytest")


_POINTLESS_DECORATORS = [
    ("example", "hypothesis_explicit_examples"),
    ("seed", "_hypothesis_internal_use_seed"),
    ("settings", "_hypothesis_internal_settings_applied"),
    ("reproduce_example", "_hypothesis_internal_use_reproduce_failure"),
]


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_call(item: Any) -> Any:
    test = getattr(item, "obj", None)
    if test is not None and getattr(test, "is_hypothesis_test", False):
        seed = getattr(item.config, "_hegel_hypothesis_seed", None)
        if seed is not None and getattr(test, "_hypothesis_internal_use_seed", None) is None:
            from hegel.hypothesis import seed as seed_test

            seed_test(seed)(test)  # type: ignore[type-var]
        if item.get_closest_marker("parametrize") is not None:
            # pytest makes a new class instance per parametrization, which
            # would look like differing executors; see Hypothesis issue 3733.
            from hegel.hypothesis._settings import HealthCheck
            from hegel.hypothesis._settings import settings as Settings

            fn = getattr(test, "__func__", test)
            current = getattr(fn, "_hypothesis_internal_use_settings", None) or Settings.default
            if HealthCheck.differing_executors not in current.suppress_health_check:
                fn._hypothesis_internal_use_settings = Settings(
                    current,
                    suppress_health_check=[
                        *current.suppress_health_check,
                        HealthCheck.differing_executors,
                    ],
                )
    else:
        # Hypothesis's decorators do nothing without @given, so refuse them.
        for name, attribute in _POINTLESS_DECORATORS:
            if getattr(test, attribute, None):
                from hegel.errors import InvalidArgument

                raise InvalidArgument(
                    f"Using `@{name}` on a test without `@given` is completely pointless."
                )
    yield
