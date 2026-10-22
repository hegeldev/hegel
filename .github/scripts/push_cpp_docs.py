"""Publish the C++ frontend's API reference to hegel.dev/cpp.

Run by the `release-cpp-docs` job of ci.yml with the documentation built
(`just cpp::check-docs` has filled frontends/cpp/build/docs/html/) and the
hegeldev/website repository checked out at `website/`.
"""

import os
import re
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRONTEND = ROOT / "frontends" / "cpp"
WEBSITE = ROOT / "website"
DOCS_SRC = FRONTEND / "build" / "docs" / "html"
DOCS_DEST = WEBSITE / "public" / "cpp"
# The docs are served at hegel.dev/cpp. Vercel's `trailingSlash: false`
# strips the trailing slash from the root page, so the browser resolves
# relative hrefs on /cpp against / — which turns `doxygen.css` into
# `/doxygen.css` (404). Doxygen's output is flat (every page sits next to
# its assets), so every page gets <base href="/cpp/">.
BASE_HREF = "/cpp/"
PUSH_ATTEMPTS = 5


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, cwd=WEBSITE)


def current_version() -> str:
    """The frontend's version, from the `project(... VERSION x.y.z ...)`
    line of its CMakeLists.txt."""
    text = (FRONTEND / "CMakeLists.txt").read_text()
    m = re.search(r"^project\(\s*hegel-cpp\s+VERSION\s+(\d+\.\d+\.\d+)", text, re.MULTILINE)
    if m is None:
        raise ValueError("no project(hegel-cpp VERSION ...) in frontends/cpp/CMakeLists.txt")
    return m.group(1)


def inject_base_href(root: Path, href: str) -> None:
    tag = f'<base href="{href}">'
    for page in root.rglob("*.html"):
        content = page.read_text(encoding="utf-8")
        content = content.replace("<head>", f"<head>\n{tag}", 1)
        page.write_text(content, encoding="utf-8")


def main() -> None:
    version = current_version()
    app_id = os.environ["HEGEL_RELEASE_APP_ID"]
    app_slug = os.environ["HEGEL_RELEASE_APP_SLUG"]

    if DOCS_DEST.exists():
        shutil.rmtree(DOCS_DEST)
    shutil.copytree(DOCS_SRC, DOCS_DEST)
    inject_base_href(DOCS_DEST, BASE_HREF)

    git("config", "user.name", f"{app_slug}[bot]")
    git("config", "user.email", f"{app_id}+{app_slug}[bot]@users.noreply.github.com")

    git("add", "public/cpp")

    status = subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=WEBSITE, text=True
    )
    if not status.strip():
        print("No doc changes to publish.")
        return

    git("commit", "-m", f"Update the C++ docs to v{version}")

    # Sibling hegel language releases can push to website main at the same
    # time. On a rejected push, rebase onto the new main and retry — each
    # language's docs live at a distinct path, so the rebase is clean. (Two
    # overlapping releases of this frontend would conflict; the rebase then
    # fails the job loudly and rerunning the newer job recovers.)
    for attempt in range(PUSH_ATTEMPTS):
        result = subprocess.run(["git", "push", "origin", "HEAD:main"], cwd=WEBSITE)
        if result.returncode == 0:
            return
        if attempt == PUSH_ATTEMPTS - 1:
            raise RuntimeError(
                f"Push to website main failed after {PUSH_ATTEMPTS} attempts."
            )
        print("Push rejected; rebasing onto origin/main and retrying.")
        time.sleep(2)
        git("fetch", "origin", "main")
        git("rebase", "origin/main")


if __name__ == "__main__":
    main()
