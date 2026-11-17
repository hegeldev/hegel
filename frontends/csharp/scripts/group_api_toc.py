#!/usr/bin/env python3
"""Group the C# API reference's table of contents by what a type is for.

`docfx metadata` lists every type of a namespace alphabetically, and nearly
all of `Hegel` is one namespace. Run between `docfx metadata` and `docfx
build`, this rewrites the `toc.yml` it is given so that each namespace holds
groups instead: the types a test is written against first, then the
generator classes, the enums, the exceptions and the attributes. A type's
group is read off its kind and its base classes, which `docfx metadata`
wrote into the type's own file next to the table of contents, so a new type
needs no entry here.
"""

import sys
from pathlib import Path

HEADER = "### YamlMime:TableOfContent"
GROUPS = ("Core", "Generators", "Enums", "Exceptions", "Attributes")
GENERATOR = "Hegel.Generator`1"


def base_classes(api: Path, uid: str) -> list[str]:
    path = api / f"{uid.replace('`', '-')}.yml"
    if not path.exists():
        raise SystemExit(f"group_api_toc: no metadata file for {uid} at {path}")
    lines = path.read_text().split("\n")
    if "  inheritance:" not in lines:
        return []
    bases = []
    for line in lines[lines.index("  inheritance:") + 1 :]:
        if not line.startswith("  - "):
            break
        bases.append(line[4:])
    return bases


def group_of(uid: str, kind: str, bases: list[str]) -> str:
    if "System.Exception" in bases:
        return "Exceptions"
    if "System.Attribute" in bases:
        return "Attributes"
    if uid == GENERATOR or any(base.startswith("Hegel.Generator{") for base in bases):
        return "Generators"
    if kind == "Enum":
        return "Enums"
    return "Core"


def field(line: str, indent: str, key: str) -> str:
    prefix = f"{indent}{key}: "
    if not line.startswith(prefix):
        raise SystemExit(f"group_api_toc: expected {prefix!r}, found {line!r}")
    return line[len(prefix) :]


def regroup(text: str, api: Path) -> str:
    lines = text.rstrip("\n").split("\n")
    if lines[:2] != [HEADER, "items:"]:
        raise SystemExit("group_api_toc: not a docfx table of contents")
    out = lines[:2]
    position = 2
    while position < len(lines) and lines[position].startswith("- "):
        out.append(f"- uid: {field(lines[position], '- ', 'uid')}")
        out.append(f"  name: {field(lines[position + 1], '  ', 'name')}")
        if field(lines[position + 2], "  ", "type") != "Namespace":
            raise SystemExit(f"group_api_toc: {lines[position]!r} is not a namespace")
        if lines[position + 3] != "  items:":
            raise SystemExit(f"group_api_toc: {lines[position]!r} has no items")
        out += lines[position + 2 : position + 4]
        position += 4
        groups: dict[str, list[str]] = {group: [] for group in GROUPS}
        while position < len(lines) and lines[position].startswith("  "):
            entry = lines[position : position + 3]
            uid = field(entry[0], "  - ", "uid")
            field(entry[1], "    ", "name")
            kind = field(entry[2], "    ", "type")
            group = group_of(uid, kind, base_classes(api, uid))
            groups[group] += [f"    {line}" for line in entry]
            position += 3
        for group in GROUPS:
            if groups[group]:
                out += [f"  - name: {group}", "    items:", *groups[group]]
    return "\n".join(out + lines[position:]) + "\n"


def main() -> None:
    path = Path(sys.argv[1])
    path.write_text(regroup(path.read_text(), path.parent))


if __name__ == "__main__":
    main()
