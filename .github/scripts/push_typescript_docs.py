"""Publish the TypeScript frontend's API reference to hegel.dev/typescript.

Run by the `release-typescript-docs` job of ci.yml with the frontend built
(`just typescript::check-docs` has filled frontends/typescript/docs/) and a
shallow checkout of main at `publish/` that can push (the site lives in its
`website/`).
"""

import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRONTEND = ROOT / "frontends" / "typescript"
PUBLISH = ROOT / "publish"
DOCS_SRC = FRONTEND / "docs"
DOCS_DEST = PUBLISH / "website" / "public" / "typescript"
# The docs are served at hegel.dev/typescript. Vercel's `trailingSlash: false`
# strips the trailing slash from the root page, so the browser resolves
# relative hrefs on /typescript against / — which turns `assets/style.css`
# into `/assets/style.css` (404). Inject <base href="/typescript/"> only on
# the root index.html to fix that one page. Subpages served at e.g.
# /typescript/classes/... already have a directory-like URL, so TypeDoc's
# `../assets/...` hrefs resolve correctly without a <base> tag.
BASE_HREF = "/typescript/"
PUSH_ATTEMPTS = 5


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, cwd=PUBLISH)


def inject_base_href(root: Path, href: str) -> None:
    index = root / "index.html"
    tag = f'<base href="{href}">'
    content = index.read_text(encoding="utf-8")
    content = content.replace("<head>", f"<head>\n{tag}", 1)
    index.write_text(content, encoding="utf-8")


def main() -> None:
    version = json.loads((FRONTEND / "package.json").read_text())["version"]
    app_id = os.environ["HEGEL_RELEASE_APP_ID"]
    app_slug = os.environ["HEGEL_RELEASE_APP_SLUG"]

    if DOCS_DEST.exists():
        shutil.rmtree(DOCS_DEST)
    shutil.copytree(DOCS_SRC, DOCS_DEST)
    inject_base_href(DOCS_DEST, BASE_HREF)

    git("config", "user.name", f"{app_slug}[bot]")
    git("config", "user.email", f"{app_id}+{app_slug}[bot]@users.noreply.github.com")

    git("add", "website/public/typescript")

    status = subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=PUBLISH, text=True
    )
    if not status.strip():
        print("No doc changes to publish.")
        return

    git("commit", "-m", f"Update the TypeScript docs to v{version}")

    # Other merges and sibling hegel language releases can push to main at
    # the same time. On a rejected push, rebase onto the new main and retry — each
    # language's docs live at a distinct path, so the rebase is clean. (Two
    # overlapping releases of this frontend would conflict; the rebase then
    # fails the job loudly and rerunning the newer job recovers.)
    for attempt in range(PUSH_ATTEMPTS):
        result = subprocess.run(["git", "push", "origin", "HEAD:main"], cwd=PUBLISH)
        if result.returncode == 0:
            return
        if attempt == PUSH_ATTEMPTS - 1:
            raise RuntimeError(
                f"Push to main failed after {PUSH_ATTEMPTS} attempts."
            )
        print("Push rejected; rebasing onto origin/main and retrying.")
        time.sleep(2)
        git("fetch", "origin", "main")
        git("rebase", "--onto", "origin/main", "HEAD~1")


if __name__ == "__main__":
    main()
