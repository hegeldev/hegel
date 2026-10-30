#!/usr/bin/env python3
"""Print every line JaCoCo reported as not fully covered.

`mvn verify` fails the build when a module's instruction or branch coverage
is below 100%, but names only the module. This reads the XML reports it
wrote (`<module>/target/site/jacoco/jacoco.xml`) and lists the offending
lines, so a failed coverage gate in CI says what to cover.
"""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    found = False
    for report in sorted(ROOT.glob("*/target/site/jacoco/jacoco.xml")):
        module = report.relative_to(ROOT).parts[0]
        for package in ET.parse(report).getroot().iter("package"):
            for source in package.iter("sourcefile"):
                for line in source.iter("line"):
                    missed_instructions = int(line.get("mi", "0"))
                    missed_branches = int(line.get("mb", "0"))
                    if missed_instructions or missed_branches:
                        found = True
                        print(
                            f"{module}: {package.get('name')}/{source.get('name')}:{line.get('nr')}"
                            f" missed {missed_instructions} instructions, {missed_branches} branches"
                        )
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
