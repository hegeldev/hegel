"""Publish the C# frontend to nuget.org.

Run by the `release-csharp` job of ci.yml from a checkout of the
`frontends/csharp/<version>` tag, with a short-lived nuget.org API key in
`NUGET_API_KEY` (obtained by the NuGet/login action through trusted
publishing). `dotnet pack` with `HegelNativesSource=release` builds the
`Hegel` package, bundling every platform's libhegel from the GitHub release of
the engine version in libhegel/Cargo.toml, and the `Hegel.FSharp` package that
depends on it at the same version; `dotnet nuget push` uploads each together
with its symbol package, `Hegel` first so that `Hegel.FSharp` never points at
a version that is not there.

A package whose version is already on nuget.org is skipped, so a re-run after
a partial failure is safe; a push that races an earlier one is made idempotent
by `--skip-duplicate`.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRONTEND = ROOT / "frontends" / "csharp"

PACKAGES = {
    "Hegel": FRONTEND / "src" / "Hegel" / "Hegel.csproj",
    "Hegel.FSharp": FRONTEND / "src" / "Hegel.FSharp" / "Hegel.FSharp.fsproj",
}
SOURCE = "https://api.nuget.org/v3/index.json"


def props_version() -> str:
    match = re.search(r"<Version>([^<]+)</Version>", (FRONTEND / "Directory.Build.props").read_text())
    if match is None:
        raise ValueError("could not find <Version> in frontends/csharp/Directory.Build.props")
    return match.group(1)


def is_published(package: str, version: str) -> bool:
    """Whether `version` of `package` is listed on nuget.org. A package that
    has never been published returns 404, which counts as not published."""
    url = f"https://api.nuget.org/v3-flatcontainer/{package.lower()}/index.json"
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            return version in json.load(response).get("versions", [])
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise


def publish(package: str, project: Path, version: str) -> None:
    if is_published(package, version):
        print(f"{package} {version} is already on nuget.org; nothing to publish.")
        return

    dist = FRONTEND / "dist"
    subprocess.run(
        [
            "dotnet",
            "pack",
            str(project),
            "-c",
            "Release",
            "-p:HegelNativesSource=release",
            "-o",
            str(dist),
        ],
        check=True,
    )
    nupkg = dist / f"{package}.{version}.nupkg"
    if not nupkg.exists():
        sys.exit(f"dotnet pack did not produce {nupkg}")
    subprocess.run(
        [
            "dotnet",
            "nuget",
            "push",
            str(nupkg),
            "--api-key",
            os.environ["NUGET_API_KEY"],
            "--source",
            SOURCE,
            "--skip-duplicate",
        ],
        check=True,
    )
    print(f"Published {package} {version}.")


def main() -> None:
    version = props_version()
    for package, project in PACKAGES.items():
        publish(package, project, version)


if __name__ == "__main__":
    main()
