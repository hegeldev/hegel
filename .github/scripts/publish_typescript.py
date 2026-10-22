"""Publish the TypeScript frontend to npm.

Run by the `release-typescript` job of ci.yml from a checkout of the
`frontends/typescript/<version>` tag, after `just typescript::build`. The five
platform packages (`@hegeldev/hegel-<os>-<arch>`, each one libhegel shared
library fetched from the GitHub release of the engine version in
libhegel/Cargo.toml) are published first and pinned into the main package's
`optionalDependencies` (which the committed manifest deliberately leaves out,
see scripts/make-platform-packages.mjs); then the main package goes out.
Every publish is skipped when that version is already on the registry, so a
re-run after a partial failure is safe.
"""

from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRONTEND = ROOT / "frontends" / "typescript"

# A fresh publish lags on the registry read path: npm processes publishes
# asynchronously ("may take a few minutes to become available"), and one
# release saw a package take over two minutes to appear. The main package
# must not be published until every platform package it pins resolves.
REGISTRY_VISIBILITY_TIMEOUT = 15 * 60
REGISTRY_POLL_INTERVAL = 10


def is_published(name: str, version: str) -> bool:
    result = subprocess.run(
        ["npm", "view", f"{name}@{version}", "version"],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and result.stdout.strip() == version


def publish(package_dir: Path) -> tuple[str, str]:
    manifest = json.loads((package_dir / "package.json").read_text())
    name, version = manifest["name"], manifest["version"]
    if is_published(name, version):
        print(f"{name}@{version} already published, skipping")
    else:
        subprocess.run(["npm", "publish"], check=True, cwd=package_dir)
    return name, version


def wait_until_visible(packages: list[tuple[str, str]]) -> None:
    start = time.monotonic()
    for name, version in packages:
        while not is_published(name, version):
            elapsed = int(time.monotonic() - start)
            if elapsed >= REGISTRY_VISIBILITY_TIMEOUT:
                raise RuntimeError(
                    f"{name}@{version} did not appear on the registry within "
                    f"{REGISTRY_VISIBILITY_TIMEOUT} seconds"
                )
            print(f"waiting for {name}@{version} to appear on the registry ({elapsed}s elapsed)")
            time.sleep(REGISTRY_POLL_INTERVAL)


def main() -> None:
    subprocess.run(
        ["node", "scripts/make-platform-packages.mjs", "--pin"], check=True, cwd=FRONTEND
    )
    platform_packages = [
        publish(package_dir)
        for package_dir in sorted((FRONTEND / "platform-packages").iterdir())
    ]
    wait_until_visible(platform_packages)
    publish(FRONTEND)


if __name__ == "__main__":
    main()
