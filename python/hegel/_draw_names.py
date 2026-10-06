"""Infer the name of a draw from the assignment it appears in.

``x = tc.draw(...)`` reports as ``x = <value>``: the caller's source file is
parsed once and every line of a ``draw`` call that is the whole right-hand
side of a single-target assignment is mapped to that target's name.
"""

from __future__ import annotations

import ast
import linecache
import threading

_cache: dict[str, dict[int, str]] = {}
_lock = threading.Lock()

_DRAW_METHODS = frozenset({"draw"})


def _target_name(node: ast.AST) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        return node.attr
    return None


def _is_draw_call(node: ast.AST) -> bool:
    return (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr in _DRAW_METHODS
    )


def _index_file(filename: str) -> dict[int, str]:
    names: dict[int, str] = {}
    source = "".join(linecache.getlines(filename))
    if not source:
        return names
    try:
        tree = ast.parse(source, filename)
    except SyntaxError:
        return names
    for node in ast.walk(tree):
        if isinstance(node, ast.Assign):
            targets = node.targets
            value = node.value
        elif isinstance(node, ast.AnnAssign) and node.value is not None:
            targets = [node.target]
            value = node.value
        elif isinstance(node, (ast.NamedExpr, ast.AugAssign)):
            targets = [node.target]
            value = node.value
        else:
            continue
        if not _is_draw_call(value):
            continue
        name = _target_name(targets[0]) if len(targets) == 1 else None
        if name is None:
            continue
        end = value.end_lineno or value.lineno
        for line in range(value.lineno, end + 1):
            names.setdefault(line, name)
    return names


def name_for_call(filename: str, lineno: int) -> str | None:
    """The assignment target of the draw call at ``filename:lineno``."""
    with _lock:
        index = _cache.get(filename)
        if index is None:
            index = _index_file(filename)
            _cache[filename] = index
    return index.get(lineno)
