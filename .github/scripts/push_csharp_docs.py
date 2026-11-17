"""Publish the .NET frontend's API references to hegel.dev/csharp and hegel.dev/fsharp.

Run by the `release-csharp-docs` job of ci.yml with the docs built
(`just csharp::check-docs` has filled frontends/csharp/docs/_site/) and a
shallow checkout of main at `publish/` that can push (the site lives in its
`website/`).
"""

import os
import re
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRONTEND = ROOT / "frontends" / "csharp"
PUBLISH = ROOT / "publish"
DOCS_SRC = FRONTEND / "docs" / "_site"
PUBLIC = PUBLISH / "website" / "public"
# The C# reference (DocFX) links relatively. Vercel's `trailingSlash: false`
# strips the trailing slash from the root page, so the browser resolves
# relative hrefs on /csharp against / — which turns `public/main.css` into
# `/public/main.css` (404). Inject <base href="/csharp/"> only on the root
# index.html to fix that one page. Subpages served at e.g. /csharp/api/...
# already have a directory-like URL, so their relative hrefs resolve
# correctly without a <base> tag. The F# reference (fsdocs) is built with
# absolute /fsharp/ links and needs none.
CSHARP_BASE_HREF = "/csharp/"
# DocFX's script does not go through <base>: it resolves the navigation bar,
# the table of contents and the search index against the page's own URL, from
# the relative paths in these <meta> tags. On the root page they point at /
# for the same reason, which leaves it with no navigation bar, so there they
# are made absolute too.
CSHARP_ROOT_META = {
    '<meta name="docfx:navrel" content="toc.html">': '<meta name="docfx:navrel" content="/csharp/toc.html">',
    '<meta name="docfx:tocrel" content="toc.html">': '<meta name="docfx:tocrel" content="/csharp/toc.html">',
    '<meta name="docfx:rel" content="">': '<meta name="docfx:rel" content="/csharp/">',
}
# What the generators emit that the site does not serve: DocFX's source maps
# (most of its output by size) and the files fsdocs copies next to its pages.
UNSERVED = ["csharp/**/*.map", "fsharp/NuGet.config", "fsharp/index.md"]
PUSH_ATTEMPTS = 5


def git(*args: str) -> None:
    subprocess.run(["git", *args], check=True, cwd=PUBLISH)


def frontend_version() -> str:
    text = (FRONTEND / "Directory.Build.props").read_text()
    return re.search(r"<Version>([^<]+)</Version>", text).group(1)


def inject_base_href(root: Path, href: str) -> None:
    index = root / "index.html"
    tag = f'<base href="{href}">'
    content = index.read_text(encoding="utf-8")
    content = content.replace("<head>", f"<head>\n{tag}", 1)
    for relative, absolute in CSHARP_ROOT_META.items():
        if relative not in content:
            raise RuntimeError(f"{index} has no {relative}")
        content = content.replace(relative, absolute)
    index.write_text(content, encoding="utf-8")


def stage_docs(src: Path, public: Path) -> None:
    for site in ("csharp", "fsharp"):
        dest = public / site
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(src / site, dest)
    for pattern in UNSERVED:
        for path in public.glob(pattern):
            path.unlink()
    inject_base_href(public / "csharp", CSHARP_BASE_HREF)


def main() -> None:
    version = frontend_version()
    app_id = os.environ["HEGEL_RELEASE_APP_ID"]
    app_slug = os.environ["HEGEL_RELEASE_APP_SLUG"]

    stage_docs(DOCS_SRC, PUBLIC)

    git("config", "user.name", f"{app_slug}[bot]")
    git("config", "user.email", f"{app_id}+{app_slug}[bot]@users.noreply.github.com")

    git("add", "website/public/csharp", "website/public/fsharp")

    status = subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=PUBLISH, text=True
    )
    if not status.strip():
        print("No doc changes to publish.")
        return

    git("commit", "-m", f"Update the .NET docs to v{version}")

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
            raise RuntimeError(f"Push to main failed after {PUSH_ATTEMPTS} attempts.")
        print("Push rejected; rebasing onto origin/main and retrying.")
        time.sleep(2)
        git("fetch", "origin", "main")
        git("rebase", "--onto", "origin/main", "HEAD~1")


if __name__ == "__main__":
    main()
