#!/usr/bin/env python3
"""Check that the #define constants lib/ffi/ffi.ml.in copies match hegel.h.

ctypes cannot read a C #define, so lib/ffi/ffi.ml.in copies each one it needs
as an OCaml literal, under a comment naming it: `(* [HEGEL_NAME]. *)`
followed by `let name = <expr>`. This script evaluates both sides and exits
non-zero if any copy differs from the header, or names a #define the header
does not have.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEADER = ROOT.parent.parent / "libhegel" / "include" / "hegel.h"
FFI = ROOT / "lib" / "ffi" / "ffi.ml.in"

C_LIMITS = {
    "UINT64_MAX": 2**64 - 1,
    "INT64_MAX": 2**63 - 1,
    "INT64_MIN": -(2**63),
}

UINT64_MAX = 2**64 - 1

OCAML_EXPRS = [
    (re.compile(r"^-?\d+$"), int),
    (
        re.compile(r"^Unsigned\.UInt64\.\(sub max_int one\)$"),
        lambda _: UINT64_MAX - 1,
    ),
]


def header_defines(text: str) -> dict[str, int]:
    defines = {}
    for name, expr in re.findall(r"^#define (HEGEL_\w+) (.+)$", text, re.MULTILINE):
        for limit, value in C_LIMITS.items():
            expr = expr.replace(limit, str(value))
        if not re.fullmatch(r"[\d\s()+\-*]+", expr):
            raise SystemExit(f"hegel.h: cannot evaluate {name} = {expr}")
        defines[name] = eval(expr)
    return defines


def ocaml_value(expr: str) -> int:
    for pattern, convert in OCAML_EXPRS:
        if pattern.match(expr):
            return convert(expr)
    raise SystemExit(f"ffi.ml.in: cannot evaluate {expr!r}")


def main() -> int:
    defines = header_defines(HEADER.read_text())
    copies = re.findall(
        r"^\(\* \[(HEGEL_\w+)\]\. \*\)\nlet (\w+) = (.+)$",
        FFI.read_text(),
        re.MULTILINE,
    )
    if not copies:
        print("ffi.ml.in: no tagged #define copies found")
        return 1
    failed = False
    for name, binding, expr in copies:
        if name not in defines:
            print(f"ffi.ml.in: {binding} copies {name}, which hegel.h does not define")
            failed = True
            continue
        value = ocaml_value(expr.strip())
        if value != defines[name]:
            print(
                f"ffi.ml.in: {binding} = {value}, but hegel.h defines {name} = {defines[name]}"
            )
            failed = True
    if not failed:
        print(f"{len(copies)} #define copies match hegel.h")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
