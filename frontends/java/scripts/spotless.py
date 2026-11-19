#!/usr/bin/env python3
"""Run a Spotless goal on a JDK palantir-java-format can run on.

palantir-java-format reads javac internals that JDK 26 and later removed, so
on a newer JDK it fails with a `NoSuchFieldError` on every file. Spotless runs
the formatter inside Maven's own JVM, so a Maven toolchain cannot route around
it. This runs `mvn spotless:<goal>` under the JDK Maven would use when that is
new enough and old enough, and otherwise under the newest supported JDK it
finds installed (`HEGEL_FORMAT_JAVA_HOME` names one explicitly), failing with
instructions when there is none.

Usage: spotless.py apply|check
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OLDEST = 17
NEWEST = 25


def major(home: Path) -> int | None:
    """The feature release of the JDK at `home`, from its `release` file."""
    try:
        text = (home / "release").read_text()
    except OSError:
        return None
    match = re.search(r'^JAVA_VERSION="(\d+)', text, re.MULTILINE)
    return int(match.group(1)) if match else None


def current_home() -> Path | None:
    """The JDK Maven would run on: `JAVA_HOME`, else the `java` on `PATH`."""
    if os.environ.get("JAVA_HOME"):
        return Path(os.environ["JAVA_HOME"])
    java = shutil.which("java")
    if java is None:
        return None
    home = Path(java).resolve().parent.parent
    return home if (home / "release").exists() else None


def candidates() -> list[Path]:
    """Every JDK home this machine has in the usual install locations."""
    homes: list[Path] = []
    for pattern in (
        "/opt/homebrew/opt/openjdk*/libexec/openjdk.jdk/Contents/Home",
        "/usr/local/opt/openjdk*/libexec/openjdk.jdk/Contents/Home",
        "/Library/Java/JavaVirtualMachines/*/Contents/Home",
        str(Path.home() / "Library/Java/JavaVirtualMachines/*/Contents/Home"),
        "/usr/lib/jvm/*",
        str(Path.home() / ".sdkman/candidates/java/*"),
    ):
        root = Path(pattern.split("*")[0]).parent
        homes.extend(sorted(root.glob(str(Path(pattern).relative_to(root)))))
    return homes


def supported(home: Path | None) -> bool:
    version = None if home is None else major(home)
    return version is not None and OLDEST <= version <= NEWEST


def formatter_home() -> Path | None:
    """The JDK to format on, or None when there is no supported one."""
    explicit = os.environ.get("HEGEL_FORMAT_JAVA_HOME")
    if explicit:
        return Path(explicit) if supported(Path(explicit)) else None
    current = current_home()
    if supported(current):
        return current
    found = [home for home in candidates() if supported(home)]
    return max(found, key=lambda home: major(home) or 0) if found else None


def main() -> int:
    if len(sys.argv) != 2 or sys.argv[1] not in ("apply", "check"):
        print(__doc__, file=sys.stderr)
        return 2
    home = formatter_home()
    if home is None:
        where = (
            f"HEGEL_FORMAT_JAVA_HOME={os.environ['HEGEL_FORMAT_JAVA_HOME']} is not one"
            if os.environ.get("HEGEL_FORMAT_JAVA_HOME")
            else "none is installed in the usual places"
        )
        print(
            f"palantir-java-format needs a JDK from {OLDEST} to {NEWEST}, and {where}. "
            f"Install one (`brew install openjdk@{NEWEST}`, or your package manager's "
            f"openjdk-{NEWEST}) or point HEGEL_FORMAT_JAVA_HOME at one.",
            file=sys.stderr,
        )
        return 1
    env = dict(os.environ, JAVA_HOME=str(home))
    env["PATH"] = f"{home / 'bin'}{os.pathsep}{env.get('PATH', '')}"
    return subprocess.call(["mvn", "-B", f"spotless:{sys.argv[1]}"], cwd=ROOT, env=env)


if __name__ == "__main__":
    sys.exit(main())
