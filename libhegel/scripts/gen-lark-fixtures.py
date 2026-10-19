#!/usr/bin/env -S uv run --script --with lark==1.3.1
"""Generate fixtures for the engine's Lark grammar loader from Lark itself.

For every `*.lark` file in `libhegel/tests/fixtures/lark/`, writes a
`*.expected` file next to it describing what Lark compiles the grammar to:
the start rule(s) (from a `// start: a b` comment in the grammar, default
`start`), each terminal's regexp and priority, the ignored terminals, and
the set of rule names (templates instantiated, unreachable rules dropped).
Grammars whose name begins with `err_` are expected to be rejected, and
their `.expected` file holds Lark's error message instead. Regexps are
written as JSON strings; rules Lark synthesizes for `*`, `+` and `~` (named
`__...`) are left out, since the port keeps repetition as repetition.

Run with `just lark-fixtures` after changing a fixture grammar.
"""

import json
import pathlib
import re
import sys

from lark import Lark
from lark.lexer import Pattern
from lark.load_grammar import load_grammar


def _get_flags_sorted(self, value):
    for f in sorted(self.flags):
        value = "(?%s:%s)" % (f, value)
    return value


# Released Lark iterates the flag set in hash order, which varies between
# processes; the port (like Lark's master branch) sorts the flags, and the
# order carries no meaning, so make the fixtures deterministic.
Pattern._get_flags = _get_flags_sorted

ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures" / "lark"


def start_rules(text):
    m = re.search(r"^// start: (.*)$", text, re.M)
    return m.group(1).split() if m else ["start"]


def expected_for(path):
    text = path.read_text(encoding="utf8")
    starts = start_rules(text)
    if path.name.startswith("err_"):
        try:
            Lark(text, start=starts, source_path=str(path))
        except Exception as e:  # noqa: BLE001 - Lark raises several types, and crashes on some
            first_line = (str(e).splitlines() or [type(e).__name__])[0]
            return "error\n" + first_line + "\n"
        raise SystemExit(f"{path}: expected Lark to reject this grammar")
    # Load and compile the way Hypothesis's from_lark sees the grammar,
    # without building a lexer or parser (whose own checks, such as LALR
    # conflicts, do not concern generation).
    grammar, _used_files = load_grammar(text, str(path), [], False)
    terminals, rules, ignore = grammar.compile(starts, ())
    lines = ["start " + " ".join(starts)]
    for t in sorted(terminals, key=lambda t: t.name):
        regexp = json.dumps(t.pattern.to_regexp(), ensure_ascii=True)
        lines.append(f"terminal {t.name}\t{t.priority}\t{regexp}")
    for name in ignore:
        lines.append(f"ignore {name}")
    for name in sorted({r.origin.name for r in rules}):
        if not name.startswith("__"):
            lines.append(f"rule {name}")
    return "\n".join(lines) + "\n"


def main():
    paths = sorted(FIXTURES.glob("*.lark"))
    if not paths:
        raise SystemExit(f"no fixture grammars in {FIXTURES}")
    for path in paths:
        try:
            expected = expected_for(path)
        except Exception as e:
            raise SystemExit(f"{path}: {type(e).__name__}: {e}") from e
        path.with_suffix(".expected").write_text(expected, encoding="utf8")
        print(f"wrote {path.with_suffix('.expected').relative_to(ROOT)}", file=sys.stderr)


if __name__ == "__main__":
    main()
