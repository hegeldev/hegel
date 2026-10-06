from __future__ import annotations

import linecache
import textwrap
from pathlib import Path

from hegel import _draw_names
from hegel._draw_names import name_for_call


def _index(tmp_path: Path, source: str) -> str:
    path = tmp_path / "mod.py"
    path.write_text(textwrap.dedent(source))
    linecache.checkcache(str(path))
    _draw_names._cache.pop(str(path), None)
    return str(path)


def test_assignment_forms(tmp_path):
    path = _index(
        tmp_path,
        """
        def body(tc):
            x = tc.draw(g)
            y: int = tc.draw(g)
            self.attr = tc.draw(g)
            total += tc.draw(g)
            if (z := tc.draw(g)):
                pass
            a, b = tc.draw(g)
            a = b = tc.draw(g)
            multi = tc.draw(
                g,
            )
            unrelated = other(g)
            tc.draw(g)
            [w] = tc.draw(g)
        """,
    )
    assert name_for_call(path, 3) == "x"
    assert name_for_call(path, 4) == "y"
    assert name_for_call(path, 5) == "attr"
    assert name_for_call(path, 6) == "total"
    assert name_for_call(path, 7) == "z"
    assert name_for_call(path, 9) is None  # tuple target
    assert name_for_call(path, 10) is None  # two targets
    assert name_for_call(path, 11) == "multi"
    assert name_for_call(path, 12) == "multi"
    assert name_for_call(path, 13) == "multi"
    assert name_for_call(path, 14) is None
    assert name_for_call(path, 15) is None
    assert name_for_call(path, 16) is None
    # cached on second lookup
    assert name_for_call(path, 3) == "x"


def test_unparseable_or_missing_sources(tmp_path):
    path = _index(tmp_path, "def broken(:\n")
    assert name_for_call(path, 1) is None
    assert name_for_call(str(tmp_path / "missing.py"), 1) is None
    assert name_for_call("<string>", 1) is None
