"""Check that a source distribution's metadata names only files it contains.

PyPI rejects an upload whose ``License-File`` entries point at paths the
archive lacks, and ``twine check`` does not look. maturin builds a Cargo
workspace member's sdist with the workspace as its root, so a license file it
discovers next to pyproject.toml lands under ``frontends/python/`` unless
pyproject.toml lists it in ``license-files``; the 0.2.0 sdist was rejected
for exactly that.
"""

import sys
import tarfile
from email.parser import HeaderParser
from pathlib import PurePosixPath


def problems(path: str) -> list[str]:
    with tarfile.open(path) as archive:
        names = set(archive.getnames())
        roots = {PurePosixPath(name).parts[0] for name in names}
        if len(roots) != 1:
            return [f"expected one top-level directory, found {sorted(roots)}"]
        root = roots.pop()
        member = archive.extractfile(f"{root}/PKG-INFO")
        if member is None:
            return [f"no {root}/PKG-INFO"]
        metadata = HeaderParser().parsestr(member.read().decode())
    license_files = metadata.get_all("License-File") or []
    if not license_files:
        return ["PKG-INFO declares no License-File"]
    return [
        f"License-File {name} is declared but {root}/{name} is not in the archive"
        for name in license_files
        if f"{root}/{name}" not in names
    ]


def main(paths: list[str]) -> int:
    if not paths:
        print("usage: check_sdist.py SDIST...", file=sys.stderr)
        return 2
    failed = False
    for path in paths:
        found = problems(path)
        for problem in found:
            print(f"{path}: {problem}", file=sys.stderr)
        failed |= bool(found)
        if not found:
            print(f"{path}: metadata names only files the archive contains")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
