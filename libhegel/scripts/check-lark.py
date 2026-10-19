#!/usr/bin/env -S uv run --script --with lark==1.3.1
"""Check that strings the engine generates from the fixture grammars parse
with Lark itself.

Runs the engine's `write_corpus` test, which draws strings from every
fixture grammar in `libhegel/tests/fixtures/lark/` that Lark can parse
without a postlexer (grammars that `%declare` terminals need one) and in
reasonable time (a grammar whose strings take Lark cubic time to parse
says so with a `// corpus: no` line), then
parses each string with Lark's Earley parser and dynamic lexer — the
most permissive configuration, whose accepted language is the grammar's.
Any string that fails to parse fails the check. Lark 1.3.1 raises a
`RuntimeError` of its own on input ending in an ignored terminal; those
strings are counted and reported separately rather than failed.

Run with `just check-lark`; `HEGEL_LARK_CORPUS_N` sets the number of
strings per grammar (default 100).
"""

import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile

from lark import Lark

ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures" / "lark"
LARK_BUG = "multiple start symbol"
DECLARE = re.compile(r"^\s*%declare\b", re.MULTILINE)
NO_CORPUS = re.compile(r"^\s*// corpus: no$", re.MULTILINE)


def corpus_grammars() -> list[str]:
    names = []
    for path in sorted(FIXTURES.glob("*.lark")):
        text = path.read_text()
        if path.stem.startswith("err_") or DECLARE.search(text) or NO_CORPUS.search(text):
            continue
        names.append(path.stem)
    return names


def start_rule(name: str) -> str:
    first = (FIXTURES / f"{name}.expected").read_text().split("\n")[0]
    assert first.startswith("start "), first
    return first.split()[1]


def generate(names: list[str], out_dir: pathlib.Path) -> None:
    env = dict(os.environ)
    env["HEGEL_LARK_CORPUS_DIR"] = str(out_dir)
    env["HEGEL_LARK_CORPUS_GRAMMARS"] = ",".join(names)
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "hegeltest-c",
            "--release",
            "--lib",
            "lark::generate::tests::write_corpus",
            "--",
            "--ignored",
            "--nocapture",
        ],
        cwd=ROOT,
        env=env,
        check=True,
    )


def check(name: str, dump: pathlib.Path) -> int:
    parser = Lark.open(
        str(FIXTURES / f"{name}.lark"),
        start=start_rule(name),
        parser="earley",
        import_paths=[str(FIXTURES)],
    )
    strings = [json.loads(line) for line in dump.read_text().split("\n") if line]
    failures = []
    lark_bug = 0
    for s in strings:
        try:
            parser.parse(s)
        except RuntimeError as e:
            if LARK_BUG in str(e):
                lark_bug += 1
            else:
                failures.append((s, e))
        except Exception as e:  # noqa: BLE001 - any parse failure is a finding
            failures.append((s, e))
    note = f" ({lark_bug} hit Lark's trailing-ignore bug)" if lark_bug else ""
    print(f"{name}: {len(strings) - len(failures)}/{len(strings)} parse{note}")
    for s, e in failures[:5]:
        message = str(e).split("\n")[0][:200]
        print(f"  FAIL {type(e).__name__}: {message}")
        print(f"       {s[:200]!r}")
    return len(failures)


def main() -> int:
    names = corpus_grammars()
    with tempfile.TemporaryDirectory() as tmp:
        out_dir = pathlib.Path(tmp)
        generate(names, out_dir)
        failures = sum(check(name, out_dir / f"{name}.jsonl") for name in names)
    if failures:
        print(f"\n{failures} generated strings did not parse with Lark")
        return 1
    print("\nEvery generated string parses with Lark")
    return 0


if __name__ == "__main__":
    sys.exit(main())
