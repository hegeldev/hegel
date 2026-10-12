#!/usr/bin/env -S uv run --script --with lark==1.3.1
"""Check that the engine's grammar parser accepts exactly the strings Lark
does.

Runs the engine's `write_corpus` test to draw strings from every fixture
grammar in `hegel-c/tests/fixtures/lark/` that Lark can parse without a
postlexer and in reasonable time (see `check-lark.py`), mutates each
string a few ways (a character deleted, duplicated, swapped with its
neighbour, or the string truncated), and asks both Lark's Earley parser
with its dynamic lexer and the engine's parser (through the
`write_verdicts` test) whether each original and mutant parses. Any disagreement fails the check. Lark 1.3.1 raises a
`RuntimeError` of its own on input ending in an ignored terminal; the
engine accepts those strings, as Lark means to, so they count as
accepted here.

Run with `just check-parse`; `HEGEL_LARK_CORPUS_N` sets the number of
strings per grammar (default 100) and `HEGEL_LARK_MUTANTS` the number of
mutants per string (default 4).
"""

import json
import os
import pathlib
import random
import re
import subprocess
import sys
import tempfile

from lark import Lark

ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "hegel-c" / "tests" / "fixtures" / "lark"
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


def run_test(test: str, env: dict[str, str]) -> None:
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "hegeltest-c",
            "--release",
            "--lib",
            test,
            "--",
            "--ignored",
            "--nocapture",
        ],
        cwd=ROOT,
        env={**os.environ, **env},
        check=True,
    )


def mutants(s: str, rng: random.Random, count: int) -> list[str]:
    out = []
    for _ in range(count):
        if not s:
            out.append("x")
            continue
        i = rng.randrange(len(s))
        kind = rng.randrange(4)
        if kind == 0:
            out.append(s[:i] + s[i + 1 :])
        elif kind == 1:
            out.append(s[:i] + s[i] + s[i:])
        elif kind == 2 and i + 1 < len(s):
            out.append(s[:i] + s[i + 1] + s[i] + s[i + 2 :])
        else:
            out.append(s[:i])
    return out


def lark_accepts(parser: Lark, s: str) -> bool:
    try:
        parser.parse(s)
    except RuntimeError as e:
        return LARK_BUG in str(e)
    except RecursionError:
        raise
    except Exception:  # noqa: BLE001 - any parse failure is a rejection
        return False
    return True


def compare(name: str, work: pathlib.Path) -> tuple[int, int]:
    parser = Lark.open(
        str(FIXTURES / f"{name}.lark"),
        start=start_rule(name),
        parser="earley",
        import_paths=[str(FIXTURES)],
    )
    candidates = [
        json.loads(line)
        for line in (work / f"{name}.candidates.jsonl").read_text().split("\n")
        if line
    ]
    verdicts = [
        json.loads(line)
        for line in (work / f"{name}.verdicts.jsonl").read_text().split("\n")
        if line
    ]
    assert len(candidates) == len(verdicts), (len(candidates), len(verdicts))
    disagreements = []
    accepted = 0
    for s, ours in zip(candidates, verdicts):
        try:
            theirs = lark_accepts(parser, s)
        except RecursionError:
            continue
        accepted += theirs
        if theirs != ours:
            disagreements.append((s, theirs))
    print(
        f"{name}: {len(candidates)} strings compared, {accepted} accepted by Lark,"
        f" {len(disagreements)} disagreements"
    )
    for s, theirs in disagreements[:5]:
        who = "Lark accepts, engine rejects" if theirs else "Lark rejects, engine accepts"
        print(f"  {who}: {s[:200]!r}")
    return len(candidates), len(disagreements)


def main() -> int:
    names = corpus_grammars()
    per_string = int(os.environ.get("HEGEL_LARK_MUTANTS", "4"))
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp)
        run_test(
            "lark::generate::tests::write_corpus",
            {
                "HEGEL_LARK_CORPUS_DIR": str(work),
                "HEGEL_LARK_CORPUS_GRAMMARS": ",".join(names),
            },
        )
        rng = random.Random(0)
        for name in names:
            strings = [
                json.loads(line)
                for line in (work / f"{name}.jsonl").read_text().split("\n")
                if line
            ]
            candidates = []
            for s in strings:
                candidates.append(s)
                candidates.extend(mutants(s, rng, per_string))
            (work / f"{name}.candidates.jsonl").write_text(
                "".join(json.dumps(c) + "\n" for c in candidates)
            )
        run_test(
            "lark::parse::tests::write_verdicts",
            {
                "HEGEL_LARK_VERDICT_DIR": str(work),
                "HEGEL_LARK_CORPUS_GRAMMARS": ",".join(names),
            },
        )
        compared = 0
        disagreements = 0
        for name in names:
            n, d = compare(name, work)
            compared += n
            disagreements += d
    if disagreements:
        print(f"\n{disagreements} of {compared} strings: the engine and Lark disagree")
        return 1
    print(f"\nThe engine and Lark agree on all {compared} strings")
    return 0


if __name__ == "__main__":
    sys.exit(main())
