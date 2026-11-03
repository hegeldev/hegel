"""Publish the OCaml frontend's API reference to hegel.dev/ocaml.

Run by the `release-ocaml-docs` job of ci.yml with the docs built
(`just ocaml::check-docs` has filled frontends/ocaml/_build/default/_doc/_html/)
and a shallow checkout of main at `publish/` that can push (the site lives in
its `website/`).
"""

import os
import re
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRONTEND = ROOT / "frontends" / "ocaml"
PUBLISH = ROOT / "publish"
DOCS_SRC = FRONTEND / "_build" / "default" / "_doc" / "_html"
DOCS_DEST = PUBLISH / "website" / "public" / "ocaml"
# The docs are served at hegel.dev/ocaml. Vercel's `trailingSlash: false` +
# `cleanUrls: true` serve `/ocaml/hegel/Hegel/index.html` at the URL
# `/ocaml/hegel/Hegel` (no trailing slash), so the browser's base directory is
# `/ocaml/hegel/` — one level shallower than the file's real location. odoc's
# relative asset hrefs (`../../odoc.support/odoc.css`, `hegel/index.html`) then
# resolve one directory too high and 404. Injecting an absolute `<base href>`
# equal to each page's own directory makes every relative URL resolve against
# the file's true location regardless of how the slash is stripped.
DOCS_URL_PREFIX = "/ocaml"
PUSH_ATTEMPTS = 5


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, cwd=PUBLISH)


def frontend_version() -> str:
    text = (FRONTEND / "dune-project").read_text()
    return re.search(r"^\(version ([^)]+)\)", text, re.MULTILINE).group(1)


def inject_base_href(root: Path) -> None:
    for html_path in root.rglob("*.html"):
        rel_dir = html_path.parent.relative_to(root).as_posix()
        href = DOCS_URL_PREFIX + "/" + (f"{rel_dir}/" if rel_dir != "." else "")
        tag = f'<base href="{href}">'
        content = html_path.read_text(encoding="utf-8")
        content = content.replace("<head>", f"<head>\n{tag}", 1)
        html_path.write_text(content, encoding="utf-8")


def push_with_retry() -> None:
    # Other merges and sibling hegel language releases can push to main at
    # the same time. Each touches a distinct path, so a rebase onto the winner is
    # always clean: on rejection, fetch + rebase + retry.
    for attempt in range(PUSH_ATTEMPTS):
        result = subprocess.run(["git", "push", "origin", "HEAD:main"], cwd=PUBLISH)
        if result.returncode == 0:
            return
        if attempt < PUSH_ATTEMPTS - 1:
            time.sleep(3)
            git("fetch", "origin", "main")
            git("rebase", "--onto", "origin/main", "HEAD~1")
    raise RuntimeError(f"Push to main failed after {PUSH_ATTEMPTS} attempts.")


def main() -> None:
    version = frontend_version()
    app_id = os.environ["HEGEL_RELEASE_APP_ID"]
    app_slug = os.environ["HEGEL_RELEASE_APP_SLUG"]

    if DOCS_DEST.exists():
        shutil.rmtree(DOCS_DEST)
    # dune emits `_build` files read-only; copy content only (shutil.copyfile,
    # not the default copy2) so the destination is writable for the base-href
    # rewrite below.
    shutil.copytree(DOCS_SRC, DOCS_DEST, copy_function=shutil.copyfile)
    inject_base_href(DOCS_DEST)
    # Serve the Hegel module page at /ocaml. The odoc-generated /ocaml index is
    # just a package listing; readers want the module docs. Safe because
    # inject_base_href already gave this file an absolute
    # `<base href="/ocaml/hegel/Hegel/">`, so its relative links keep resolving
    # after the copy regardless of the URL it is served at.
    shutil.copyfile(DOCS_DEST / "hegel" / "Hegel" / "index.html", DOCS_DEST / "index.html")

    git("config", "user.name", f"{app_slug}[bot]")
    git("config", "user.email", f"{app_id}+{app_slug}[bot]@users.noreply.github.com")

    git("add", "website/public/ocaml")

    status = subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=PUBLISH, text=True
    )
    if not status.strip():
        print("No doc changes to publish.")
        return

    git("commit", "-m", f"Update the OCaml docs to v{version}")
    push_with_retry()


if __name__ == "__main__":
    main()
