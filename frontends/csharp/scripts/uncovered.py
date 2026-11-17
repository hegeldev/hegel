#!/usr/bin/env python3
"""Print the lines of frontends/csharp/src that the last `just test` left uncovered."""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPORT = ROOT / "coverage" / "coverage.cobertura.xml"


def main() -> int:
    report = Path(sys.argv[1]) if len(sys.argv) > 1 else REPORT
    if not report.exists():
        print(f"no coverage report at {report}; run `just test` first", file=sys.stderr)
        return 2
    tree = ET.parse(report)
    uncovered: dict[str, list[int]] = {}
    for cls in tree.iter("class"):
        filename = cls.get("filename", "")
        for line in cls.iter("line"):
            if line.get("hits") == "0":
                uncovered.setdefault(filename, []).append(int(line.get("number", "0")))
    if not uncovered:
        print("every line is covered")
        return 0
    for filename in sorted(uncovered):
        lines = sorted(set(uncovered[filename]))
        ranges: list[str] = []
        start = prev = lines[0]
        for n in lines[1:]:
            if n == prev + 1:
                prev = n
                continue
            ranges.append(str(start) if start == prev else f"{start}-{prev}")
            start = prev = n
        ranges.append(str(start) if start == prev else f"{start}-{prev}")
        print(f"{filename}: {', '.join(ranges)}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
