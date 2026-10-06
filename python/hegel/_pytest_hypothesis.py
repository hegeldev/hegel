"""The hegel.hypothesis half of the pytest plugin: Hypothesis's own
``--hypothesis-*`` options, and its checks on how tests are decorated.

Kept apart from :mod:`hegel.pytest_plugin`, which calls into it, and
importing nothing from :mod:`hegel.hypothesis` until a test or option needs
it, so a run that doesn't use hegel.hypothesis never loads it.
"""

from __future__ import annotations

from typing import Any


def addoption(group: Any) -> None:
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


def configure(config: Any) -> None:
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


def before_call(item: Any) -> None:
    """Prepare a test to run: apply ``--hypothesis-seed`` to a @given test,
    and refuse Hypothesis's decorators on a test without @given."""
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
        for name, attribute in _POINTLESS_DECORATORS:
            if getattr(test, attribute, None):
                from hegel.errors import InvalidArgument

                raise InvalidArgument(
                    f"Using `@{name}` on a test without `@given` is completely pointless."
                )
