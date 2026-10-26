"""pytest integration, registered automatically when hegel is installed.

Hegel tests are marked ``hegel``, and these options adjust the default
settings profile for the run:

``--hegel-profile NAME``
    Start from the named settings profile.
``--hegel-setting KEY=VALUE``
    Set any setting, by its ``hegel.toml`` key, with the value spelled as
    in ``hegel.toml``. Repeatable.
``--hegel-seed SEED``
    Fix the seed (0 to 2**64 - 1, or ``none``).
``--hegel-verbosity {quiet,normal,verbose,debug}``
    Set the verbosity.
``--hegel-derandomize``
    Derive each test's seed from the test itself.
``--hegel-database PATH``
    Keep the example database in ``PATH``; ``--hegel-database=`` (empty)
    or ``disabled`` disables it, and ``default`` is the default directory.
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
        "--hegel-setting",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="Set a Hegel setting by its hegel.toml key, e.g. test_cases=1000 (repeatable)",
    )
    group.addoption(
        "--hegel-seed", action="store", default=None, help="Seed for Hegel's generation"
    )
    group.addoption(
        "--hegel-verbosity",
        action="store",
        default=None,
        help="Hegel output verbosity: quiet, normal, verbose or debug",
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


def _settings_from_options(config: Any) -> list[tuple[str, str, str]]:
    """Each setting the options give, as ``(option, key, value)`` with the
    key and value as the engine reads them from ``hegel.toml``."""
    values = []
    for option in config.getoption("--hegel-setting"):
        key, equals, value = option.partition("=")
        if not equals:
            raise pytest.UsageError(f"--hegel-setting={option!r} must be KEY=VALUE")
        values.append(("--hegel-setting", key.strip(), value.strip()))
    seed = config.getoption("--hegel-seed")
    if seed is not None:
        values.append(("--hegel-seed", "seed", seed))
    verbosity = config.getoption("--hegel-verbosity")
    if verbosity is not None:
        values.append(("--hegel-verbosity", "verbosity", verbosity))
    if config.getoption("--hegel-derandomize"):
        values.append(("--hegel-derandomize", "derandomize", "true"))
    database = config.getoption("--hegel-database")
    if database is not None:
        values.append(("--hegel-database", "database", database or "disabled"))
    return values


def pytest_configure(config: Any) -> None:
    config.addinivalue_line("markers", "hegel: a Hegel property-based test")
    profile = config.getoption("--hegel-profile")
    values = _settings_from_options(config)
    if profile is None and not values:
        return
    from hegel import _native
    from hegel.errors import InvalidArgument
    from hegel.settings import Settings, _set_from_string

    settings = Settings() if profile is None else Settings.from_profile(profile)
    native = settings._build()
    for option, key, value in values:
        try:
            _set_from_string(native, key, value)
        except InvalidArgument as e:
            raise pytest.UsageError(f"{option}: {e}") from None
    _native.register_profile("pytest", native)
    Settings.set_default_profile("pytest")


def pytest_itemcollected(item: Any) -> None:
    test = getattr(item, "obj", None)
    if getattr(test, "is_hegel_test", False):
        item.add_marker(pytest.mark.hegel)
