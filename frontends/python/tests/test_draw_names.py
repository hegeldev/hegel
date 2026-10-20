from __future__ import annotations

import linecache
import textwrap
from pathlib import Path

from hegel import _draw_names
from hegel._draw_names import DrawName, name_for_call


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
    assert name_for_call(path, 3) == DrawName("x", False)
    assert name_for_call(path, 4) == DrawName("y", False)
    assert name_for_call(path, 5) == DrawName("attr", False)
    assert name_for_call(path, 6) is None  # augmented: the draw is not the total
    assert name_for_call(path, 7) == DrawName("z", False)
    assert name_for_call(path, 9) is None  # tuple target
    assert name_for_call(path, 10) is None  # two targets
    assert name_for_call(path, 11) == DrawName("multi", False)
    assert name_for_call(path, 12) == DrawName("multi", False)
    assert name_for_call(path, 13) == DrawName("multi", False)
    assert name_for_call(path, 14) is None
    assert name_for_call(path, 15) is None
    assert name_for_call(path, 16) is None
    # cached on second lookup
    assert name_for_call(path, 3) == DrawName("x", False)


def test_repeatable_names(tmp_path):
    path = _index(
        tmp_path,
        """
        def body(tc):
            once = tc.draw(g)
            twice = tc.draw(g)
            twice = tc.draw(g)
            for _ in range(3):
                looped = tc.draw(g)
            while True:
                if cond:
                    nested = tc.draw(g)
            values = [(item := tc.draw(g)) for _ in range(3)]

            def helper():
                once = tc.draw(g)

        async def other(tc):
            async for _ in xs:
                looped = tc.draw(g)
            after = tc.draw(g)
        """,
    )
    assert name_for_call(path, 3) == DrawName("once", False)
    assert name_for_call(path, 4) == DrawName("twice", True)
    assert name_for_call(path, 5) == DrawName("twice", True)
    assert name_for_call(path, 7) == DrawName("looped", True)
    assert name_for_call(path, 10) == DrawName("nested", True)
    assert name_for_call(path, 11) == DrawName("item", True)
    assert name_for_call(path, 14) == DrawName("once", False)  # its own scope
    assert name_for_call(path, 18) == DrawName("looped", True)
    assert name_for_call(path, 19) == DrawName("after", False)


def test_unparseable_or_missing_sources(tmp_path):
    path = _index(tmp_path, "def broken(:\n")
    assert name_for_call(path, 1) is None
    assert name_for_call(str(tmp_path / "missing.py"), 1) is None
    assert name_for_call("<string>", 1) is None
