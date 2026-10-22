#!/usr/bin/env python3
"""The libhegel engine this frontend is built and tested against.

The engine is the in-tree libhegel: the `hegeltest-c` crate in `libhegel/`,
compiled natively by `just java::build-libhegel` into the workspace's
`target/` directory (`CARGO_TARGET_DIR` moves it, as for cargo;
`HEGEL_LIBHEGEL_PATH` points at a library built elsewhere). Maven runs this
script from the poms: `build-info` generates `BuildInfo.java` from
`libhegel/Cargo.toml`, so the version the bindings report they were built
against is always the engine they were built with, and `stage` copies the
shared libraries into a module's build output as the `native/<os>-<arch>/`
classpath resources `LibraryLoader` unpacks at runtime.

Development and tests stage the host's in-tree build alone, so the test JVM
runs the engine of the checkout through the same bundled-resource path a user
gets. The release job stages every platform's library from the GitHub release
of the libhegel version in `libhegel/Cargo.toml`, each verified against the
`.sha256` sidecar published next to it, so the published jars carry the
engine for every supported platform. The bundled native is the only way end
users get the engine (there is no download at runtime), so a release staging
that cannot produce every library fails the build.

Usage:
  scripts/engine.py version                    # print libhegel's version
  scripts/engine.py native                     # print the host's in-tree library's path
  scripts/engine.py build-info --out DIR       # write dev/hegel/lowlevel/BuildInfo.java under DIR
  scripts/engine.py stage --source local --out DIR    # stage the host's in-tree build under DIR
  scripts/engine.py stage --source release --out DIR  # stage every platform's released library under DIR
"""

from __future__ import annotations

import argparse
import hashlib
import os
import platform
import re
import shutil
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MONOREPO = ROOT.parent.parent
CARGO_TOML = MONOREPO / "libhegel" / "Cargo.toml"
NATIVE_DIR = ROOT / "native"

RELEASE_REPO = "hegeldev/hegel"

# The libraries a libhegel release publishes, keyed by the `<os>-<arch>` of
# the classpath resource directory LibraryLoader looks in for the host.
RELEASE_ASSETS = {
    "darwin-arm64": "libhegel-darwin-arm64.dylib",
    "linux-amd64": "libhegel-linux-amd64.so",
    "linux-arm64": "libhegel-linux-arm64.so",
    "windows-amd64": "libhegel-windows-amd64.dll",
    "windows-arm64": "libhegel-windows-arm64.dll",
}

# What cargo names the shared library it builds for each host.
CARGO_LIBRARIES = {
    "darwin": "libhegel_c.dylib",
    "linux": "libhegel_c.so",
    "windows": "hegel_c.dll",
}

BUNDLED_EXTENSIONS = {"darwin": "dylib", "linux": "so", "windows": "dll"}


def log(message: str) -> None:
    print(f"[engine] {message}", file=sys.stderr)


def libhegel_version() -> str:
    match = re.search(r'^version = "([^"]+)"$', CARGO_TOML.read_text(), re.MULTILINE)
    if match is None:
        raise SystemExit(f"could not find the package version in {CARGO_TOML}")
    return match.group(1)


def host_os() -> str:
    system = platform.system().lower()
    if system == "darwin":
        return "darwin"
    if system == "linux":
        return "linux"
    if system.startswith("win"):
        return "windows"
    raise SystemExit(f"unsupported host operating system {platform.system()} for libhegel")


def host_arch() -> str:
    machine = platform.machine().lower()
    if machine in ("x86_64", "amd64"):
        return "amd64"
    if machine in ("aarch64", "arm64"):
        return "arm64"
    raise SystemExit(f"unsupported host architecture {platform.machine()} for libhegel")


def target_dir() -> Path:
    override = os.environ.get("CARGO_TARGET_DIR")
    return Path(override) if override else MONOREPO / "target"


def native_library_path() -> Path:
    override = os.environ.get("HEGEL_LIBHEGEL_PATH")
    location = Path(override) if override else target_dir() / "release" / CARGO_LIBRARIES[host_os()]
    if not location.is_file():
        raise SystemExit(
            f"libhegel not found at {location}; run `just java::build-libhegel` or set HEGEL_LIBHEGEL_PATH"
        )
    return location


def bundled_name(os_name: str) -> str:
    return f"libhegel.{BUNDLED_EXTENSIONS[os_name]}"


def stage_library(source: Path, out: Path, platform_dir: str) -> None:
    dest_dir = out / "native" / platform_dir
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / bundled_name(platform_dir.split("-")[0])
    shutil.copyfile(source, dest)
    log(f"staged {source.name} -> {dest.relative_to(out)}")


def stage_local(out: Path) -> None:
    stage_library(native_library_path(), out, f"{host_os()}-{host_arch()}")


def asset_url(asset: str, version: str) -> str:
    return f"https://github.com/{RELEASE_REPO}/releases/download/libhegel/{version}/{asset}"


def download(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "hegel-java-build"})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def sidecar_digest(text: str, asset: str) -> str:
    match = re.fullmatch(r"([a-f0-9]{64})(?:\s+\*?(\S+))?", text.strip())
    if match is None or (match.group(2) is not None and match.group(2) != asset):
        raise SystemExit(f"malformed checksum sidecar for {asset}")
    return match.group(1)


def fetch_asset(asset: str, version: str) -> Path:
    dest = NATIVE_DIR / version / asset
    if dest.is_file():
        log(f"{asset} already present")
        return dest
    dest.parent.mkdir(parents=True, exist_ok=True)
    url = asset_url(asset, version)
    log(f"downloading {asset}")
    data = download(url)
    expected = sidecar_digest(download(f"{url}.sha256").decode("utf-8"), asset)
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected:
        raise SystemExit(f"{asset} does not match its published checksum: expected {expected}, got {actual}")
    partial = dest.with_name(f"{dest.name}.{os.getpid()}.partial")
    partial.write_bytes(data)
    partial.replace(dest)
    return dest


def stage_release(out: Path) -> None:
    version = libhegel_version()
    for platform_dir, asset in RELEASE_ASSETS.items():
        stage_library(fetch_asset(asset, version), out, platform_dir)


def build_info_source() -> str:
    return (
        "package dev.hegel.lowlevel;\n"
        "\n"
        "/** Generated by scripts/engine.py from libhegel/Cargo.toml. Do not edit. */\n"
        "final class BuildInfo {\n"
        "    /** The libhegel version these bindings were built against. */\n"
        f'    static final String ENGINE_VERSION = "{libhegel_version()}";\n'
        "\n"
        "    private BuildInfo() {}\n"
        "}\n"
    )


def write_build_info(out: Path) -> None:
    dest = out / "dev" / "hegel" / "lowlevel" / "BuildInfo.java"
    dest.parent.mkdir(parents=True, exist_ok=True)
    source = build_info_source()
    if not dest.is_file() or dest.read_text() != source:
        dest.write_text(source)


def main() -> None:
    parser = argparse.ArgumentParser(description="The libhegel engine this frontend is built against.")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("version")
    subparsers.add_parser("native")
    build_info = subparsers.add_parser("build-info")
    build_info.add_argument("--out", required=True, type=Path)
    stage = subparsers.add_parser("stage")
    stage.add_argument("--out", required=True, type=Path)
    stage.add_argument("--source", choices=("local", "release"), default="local")
    args = parser.parse_args()

    if args.command == "version":
        print(libhegel_version())
    elif args.command == "native":
        print(native_library_path())
    elif args.command == "build-info":
        write_build_info(args.out)
    elif args.source == "release":
        stage_release(args.out)
    else:
        stage_local(args.out)


if __name__ == "__main__":
    main()
