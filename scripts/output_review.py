#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Review before/after captures of what each frontend prints, in the browser.

Serves a matrix of every capture, side-by-side before/after diffs, an
all-frontends view per scenario and a review mode that walks through every
capture whose before and after differ, with ok / problem / question verdicts.

Usage:

    uv run --script scripts/output_review.py DATA_DIR [--port 8000]
        [--reference rust] [--spec FILE]
        [--normalize PATH=TOKEN ...] [--nondeterministic SCENARIO ...]

DATA_DIR is laid out as

    DATA_DIR/<frontend>/<scenario>/<verbosity>/<side>/{stdout.txt,stderr.txt,meta.json}

with <side> one of `before` and `after`, `meta.json` holding at least
`exit_code`, and optionally `DATA_DIR/<frontend>/<scenario>/UNSUPPORTED` (the
reason the frontend cannot run the scenario) and `DATA_DIR/<frontend>/HARNESS.md`.
Frontends, scenarios and verbosities are discovered from the directory.

Before comparing, both sides are normalized: ANSI codes, durations, addresses,
thread ids, timestamps, /tmp paths, cargo `deps/` hashes, source line numbers, Alcotest run
ids and the words `before`/`after` used as path or name components are replaced
by placeholders, as is every `--normalize` path. Statuses are computed at
startup and recomputed on /refresh.

Each saved verdict is printed to stdout as a `FEEDBACK {json}` line and kept in
memory until the server stops. Runs of unchanged lines in a diff are folded;
click one to unfold it.
In review mode, o / p / q save a verdict and move on, s skips to the next diff
without saving, j / k go to the next and previous diff and f focuses the note.
"""

import argparse
import datetime
import difflib
import html
import json
import re
import threading
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

SIDES = ["before", "after"]
STREAMS = ["stdout", "stderr"]
KNOWN_VERBOSITIES = ["quiet", "normal", "verbose", "debug"]
DIFF_LIMIT = 4000
CROSS_LIMIT = 1500
REVIEWABLE = ("changed", "reordered", "moved", "partial")
VERDICTS = ["", "ok", "problem", "question"]

GENERIC_NORMALIZERS = [
    (re.compile(r"/tmp/[^\s:'\")]+"), "<TMP>"),
    (re.compile(r"([-_.])(before|after)(?=[/._\s]|$)"), r"\1<SIDE>"),
    (re.compile(r"/(before|after)/"), "/<SIDE>/"),
    (re.compile(r"(deps/[A-Za-z0-9_]+)-[0-9a-f]{16}"), r"\1-<HASH>"),
    (re.compile(r"This run has ID `[A-Z0-9]+'"), "This run has ID `<RUNID>'"),
    (
        re.compile(r'(file "[^"]+"), line \d+, characters \d+-\d+'),
        r"\1, line <L>, characters <C>",
    ),
    (
        re.compile(
            r"(\b[A-Za-z_][\w]*\.(?:java|cs|ts|js|go|ml|mli|py|rs|cpp|cc|h|hpp)):(line )?\d+(:\d+)?"
        ),
        r"\1:\2<L>",
    ),
    (re.compile(r"\+\d+(\.\d+)?ms\]"), "+<T>ms]"),
    (re.compile(r"\b\d+(\.\d+)?\s?(ms|s|sec|seconds)\b"), "<DUR>"),
    (re.compile(r"\b0x[0-9a-fA-F]{4,}\b"), "<ADDR>"),
    (re.compile(r"(thread '[^']*' )\(\d+\)"), r"\1(<TID>)"),
    (re.compile(r"\b\d{2}:\d{2}:\d{2}(\.\d+)?\b"), "<TIME>"),
    (re.compile(r"\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}\S*"), "<DATETIME>"),
]


@dataclass
class Config:
    data: Path
    reference: str
    spec: Path | None
    nondeterministic: set[str]
    normalizers: list = field(default_factory=list)
    frontends: list[str] = field(default_factory=list)
    scenarios: list[str] = field(default_factory=list)
    verbosities: list[str] = field(default_factory=list)


CFG: Config
STATUS: dict = {}
BLOCKS: dict = {}
FEEDBACK_LOCK = threading.Lock()
FEEDBACK: dict = {}
FOLD_CONTEXT = 3


def path_normalizers(specs):
    """The rules for `--normalize PATH=TOKEN`: PATH itself, and `../`-relative paths to it."""
    rules = []
    for spec in sorted(specs, key=lambda s: -len(s.partition("=")[0])):
        path, _, token = spec.partition("=")
        path = path.rstrip("/")
        end = r"(?![\w.-])"
        rules.append((re.compile(re.escape(path) + end), token))
        name = Path(path).name
        if name:
            rules.append((re.compile(r"(?:\.\./)+" + re.escape(name) + end), token))
    return rules


def repo_line_normalizer(tokens):
    """Line numbers in frontend or hegel package paths under a token or site-packages."""
    roots = "|".join([*(re.escape(t) for t in tokens), "site-packages"])
    return (
        re.compile(
            rf"((?:{roots})/[^\s:\"']*?(?:frontends/|hegel/)[^\s:\"']+)(:|\", line )\d+(:\d+)?"
        ),
        r"\1\2<L>",
    )


def normalize(text):
    for pattern, repl in CFG.normalizers:
        text = pattern.sub(repl, text)
    return text


def subdirs(path):
    return [p.name for p in path.iterdir() if p.is_dir()] if path.is_dir() else []


def discover():
    """Read the frontends, scenarios and verbosities present under the data directory."""
    frontends = sorted(
        fe for fe in subdirs(CFG.data) if not fe.startswith(".") and subdirs(CFG.data / fe)
    )
    if CFG.reference in frontends:
        frontends.remove(CFG.reference)
        frontends.insert(0, CFG.reference)
    scenarios, verbosities = set(), set()
    for fe in frontends:
        for sc in subdirs(CFG.data / fe):
            scenarios.add(sc)
            verbosities.update(subdirs(CFG.data / fe / sc))
    known = [v for v in KNOWN_VERBOSITIES if v in verbosities]
    CFG.frontends = frontends
    CFG.scenarios = sorted(scenarios)
    CFG.verbosities = known + sorted(verbosities - set(known))


def capture_dir(fe, sc, vb, side):
    return CFG.data / fe / sc / vb / side


def unsupported(fe, sc):
    return (CFG.data / fe / sc / "UNSUPPORTED").exists()


def read_capture(fe, sc, vb, side, norm=True):
    d = capture_dir(fe, sc, vb, side)
    if not d.is_dir():
        return None
    out = {}
    for stream in STREAMS:
        p = d / f"{stream}.txt"
        text = p.read_text(errors="replace") if p.exists() else ""
        out[stream] = normalize(text) if norm else text
    meta_path = d / "meta.json"
    try:
        out["meta"] = json.loads(meta_path.read_text()) if meta_path.exists() else {}
    except json.JSONDecodeError as e:
        out["meta"] = {"error": f"bad meta.json: {e}"}
    return out


def compute_statuses():
    """Compare every capture once; pages read the result from STATUS."""
    discover()
    STATUS.clear()
    for fe in CFG.frontends:
        for sc in CFG.scenarios:
            for vb in CFG.verbosities:
                STATUS[(fe, sc, vb)] = compute_status(fe, sc, vb)


def status(fe, sc, vb):
    return STATUS.get((fe, sc, vb), "missing")


def compute_status(fe, sc, vb):
    if unsupported(fe, sc):
        return "unsupported"
    b = read_capture(fe, sc, vb, "before")
    a = read_capture(fe, sc, vb, "after")
    if b is None and a is None:
        return "missing"
    if b is None or a is None:
        return "partial"
    same_exit = b["meta"].get("exit_code") == a["meta"].get("exit_code")
    if all(b[s] == a[s] for s in STREAMS) and same_exit:
        return "same"
    if b["stdout"] + b["stderr"] == a["stdout"] + a["stderr"]:
        return "moved"
    if all(sorted(b[s].splitlines()) == sorted(a[s].splitlines()) for s in STREAMS):
        return "reordered"
    return "changed"


CSS = """
:root { --bg:#fff; --fg:#1d1d1f; --muted:#6b6b70; --line:#e3e3e6; --card:#f7f7f8;
  --add:#e6f4ea; --addfg:#1a7f37; --del:#fdecea; --delfg:#b42318; --chg:#fff4d6;
  --accent:#2f5fd0; --same:#d9f2e0; --changed:#fde3c7; --moved:#e3e1fb; --missing:#eee;
  --unsup:#f0f0f0; }
@media (prefers-color-scheme: dark) { :root { --bg:#141416; --fg:#e8e8ea; --muted:#9a9aa2;
  --line:#2c2c31; --card:#1c1c20; --add:#14301f; --addfg:#7ee2a0; --del:#3a1a18;
  --delfg:#ff9b8f; --chg:#3a3216; --accent:#8fb0ff; --same:#173323; --changed:#43301a;
  --moved:#2b2945; --missing:#222; --unsup:#1f1f22; } }
a, a:visited { color:var(--accent); text-decoration:none } a:hover { text-decoration:underline }
* { box-sizing:border-box } body { margin:0; background:var(--bg); color:var(--fg);
  font:14px/1.45 system-ui,-apple-system,Segoe UI,sans-serif }
header { padding:14px 20px; border-bottom:1px solid var(--line); display:flex; gap:18px;
  align-items:baseline; flex-wrap:wrap }
header a { color:var(--accent); text-decoration:none } header h1 { font-size:17px; margin:0 }
main { padding:16px 20px; max-width:1800px }
table.matrix { border-collapse:collapse; font-size:13px }
.matrix th, .matrix td { border:1px solid var(--line); padding:4px 7px; text-align:center }
.matrix th { background:var(--card); font-weight:600 }
.matrix td a { color:inherit; text-decoration:none; display:block }
.st-same{background:var(--same)} .st-changed{background:var(--changed)}
.st-moved{background:var(--moved)} .st-reordered{background:var(--moved)}
.st-missing,.st-partial{background:var(--missing);color:var(--muted)}
.st-unsupported{background:var(--unsup);color:var(--muted)}
.legend span { display:inline-block; padding:2px 8px; margin-right:6px; border-radius:4px;
  font-size:12px }
pre, .code { font:12px/1.4 ui-monospace,SFMono-Regular,Menlo,monospace; white-space:pre-wrap;
  word-break:break-word; margin:0 }
table.diff { border-collapse:collapse; width:100%; table-layout:fixed }
.diff td { vertical-align:top; padding:1px 6px; border-bottom:1px solid var(--line) }
.diff col.lncol { width:4.5ch }
.diff td.ln { width:4.5ch; color:var(--muted); text-align:right; font:11px ui-monospace,monospace;
  user-select:none }
.diff .add { background:var(--add) } .diff .del { background:var(--del) }
.diff .chg { background:var(--chg) }
.diff tbody.folded { display:none }
.diff tr.foldbar td { cursor:pointer; color:var(--muted); background:var(--card);
  font:12px ui-monospace,monospace; padding:3px 8px; text-align:center }
.diff tr.foldbar:hover td { color:var(--accent) }
.diff th { text-align:left; padding:4px 6px; background:var(--card); font-weight:600 }
h2 { font-size:15px; margin:22px 0 8px }
h3 { font-size:13px; margin:14px 0 6px; color:var(--muted) }
.meta { color:var(--muted); font-size:12px }
.pill { padding:1px 7px; border-radius:10px; font-size:12px; background:var(--card);
  border:1px solid var(--line) }
.grid { display:grid; gap:12px; grid-template-columns:repeat(auto-fill,minmax(420px,1fr)) }
.card { border:1px solid var(--line); border-radius:6px; background:var(--card); padding:8px 10px;
  overflow:auto }
.card h4 { margin:0 0 6px; font-size:13px } .stream { color:var(--muted); font-size:11px;
  margin:6px 0 2px }
section.row { border-top:3px solid var(--line); margin-top:18px; padding-top:4px }
section.row h2 a { color:inherit }
.pill.st-same, .pill.st-changed, .pill.st-moved, .pill.st-reordered, .pill.st-missing,
.pill.st-partial, .pill.st-unsupported { border:none }
.streamblock { border-left:5px solid var(--sc); border-radius:4px; margin:12px 0;
  padding:2px 0 2px 8px; background:color-mix(in srgb, var(--sc) 5%, transparent) }
.streamblock.stdout { --sc:#2f6fd0 } .streamblock.stderr { --sc:#d0622f }
.badge { display:inline-block; font:600 11px ui-monospace,monospace; letter-spacing:.06em;
  color:#fff; background:var(--sc); padding:2px 8px; border-radius:3px; margin:4px 0 6px }
.ref { border:2px dashed var(--line); border-radius:6px; padding:4px 10px 8px; margin-top:16px;
  background:var(--card) }
.ref > h2 { margin-top:6px }
.ref .reftitle { color:var(--muted); font-size:12px; text-transform:uppercase;
  letter-spacing:.08em }
.review-bar { position:sticky; top:0; z-index:5; background:var(--bg);
  border-bottom:1px solid var(--line); padding:8px 0; display:flex; gap:14px;
  align-items:center; flex-wrap:wrap }
.review-bar a.btn, .review-bar button { font:inherit; padding:4px 12px;
  border:1px solid var(--line); border-radius:5px; background:var(--card); color:var(--fg);
  text-decoration:none; cursor:pointer }
form.fb { border:1px solid var(--line); border-radius:6px; padding:10px; margin:12px 0;
  background:var(--card) }
form.fb textarea { width:100%; min-height:90px; font:13px/1.4 ui-monospace,monospace;
  background:var(--bg); color:var(--fg); border:1px solid var(--line); border-radius:4px;
  padding:6px }
form.fb label { margin-right:14px }
.verdict-ok{color:#1a7f37} .verdict-problem{color:#b42318} .verdict-question{color:#9a6700}
.saved { color:#1a7f37; font-size:12px }
.vbtns { margin-top:8px; display:flex; gap:10px }
.vbtn { font:600 14px system-ui,sans-serif; padding:7px 18px; border-radius:6px; cursor:pointer;
  border:2px solid currentColor; background:var(--bg) }
.vbtn.verdict-ok { color:#1a7f37 } .vbtn.verdict-problem { color:#b42318 }
.vbtn.verdict-question { color:#9a6700 }
a.vbtn.verdict-skip { color:var(--muted); text-decoration:none; margin-left:auto }
.vbtn.on { background:currentColor }
.vbtn.on.verdict-ok, .vbtn.on.verdict-problem, .vbtn.on.verdict-question { color:#fff }
.vbtn.on.verdict-ok { background:#1a7f37 } .vbtn.on.verdict-problem { background:#b42318 }
.vbtn.on.verdict-question { background:#9a6700 }
nav.tabs a { margin-right:10px; color:var(--accent); text-decoration:none }
nav.tabs a.on { font-weight:700; text-decoration:underline }
"""


def page(title, body):
    spec = "<a href='/spec'>Spec</a>" if CFG.spec else ""
    return (
        "<!doctype html><html><head><meta charset=utf-8>"
        "<meta name=viewport content='width=device-width,initial-scale=1'>"
        f"<title>{html.escape(title)}</title><style>{CSS}</style></head><body>"
        "<header><h1>Hegel output review</h1><a href='/'>Matrix</a>"
        "<a href='/cross'>All frontends</a><a href='/review'>Review</a>"
        f"<a href='/feedback'>Feedback</a>{spec}<span class=meta>{html.escape(title)}</span>"
        f"</header><main>{body}</main></body></html>"
    )


def qs_link(path, **params):
    parts = "&".join(f"{k}={v}" for k, v in params.items() if v is not None)
    return f"{path}?{parts}" if parts else path


def matrix():
    feedback = load_feedback()
    scenarios, verbosities = CFG.scenarios, CFG.verbosities
    head = (
        "<tr><th rowspan=2>frontend</th>"
        + "".join(f"<th colspan={len(verbosities)}>{sc}</th>" for sc in scenarios)
        + "</tr><tr>"
        + "".join(f"<th>{vb[0].upper()}</th>" for _ in scenarios for vb in verbosities)
        + "</tr>"
    )
    marks = {"ok": "✓", "problem": "✗", "question": "?"}
    rows = []
    for fe in CFG.frontends:
        cells = [f"<th><a href='/harness?fe={fe}'>{fe}</a></th>"]
        for sc in scenarios:
            for vb in verbosities:
                st = status(fe, sc, vb)
                link = qs_link("/diff", fe=fe, sc=sc, vb=vb)
                nd = sc in CFG.nondeterministic and st in ("changed", "moved", "reordered")
                fb = feedback.get(fb_key(fe, sc, vb))
                mark = marks.get(fb.get("verdict"), "•") if fb else ""
                cells.append(
                    f"<td class='st-{st}{' nd' if nd else ''}' title='{fe} {sc} {vb}: {st}'>"
                    f"<a href='{link}'>{st[0].upper()}{'*' if nd else ''}{mark}</a></td>"
                )
        rows.append("<tr>" + "".join(cells) + "</tr>")
    columns = ", ".join(f"{vb[0].upper()} {vb}" for vb in verbosities)
    nd_note = (
        "* nondeterministic scenario: run-to-run noise on both sides &nbsp; "
        if CFG.nondeterministic
        else ""
    )
    legend = (
        "<p class=legend><span class=st-same>S same</span>"
        "<span class=st-changed>C changed</span>"
        "<span class=st-moved>M same text, different stream</span>"
        "<span class=st-reordered>R same lines, different order</span>"
        "<span class=st-partial>P/M partial or missing</span>"
        f"<span class=st-unsupported>U unsupported</span> {nd_note}columns: {columns}. "
        "Comparison is after normalization (paths, timings, addresses, thread ids). "
        "Computed at startup; <a href='/refresh'>recompute</a> after captures change. "
        "Feedback marks: ✓ ok, ✗ problem, ? question, • note only.</p>"
    )
    return legend + "<table class=matrix>" + head + "".join(rows) + "</table>"


def diff_table(a_text, b_text, a_label, b_label, limit=None):
    limit = limit or DIFF_LIMIT
    a, b = a_text.splitlines(), b_text.splitlines()
    note = ""
    if len(a) > limit or len(b) > limit:
        note = (
            f"<p class=meta>Large output ({len(a)} vs {len(b)} lines): showing the first "
            f"{limit} lines of each side. Open the frontend's own page for more, or the "
            "files under the data directory.</p>"
        )
        a, b = a[:limit], b[:limit]
    sm = difflib.SequenceMatcher(None, a, b, autojunk=len(a) + len(b) > 2 * limit)
    rows = [
        f"<tr><th colspan=2>{html.escape(a_label)} ({len(a_text.splitlines())} lines)</th>"
        f"<th colspan=2>{html.escape(b_label)} ({len(b_text.splitlines())} lines)</th></tr>"
    ]

    def cell(n, s, cls):
        return (
            f"<td class='ln {cls}'>{n if n is not None else ''}</td>"
            f"<td class='{cls}'><pre>{html.escape(s) if s is not None else ''}</pre></td>"
        )

    def same(k0, k1, i1, j1):
        return [
            "<tr>" + cell(i1 + k + 1, a[i1 + k], "") + cell(j1 + k + 1, b[j1 + k], "") + "</tr>"
            for k in range(k0, k1)
        ]

    opcodes = sm.get_opcodes()
    for n, (op, i1, i2, j1, j2) in enumerate(opcodes):
        if op == "equal":
            size = i2 - i1
            head = FOLD_CONTEXT if n > 0 else 0
            tail = FOLD_CONTEXT if n < len(opcodes) - 1 else 0
            if size - head - tail < 2:
                rows.extend(same(0, size, i1, j1))
                continue
            hidden = size - head - tail
            rows.extend(same(0, head, i1, j1))
            rows.append(
                '</tbody><tbody><tr class=foldbar onclick="this.parentNode.nextElementSibling'
                ".classList.toggle('folded')\"><td colspan=4>⋯ "
                f"{hidden} unchanged lines (click to toggle)</td></tr></tbody>"
                "<tbody class=folded>"
            )
            rows.extend(same(head, head + hidden, i1, j1))
            rows.append("</tbody><tbody>")
            rows.extend(same(head + hidden, size, i1, j1))
        else:
            for k in range(max(i2 - i1, j2 - j1)):
                ai, bj = i1 + k, j1 + k
                left = (
                    cell(ai + 1, a[ai], "del" if op != "insert" else "")
                    if ai < i2
                    else cell(None, None, "")
                )
                right = (
                    cell(bj + 1, b[bj], "add" if op != "delete" else "")
                    if bj < j2
                    else cell(None, None, "")
                )
                rows.append("<tr>" + left + right + "</tr>")
    if len(rows) == 1:
        rows.append(
            "<tr><td></td><td class=meta>(empty)</td><td></td><td class=meta>(empty)</td></tr>"
        )
    cols = "<colgroup><col class=lncol><col><col class=lncol><col></colgroup>"
    return note + "<table class=diff>" + cols + "<tbody>" + "".join(rows) + "</tbody></table>"


def tabs(path, current, options, **params):
    return " ".join(
        f"<a class='{'on' if o == current else ''}' href='{qs_link(path, **{**params, key: o})}'>"
        f"{o}</a>"
        for key, o in options
    )


def diff_block(fe, sc, vb, norm, limit=DIFF_LIMIT, heading="h2"):
    key = (fe, sc, vb, norm, limit, heading)
    if key not in BLOCKS:
        BLOCKS[key] = render_diff_block(fe, sc, vb, norm, limit, heading)
    return BLOCKS[key]


def prewarm_review():
    for fe, sc, vb in review_items():
        diff_block(fe, sc, vb, True)


def prewarm():
    for vb in CFG.verbosities:
        for sc in CFG.scenarios:
            for fe in CFG.frontends:
                diff_block(fe, sc, vb, True, CROSS_LIMIT)
                diff_block(fe, sc, vb, True)


def render_diff_block(fe, sc, vb, norm, limit, heading):
    """The before/after comparison of one capture: exit codes and a diff per stream."""
    link = qs_link("/diff", fe=fe, sc=sc, vb=vb, raw=None if norm else 1)
    st = status(fe, sc, vb)
    hint = (
        " <span class=meta>— every line is on both sides; only the order differs</span>"
        if st == "reordered"
        else ""
    )
    head = (
        f"<{heading} id='{fe}'><a href='{link}'>{fe}</a> · {sc} · {vb} "
        f"<span class='pill st-{st}'>{st}</span>{hint}</{heading}>"
    )
    if unsupported(fe, sc):
        reason = (CFG.data / fe / sc / "UNSUPPORTED").read_text(errors="replace")
        return head + f"<p>Unsupported: <span class=code>{html.escape(reason)}</span></p>"
    b = read_capture(fe, sc, vb, "before", norm)
    a = read_capture(fe, sc, vb, "after", norm)
    if b is None and a is None:
        return head + "<p class=meta>no capture</p>"
    empty = {"stdout": "", "stderr": "", "meta": {"missing": True}}
    b, a = b or empty, a or empty
    out = [head]
    exit_b, exit_a = b["meta"].get("exit_code", "?"), a["meta"].get("exit_code", "?")
    if exit_b != exit_a:
        out.append(f"<p><span class=pill>exit code {exit_b} → {exit_a}</span></p>")
    for stream in STREAMS:
        badge = f"<div class=badge>{stream.upper()}</div>"
        if not b[stream] and not a[stream]:
            out.append(
                f"<div class='streamblock {stream}'>{badge} "
                "<span class=meta>(empty on both sides)</span></div>"
            )
            continue
        table = diff_table(b[stream], a[stream], f"before {stream}", f"after {stream}", limit)
        if b[stream] == a[stream]:
            lines = len(b[stream].splitlines())
            table = (
                f"<details><summary class=meta>unchanged ({lines} lines)</summary>{table}</details>"
            )
        out.append(f"<div class='streamblock {stream}'>{badge}{table}</div>")
    return "".join(out)


def raw_toggle(path, norm, **params):
    if norm:
        return f"<a href='{qs_link(path, **params, raw=1)}'>show raw</a>"
    return f"<a href='{qs_link(path, **params)}'>show normalized</a>"


def diff_page(fe, sc, vb, norm):
    raw = None if norm else 1
    sep = " &nbsp;|&nbsp; "
    nav = (
        "<nav class=tabs>"
        + tabs("/diff", vb, [("vb", v) for v in CFG.verbosities], fe=fe, sc=sc, raw=raw)
        + sep
        + tabs("/diff", sc, [("sc", x) for x in CFG.scenarios], fe=fe, vb=vb, raw=raw)
        + sep
        + raw_toggle("/diff", norm, fe=fe, sc=sc, vb=vb)
        + sep
        + f"<a href='{qs_link('/cross', sc=sc, vb=vb, raw=raw)}'>all frontends</a></nav>"
    )
    return nav + diff_block(fe, sc, vb, norm)


def cross_page(sc, vb, norm, only_changed):
    if not CFG.scenarios or not CFG.verbosities:
        return "<p class=meta>no captures</p>"
    sc = sc or CFG.scenarios[0]
    vb = vb or ("normal" if "normal" in CFG.verbosities else CFG.verbosities[0])
    raw = None if norm else 1
    changed = 1 if only_changed else None
    sep = " &nbsp;|&nbsp; "
    toggle = (
        f"<a href='{qs_link('/cross', sc=sc, vb=vb, raw=raw)}'>show all frontends</a>"
        if only_changed
        else f"<a href='{qs_link('/cross', sc=sc, vb=vb, raw=raw, changed=1)}'>only changed</a>"
    )
    nav = (
        "<nav class=tabs>"
        + tabs("/cross", sc, [("sc", x) for x in CFG.scenarios], vb=vb, raw=raw, changed=changed)
        + sep
        + tabs("/cross", vb, [("vb", v) for v in CFG.verbosities], sc=sc, raw=raw, changed=changed)
        + sep
        + raw_toggle("/cross", norm, sc=sc, vb=vb, changed=changed)
        + sep
        + toggle
        + "</nav>"
    )
    shown = [fe for fe in CFG.frontends if not only_changed or status(fe, sc, vb) in REVIEWABLE]
    jump = (
        "<p class=meta>"
        + " · ".join(
            f"<a href='#{fe}'>{fe}</a> "
            f"<span class='pill st-{status(fe, sc, vb)}'>{status(fe, sc, vb)}</span>"
            for fe in shown
        )
        + "</p>"
    )
    rows = "".join(
        f"<section class=row>{diff_block(fe, sc, vb, norm, CROSS_LIMIT)}</section>" for fe in shown
    )
    body = rows or "<p class=meta>no frontend changed</p>"
    return f"<h2>{sc} · {vb} · all frontends</h2>" + nav + jump + body


def load_feedback():
    with FEEDBACK_LOCK:
        return dict(FEEDBACK)


def save_feedback(key, verdict, text):
    with FEEDBACK_LOCK:
        data = FEEDBACK
        if not verdict and not text.strip():
            data.pop(key, None)
        else:
            data[key] = {
                "verdict": verdict,
                "text": text,
                "updated": datetime.datetime.now().isoformat(timespec="seconds"),
            }
        event = {"key": key, "verdict": verdict, "text": text, "deleted": key not in data}
        print("FEEDBACK " + json.dumps(event), flush=True)


def fb_key(fe, sc, vb):
    return f"{fe}/{sc}/{vb}"


def review_items():
    """Every capture whose before and after differ, in review order."""
    return [
        (fe, sc, vb)
        for sc in CFG.scenarios
        for vb in CFG.verbosities
        for fe in CFG.frontends
        if status(fe, sc, vb) in REVIEWABLE
    ]


def reference_panel(sc, vb, norm):
    """The reference frontend's after output for the same case, as the point of comparison."""
    ref = CFG.reference
    if ref not in CFG.frontends or unsupported(ref, sc):
        return ""
    cap = read_capture(ref, sc, vb, "after", norm)
    if cap is None:
        return ""
    body = []
    for stream in STREAMS:
        lines = cap[stream].splitlines()
        shown = "\n".join(lines[:CROSS_LIMIT])
        more = (
            f"<p class=meta>… {len(lines) - CROSS_LIMIT} more lines</p>"
            if len(lines) > CROSS_LIMIT
            else ""
        )
        content = (
            f"<pre>{html.escape(shown)}</pre>{more}"
            if shown
            else ("<span class=meta>(empty)</span>")
        )
        body.append(
            f"<div class='streamblock {stream}'><div class=badge>{stream.upper()}</div>"
            f"{content}</div>"
        )
    link = qs_link("/diff", fe=ref, sc=sc, vb=vb)
    return (
        f"<div class=ref><div class=reftitle>reference: {ref} · after · {sc} · {vb} "
        f"(exit {cap['meta'].get('exit_code', '?')}) · "
        f"<a href='{link}'>{ref} before/after</a></div>{''.join(body)}</div>"
    )


def review_index(items, key, feedback):
    """Where to open the review: the diff named `key`, else the first one without feedback."""
    keys = [fb_key(*it) for it in items]
    if key in keys:
        return keys.index(key)
    return next((n for n, k in enumerate(keys) if k not in feedback), 0)


REVIEW_SCRIPT = """<script>
document.addEventListener('keydown', e => {
  const t = e.target.tagName;
  if (t === 'TEXTAREA' || t === 'INPUT') return;
  const verdict = {o: 'ok', p: 'problem', q: 'question'}[e.key];
  if (verdict && !e.ctrlKey && !e.metaKey && !e.altKey) {
    e.preventDefault();
    const f = document.querySelector('form.fb');
    f.requestSubmit(f.querySelector(`button[value=${verdict}]`));
    return; }
  if (e.key === 's') location.href = document.getElementById('skip').href;
  if (e.key === 'j') location.href = document.getElementById('next').href;
  if (e.key === 'k') location.href = document.getElementById('prev').href;
  if (e.key === 'f') { e.preventDefault(); document.getElementById('fbtext').focus(); }
});
</script>"""


def review_page(key, norm, saved):
    items = review_items()
    if not items:
        return "<p class=meta>nothing changed</p>"
    feedback = load_feedback()
    i = review_index(items, key, feedback)
    fe, sc, vb = items[i]
    current_key = fb_key(fe, sc, vb)
    raw = None if norm else 1
    entry = feedback.get(current_key, {})
    done = sum(1 for it in items if fb_key(*it) in feedback)
    next_key = fb_key(*items[min(i + 1, len(items) - 1)])
    prev_link = qs_link("/review", key=fb_key(*items[max(i - 1, 0)]), raw=raw)
    next_link = qs_link("/review", key=next_key, raw=raw)
    next_open = next(
        (k for k in range(i + 1, len(items)) if fb_key(*items[k]) not in feedback), None
    )
    open_link = (
        f"<a href='{qs_link('/review', key=fb_key(*items[next_open]), raw=raw)}'>"
        "next without feedback</a>"
        if next_open is not None
        else ""
    )
    bar = (
        f"<div class=review-bar><a class=btn id=prev href='{prev_link}'>← prev (k)</a>"
        f"<b>{i + 1} / {len(items)}</b>"
        f"<a class=btn id=next href='{next_link}'>next (j) →</a>{open_link}"
        f"<span class=meta>{done} with feedback · only diffs where before and after differ</span> "
        + raw_toggle("/review", norm, key=current_key)
        + ("<span class=saved>saved</span>" if saved else "")
        + "</div>"
    )
    current = entry.get("verdict", "")
    buttons = "".join(
        f"<button class='vbtn verdict-{v}{' on' if current == v else ''}' name=verdict "
        f"value='{v}' accesskey='{v[0]}' "
        f"title='{v}: save with the note and go to the next diff ({v[0]})'>{v}</button> "
        for v in ("ok", "problem", "question")
    )
    saved_note = (
        f" <span class=meta>· current: <span class='verdict-{html.escape(current)}'>"
        f"{html.escape(current)}</span> · saved {html.escape(entry['updated'])}</span>"
        if entry.get("updated")
        else ""
    )
    form = (
        "<form class=fb method=post action='/feedback'>"
        f"<input type=hidden name=key value='{html.escape(current_key)}'>"
        f"<input type=hidden name=next value='{html.escape(next_key)}'>"
        f"<input type=hidden name=raw value='{raw or ''}'><input type=hidden name=go value=next>"
        f"<div><b>Feedback on {html.escape(current_key)}</b>{saved_note}</div>"
        "<textarea name=text id=fbtext placeholder='Optional note. Then press ok / problem / "
        "question (or o / p / q when not typing).'>"
        f"{html.escape(entry.get('text', ''))}</textarea>"
        f"<div class=vbtns>{buttons}<a class='vbtn verdict-skip' id=skip href='{next_link}' "
        "title='go to the next diff without saving (s)'>skip</a></div></form>"
    )
    ref = reference_panel(sc, vb, norm) if fe != CFG.reference else ""
    return bar + form + diff_block(fe, sc, vb, norm) + ref + REVIEW_SCRIPT


def feedback_page():
    data = load_feedback()
    if not data:
        return "<p class=meta>no feedback yet</p>"
    rows = []
    for key, entry in data.items():
        verdict = entry.get("verdict") or ""
        rows.append(
            f"<div class=card><h4><a href='{qs_link('/review', key=key)}'>{html.escape(key)}</a> "
            f"<span class='verdict-{html.escape(verdict or 'none')}'>{html.escape(verdict)}</span> "
            f"<span class=meta>{html.escape(entry.get('updated', ''))}</span></h4>"
            f"<pre>{html.escape(entry.get('text', ''))}</pre></div>"
        )
    return "<p class=meta>Kept in memory until the server stops.</p>" + "".join(rows)


def text_page(path):
    if path is None or not path.exists():
        return "<p class=meta>not written yet</p>"
    return f"<pre>{html.escape(path.read_text(errors='replace'))}</pre>"


def refresh():
    compute_statuses()
    BLOCKS.clear()
    prewarm_review()
    threading.Thread(target=prewarm, daemon=True).start()


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        url = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(url.query).items()}
        norm = "raw" not in q
        try:
            if url.path == "/refresh":
                refresh()
                self.send_response(303)
                self.send_header("Location", "/")
                self.end_headers()
                return
            fe, sc, vb = q.get("fe"), q.get("sc"), q.get("vb")
            if url.path == "/":
                body = page("matrix", matrix())
            elif (
                url.path == "/diff"
                and fe in CFG.frontends
                and sc in CFG.scenarios
                and vb in CFG.verbosities
            ):
                body = page(f"{fe} {sc} {vb}", diff_page(fe, sc, vb, norm))
            elif url.path == "/cross":
                sc = sc if sc in CFG.scenarios else None
                vb = vb if vb in CFG.verbosities else None
                body = page("all frontends", cross_page(sc, vb, norm, "changed" in q))
            elif url.path == "/review":
                body = page("review", review_page(q.get("key"), norm, "saved" in q))
            elif url.path == "/feedback":
                body = page("feedback", feedback_page())
            elif url.path == "/spec" and CFG.spec:
                body = page("spec", text_page(CFG.spec))
            elif url.path == "/harness" and fe in CFG.frontends:
                harness = text_page(CFG.data / fe / "HARNESS.md")
                body = page(f"{fe} harness", f"<h2>{fe} harness</h2>" + harness)
            else:
                self.send_error(404)
                return
        except Exception as e:  # keep the viewer up while captures are half-written
            body = page("error", f"<pre>{html.escape(repr(e))}</pre>")
        data = body.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if urlparse(self.path).path != "/feedback":
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length).decode()
        form = {k: v[0] for k, v in parse_qs(body, keep_blank_values=True).items()}
        key = form.get("key", "")
        if key.count("/") != 2:
            self.send_error(400)
            return
        verdict = form.get("verdict", "")
        save_feedback(key, verdict if verdict in VERDICTS else "", form.get("text", ""))
        go_next = form.get("go") == "next"
        params = {
            "key": form.get("next") if go_next else key,
            "raw": form.get("raw") or None,
            "saved": None if go_next else 1,
        }
        self.send_response(303)
        self.send_header("Location", qs_link("/review", **params))
        self.end_headers()

    def log_message(self, *args):
        pass


def normalize_spec(value):
    path, sep, token = value.partition("=")
    if not sep or not path or not token:
        raise argparse.ArgumentTypeError(f"expected PATH=TOKEN, got {value!r}")
    return value


def parse_args():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("data", type=Path, metavar="DATA_DIR", help="the capture directory")
    parser.add_argument("--port", type=int, default=8000, help="port to serve on (default 8000)")
    parser.add_argument(
        "--reference",
        default="rust",
        help="frontend listed first and shown as the reference in review mode (default rust)",
    )
    parser.add_argument(
        "--spec",
        type=Path,
        metavar="FILE",
        help="capture spec shown at /spec (default DATA_DIR/SPEC.md or DATA_DIR/../SPEC.md, "
        "when present)",
    )
    parser.add_argument(
        "--normalize",
        action="append",
        default=[],
        type=normalize_spec,
        metavar="PATH=TOKEN",
        help="replace PATH, and ../-relative references ending in its last component, with "
        "TOKEN before comparing, e.g. --normalize /home/me/hegel-before=<ROOT> "
        "--normalize /home/me/hegel=<ROOT>; repeatable, longest PATH applied first",
    )
    parser.add_argument(
        "--nondeterministic",
        action="append",
        default=[],
        metavar="SCENARIO",
        help="mark SCENARIO's differences as run-to-run noise in the matrix; repeatable",
    )
    return parser.parse_args()


def main():
    global CFG
    args = parse_args()
    data = args.data.resolve()
    if not data.is_dir():
        raise SystemExit(f"{data} is not a directory")
    spec = args.spec
    if spec is None:
        spec = next((p for p in (data / "SPEC.md", data.parent / "SPEC.md") if p.exists()), None)
    CFG = Config(
        data=data,
        reference=args.reference,
        spec=spec,
        nondeterministic=set(args.nondeterministic),
    )
    ansi = [(re.compile(r"\x1b\[[0-9;]*[A-Za-z]"), "")]
    tokens = sorted({spec.partition("=")[2] for spec in args.normalize})
    CFG.normalizers = (
        ansi
        + path_normalizers(args.normalize)
        + GENERIC_NORMALIZERS[:7]
        + [repo_line_normalizer(tokens)]
        + GENERIC_NORMALIZERS[7:]
    )
    compute_statuses()
    prewarm_review()
    print(
        f"ready: {len(CFG.frontends)} frontends, {len(CFG.scenarios)} scenarios, "
        f"{len(CFG.verbosities)} verbosities, {len(review_items())} diffs to review; "
        f"serving on port {args.port}",
        flush=True,
    )
    threading.Thread(target=prewarm, daemon=True).start()
    ThreadingHTTPServer(("0.0.0.0", args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
