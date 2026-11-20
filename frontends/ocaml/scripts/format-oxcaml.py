#!/usr/bin/env python3
"""Format the sources `dune fmt` cannot: the cppo templates (*.ml.in, *.mli.in)
and the OxCaml-only files listed in .ocamlformat-ignore.

Run with the OxCaml ocamlformat on PATH (its parser reads both OxCaml syntax
and the mode and kind syntax the cppo macros expand to). Each file is masked
into OCaml that ocamlformat parses, formatted, unmasked and written back:

- cppo directive lines and file-level modalities (`@@ portable`) become
  `(*cppo_N*)` comments, restored verbatim on a line of their own;
- an #if block whose branches are each one line inside a declaration (a kind
  annotation, an arrow parameter, a type) becomes a single token of that kind,
  restored verbatim;
- macro uses become the syntax they expand to, tagged so they can be found
  again, or an opaque kind name.

Comments and doc comments are left as written, as ocamlformat 0.29 leaves
them in the rest of the frontend. With no file arguments it formats every
such file under the frontend; --check prints the changes instead of writing
them and exits non-zero if there are any.
"""

from __future__ import annotations

import difflib
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

ITEM_START = re.compile(
    r"^\s*(let|and|module|type|val|external|open|include|exception|class|end|"
    r"#|\(\*|@@|\[@@|;;)"
)
DIRECTIVE = re.compile(r"^(#(ifdef|ifndef|else|endif|define)\b.*|@@ \w+)$")
IF = re.compile(r"^#(ifdef|ifndef)\b")
LEFTOVER = re.compile(r"__cppo\w+|cppo_\w*\d+|\[@cppo_nontail\]")


class Masker:
    def __init__(self) -> None:
        self.lines: list[str] = []
        self.blocks: list[str] = []
        self.kinds: list[str] = []

    def kind(self, text: str) -> str:
        self.kinds.append(text)
        return f"cppo_k_{len(self.kinds) - 1}"

    def mask(self, text: str) -> str:
        src = text.split("\n")
        out: list[str] = []
        i = 0
        while i < len(src):
            block = self.inline_block(src, i)
            if block is not None:
                end, body = block
                n = len(self.blocks)
                self.blocks.append("\n".join(src[i : end + 1]))
                first = body.lstrip()
                if first.startswith(":"):
                    token = f": cppo_b_{n}"
                elif first.startswith("->"):
                    token = f"-> cppo_b_{n}"
                else:
                    token = f"cppo_b_{n}"
                out[-1] = f"{out[-1]} {token}"
                i = end + 1
            elif DIRECTIVE.match(src[i]):
                out.append(f"(*cppo_{len(self.lines)}*)")
                self.lines.append(src[i])
                i += 1
            else:
                out.append(self.macros(src[i]))
                i += 1
        return "\n".join(out)

    def inline_block(self, src: list[str], i: int) -> tuple[int, str] | None:
        if i == 0 or not IF.match(src[i]):
            return None
        bodies: list[list[str]] = []
        current: list[str] = []
        j = i + 1
        while j < len(src):
            if src[j].startswith("#else"):
                bodies.append(current)
                current = []
            elif src[j].startswith("#endif"):
                bodies.append(current)
                break
            elif src[j].startswith("#"):
                return None
            else:
                current.append(src[j])
            j += 1
        else:
            return None
        if any(len(b) != 1 or ITEM_START.match(b[0]) for b in bodies):
            return None
        if not src[i - 1].strip() or DIRECTIVE.match(src[i - 1]):
            return None
        return j, bodies[0][0]

    def macros(self, line: str) -> str:
        line = re.sub(
            r"\bKIND\(([^)]*)\)", lambda m: ": " + self.kind(f"KIND({m.group(1)})"), line
        )
        line = re.sub(r"\bCROSSING\b", lambda m: ": " + self.kind("CROSSING"), line)
        line = re.sub(
            r"\bABSTRACT\((\w+), ([^)]*)\)",
            lambda m: f"(type ({m.group(1)} : "
            f"{self.kind(f'ABSTRACT({m.group(1)}, {m.group(2)})')}))",
            line,
        )
        line = re.sub(r"\bMODE\((\w+)\)", r"@ \1__cppoMODE", line)
        line = re.sub(r"\bMODALITY\((\w+)\)", r"@@ \1__cppoMODALITY", line)
        line = re.sub(r"\bLOCAL\((\w+)\)", r"(\1 @ local__cppoLOCAL)", line)
        line = re.sub(r"\bLOCAL_CTX\b", "~(ctx @ local__cppoLOCALCTX)", line)
        line = re.sub(
            r"\bCONCURRENCY_PARAM\b",
            "~(concurrency : _ Concurrency.t @ local__cppoCONCPARAM)",
            line,
        )
        line = re.sub(r"\bNONTAIL\b", "[@cppo_nontail]", line)
        line = re.sub(r"\bNONPORTABLE\b", "@@ nonportable__cppoNONPORTABLE", line)
        line = re.sub(r"\bPORTABLE\b", "@@ portable__cppoPORTABLE", line)
        return line

    def unmask(self, text: str) -> str:
        text = re.sub(
            r"[ \t]*\n?[ \t]*(?::|->)?[ \t]*\bcppo_b_(\d+)\b[ \t]*\n?",
            lambda m: "\n" + self.blocks[int(m.group(1))] + "\n",
            text,
        )
        text = re.sub(
            r"\(type\s+\(?\w+\s*:\s*cppo_k_(\d+)\)?\)",
            lambda m: self.kinds[int(m.group(1))],
            text,
        )
        text = re.sub(r":\s*cppo_k_(\d+)\b", lambda m: self.kinds[int(m.group(1))], text)
        text = re.sub(r"@\s+(\w+)__cppoMODE", r"MODE(\1)", text)
        text = re.sub(r"@@\s+(\w+)__cppoMODALITY", r"MODALITY(\1)", text)
        text = re.sub(r"\((\w+)\s+@\s+local__cppoLOCAL\)", r"LOCAL(\1)", text)
        text = re.sub(r"~\(ctx\s+@\s+local__cppoLOCALCTX\)", "LOCAL_CTX", text)
        text = re.sub(
            r"~\(concurrency\s*:\s*_\s+Concurrency\.t\s+@\s+local__cppoCONCPARAM\)",
            "CONCURRENCY_PARAM",
            text,
        )
        text = re.sub(r"\[@cppo_nontail\]", "NONTAIL", text)
        text = re.sub(r"@@\s+nonportable__cppoNONPORTABLE", "NONPORTABLE", text)
        text = re.sub(r"@@\s+portable__cppoPORTABLE", "PORTABLE", text)

        out: list[str] = []
        for line in text.split("\n"):
            pieces = re.split(r"\(\*\s*cppo_(\d+)\s*\*\)", line)
            if len(pieces) == 1:
                out.append(line.rstrip())
                continue
            indent = line[: len(line) - len(line.lstrip())]
            for k, piece in enumerate(pieces):
                if k % 2 == 1:
                    out.append(self.lines[int(piece)])
                elif piece.strip():
                    out.append(indent + piece.strip())
        return re.sub(r"\n{3,}", "\n\n", "\n".join(out))


def format_file(path: Path, check: bool) -> bool:
    name = path.name[: -len(".in")] if path.name.endswith(".in") else path.name
    stand_in = path.parent / f"__format_oxcaml__{Path(name).suffix}"
    source = path.read_text()
    masker = Masker()
    result = subprocess.run(
        [
            "ocamlformat",
            "--no-version-check",
            "--no-wrap-comments",
            "--no-parse-docstrings",
            f"--name={stand_in}",
            "-",
        ],
        input=masker.mask(source),
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    if result.returncode != 0:
        print(f"{path}: ocamlformat failed\n{result.stderr}", file=sys.stderr)
        return False
    formatted = masker.unmask(result.stdout)
    leftover = sorted(set(LEFTOVER.findall(formatted)))
    if leftover:
        print(f"{path}: masks left after formatting: {leftover}", file=sys.stderr)
        return False
    if formatted == source:
        return True
    if check:
        diff = difflib.unified_diff(
            source.splitlines(keepends=True),
            formatted.splitlines(keepends=True),
            fromfile=str(path.relative_to(ROOT)),
            tofile=f"{path.relative_to(ROOT)} (formatted)",
        )
        sys.stdout.writelines(diff)
        return False
    path.write_text(formatted)
    return True


def default_files() -> list[Path]:
    listed = subprocess.run(
        ["git", "ls-files", "*.ml.in", "*.mli.in"],
        capture_output=True,
        text=True,
        check=True,
        cwd=ROOT,
    ).stdout.split()
    ignored = (ROOT / ".ocamlformat-ignore").read_text().split()
    return [ROOT / p for p in listed + ignored]


def main() -> int:
    args = sys.argv[1:]
    check = "--check" in args
    files = [Path(a).resolve() for a in args if a != "--check"] or default_files()
    ok = all([format_file(f, check) for f in files])
    if check and not ok:
        print("Run `just ocaml::format` to format these files.", file=sys.stderr)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
