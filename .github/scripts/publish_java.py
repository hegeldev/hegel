"""Publish the Java frontend to Maven Central.

Run by the `release-java` job of ci.yml from a checkout of the
`frontends/java/<version>` tag, with the Central Portal credentials
(`CENTRAL_TOKEN_USER` / `CENTRAL_TOKEN_PASS`) in the environment and the
release signing key in the runner's keyring (`GPG_KEYNAME` pins it by
fingerprint). `mvn -P release deploy` builds and signs the three artifacts
(`dev.hegel:hegel-lowlevel`, `dev.hegel:hegel`, `dev.hegel:hegel-jna`),
bundling every platform's libhegel from the GitHub release of the engine
version in libhegel/Cargo.toml, and uploads them as one deployment that the
Central Portal validates and publishes server-side.

The publish is skipped when the version is already on Central, so a re-run
after a partial failure is safe. The deploy can also exit nonzero even
though the release went (or is going) out: the plugin's wait-until-published
poll can hit a transient error after the bundle uploaded. So on failure the
version is checked against the Central Publisher API, polling for a while if
the upload succeeded, and the publish counts as done once it is live.
"""

from __future__ import annotations

import base64
import json
import os
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRONTEND = ROOT / "frontends" / "java"

NAMESPACE = "dev.hegel"
ARTIFACTS = ["hegel-lowlevel", "hegel", "hegel-jna"]

# Printed by central-publishing-maven-plugin once the bundle is on the portal.
# From that point the deployment validates and publishes server-side
# (autoPublish) no matter how the mvn process exits.
UPLOAD_MARKER = "Uploaded bundle successfully"

CENTRAL_PUBLISHED_URL = "https://central.sonatype.com/api/v1/publisher/published"
PUBLISH_WAIT_SECONDS = 10 * 60
PUBLISH_POLL_SECONDS = 30


def pom_version() -> str:
    match = re.search(r"<version>([^<]+)</version>", (FRONTEND / "pom.xml").read_text())
    if match is None:
        raise ValueError("could not find <version> in frontends/java/pom.xml")
    return match.group(1)


def is_published(version: str) -> bool:
    """Whether every artifact at `version` is live on Maven Central. Network and
    server errors count as "not (yet) published", so a transiently failing
    status endpoint just means polling again."""
    credentials = f"{os.environ['CENTRAL_TOKEN_USER']}:{os.environ['CENTRAL_TOKEN_PASS']}"
    token = base64.b64encode(credentials.encode()).decode()
    for name in ARTIFACTS:
        query = urllib.parse.urlencode({"namespace": NAMESPACE, "name": name, "version": version})
        request = urllib.request.Request(
            f"{CENTRAL_PUBLISHED_URL}?{query}", headers={"Authorization": f"Bearer {token}"}
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                if not json.load(response).get("published"):
                    return False
        except (urllib.error.URLError, TimeoutError, ValueError):
            return False
    return True


def run_deploy(mvn_args: list[str]) -> tuple[int, bool]:
    """Run `mvn deploy`, echoing its output, and return (exit code, whether the
    bundle upload succeeded)."""
    process = subprocess.Popen(
        mvn_args, cwd=FRONTEND, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
    )
    uploaded = False
    assert process.stdout is not None
    for line in process.stdout:
        print(line, end="", flush=True)
        if UPLOAD_MARKER in line:
            uploaded = True
    return process.wait(), uploaded


def deploy_and_verify(mvn_args: list[str], version: str) -> None:
    returncode, uploaded = run_deploy(mvn_args)
    if returncode == 0:
        return
    error = subprocess.CalledProcessError(returncode, mvn_args)
    deadline = time.monotonic() + (PUBLISH_WAIT_SECONDS if uploaded else 0)
    while True:
        if is_published(version):
            print(f"mvn deploy failed (exit {returncode}) but {version} is published on Central; continuing.")
            return
        if time.monotonic() >= deadline:
            raise error
        print(f"mvn deploy failed (exit {returncode}) after upload; waiting for {version} to publish...")
        time.sleep(PUBLISH_POLL_SECONDS)


def main() -> None:
    version = pom_version()
    if is_published(version):
        print(f"{NAMESPACE} {version} already published, skipping")
        return
    mvn_args = ["mvn", "-B", "-P", "release", "deploy"]
    # Pin the signing key by fingerprint so the build signs with the Hegel
    # release key specifically and fails if that key is not in the keyring,
    # rather than falling back to whatever gpg's default key is.
    keyname = os.environ.get("GPG_KEYNAME")
    if keyname:
        mvn_args.append(f"-Dgpg.keyname={keyname}")
    deploy_and_verify(mvn_args, version)


if __name__ == "__main__":
    main()
