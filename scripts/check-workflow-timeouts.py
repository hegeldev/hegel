#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["pyyaml>=6,<7"]
# ///
"""Check that every GitHub Actions job that runs steps sets `timeout-minutes`.

Without one, a job whose step hangs runs until GitHub's six-hour limit,
which holds its whole run open and blocks rerunning any other job in it.

A job that calls a reusable workflow (`uses:`) cannot take a timeout: the
limit belongs on the jobs inside the workflow it calls, which this script
checks like any other.
"""

from __future__ import annotations

import sys
from pathlib import Path

import yaml

WORKFLOWS = Path(".github/workflows")


def is_timeout(value: object) -> bool:
    if isinstance(value, bool):
        return False
    if isinstance(value, (int, float)):
        return value > 0
    return isinstance(value, str) and "${{" in value


def main() -> int:
    paths = sorted([*WORKFLOWS.glob("*.yml"), *WORKFLOWS.glob("*.yaml")])
    if not paths:
        print(f"check-workflow-timeouts: no workflows found in {WORKFLOWS}")
        return 1

    missing: list[str] = []
    for path in paths:
        workflow = yaml.safe_load(path.read_text())
        for name, job in workflow["jobs"].items():
            if "uses" in job:
                continue
            if not is_timeout(job.get("timeout-minutes")):
                missing.append(f"{path}: {name}")

    if missing:
        print("Jobs with no `timeout-minutes`:")
        print()
        for entry in missing:
            print(f"  {entry}")
        print()
        print("Give each one a limit of two to three times its slowest healthy")
        print("run, so that a hung step fails in minutes and not at GitHub's")
        print("six-hour limit. A release or publish job should get a looser one.")
        return 1

    print("check-workflow-timeouts: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
