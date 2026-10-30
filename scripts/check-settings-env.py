#!/usr/bin/env python3
"""Check that every per-run setting in the C ABI has its environment
variable.

A setting is not done until it is reachable every way the others are (see
the `new-setting` skill), and the `HEGEL_<SETTING>` variable is the one
way to change a setting for a single run of a test whose command line the
test harness owns. This script pairs each `hegel_settings_set_<name>`
function declared in `libhegel/include/hegel.h` with a `HEGEL_<NAME>`
variable read in `libhegel/src/settings.rs`, and fails when one is
missing.

Settings that describe one test rather than one run have no variable:
`database_key` and `test_location` are allow-listed below. The variable
is `HEGEL_<NAME>` unless `VARIABLE_NAMES` says otherwise, and
`hegel_settings_set_from_string`, which sets a setting named at run time,
is not itself a setting.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

HEADER = Path("libhegel/include/hegel.h")
SETTINGS = Path("libhegel/src/settings.rs")

SETTER_RE = re.compile(r"\bhegel_settings_set_([a-z_]+)\s*\(")
VARIABLE_RE = re.compile(r'"(HEGEL_[A-Z_]+)"')

# Per-test identity, not per-run configuration: there is no sensible
# process-wide value for either, so neither has a variable.
PER_TEST_SETTINGS = {"database_key", "test_location"}

# `hegel_settings_set_from_string` takes the setting's name as an argument.
NOT_SETTINGS = {"from_string"}

# Variables whose name is not `HEGEL_<SETTING>`. `HEGEL_STATISTICS`
# predates the naming rule and is documented under that name.
VARIABLE_NAMES = {"show_statistics": "HEGEL_STATISTICS"}


def variable_for(setting: str) -> str:
    return VARIABLE_NAMES.get(setting, f"HEGEL_{setting.upper()}")


def main() -> int:
    setters = sorted(set(SETTER_RE.findall(HEADER.read_text())))
    variables = set(VARIABLE_RE.findall(SETTINGS.read_text()))

    missing = [
        name
        for name in setters
        if name not in PER_TEST_SETTINGS
        and name not in NOT_SETTINGS
        and variable_for(name) not in variables
    ]

    if missing:
        print(f"Settings in {HEADER} with no environment variable in {SETTINGS}:")
        print()
        for name in missing:
            print(f"  hegel_settings_set_{name}: expected {variable_for(name)}")
        print()
        print("Every per-run setting needs a HEGEL_<SETTING> variable applied in")
        print("Settings::with_env_overrides_from (see the new-setting skill), or")
        print("an entry in this script's PER_TEST_SETTINGS if it describes one")
        print("test rather than one run.")
        return 1

    print("check-settings-env: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
