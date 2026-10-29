"""Infer the name of a draw from the assignment it appears in.

``x = tc.draw(...)`` reports as ``x = <value>``: the caller's source file is
parsed once and every line of a ``draw`` call that is the whole right-hand
side of a single-target assignment is mapped to that target's name.

As in the Rust library's ``#[hegel::test]``, a name is *repeatable* when its
draw can run more than once per test case: it is assigned inside a loop or
comprehension, by more than one draw in the same function, or inside a
function nested in a ``@hegel.test`` function (the ``step`` of a
``tc.repeat(step)``, say), which may be called any number of times. A
repeatable name is reported numbered from one (``x_1``, ``x_2``, ...); any
other name is reported as written.
"""

from __future__ import annotations

import ast
import linecache
import threading
from dataclasses import dataclass, field

__all__ = ["DrawName", "name_for_call"]


@dataclass(frozen=True)
class DrawName:
    """The name a draw call is reported under."""

    name: str
    repeatable: bool


_cache: dict[str, dict[int, DrawName]] = {}
_lock = threading.Lock()

_LOOPS = (
    ast.For,
    ast.AsyncFor,
    ast.While,
    ast.ListComp,
    ast.SetComp,
    ast.DictComp,
    ast.GeneratorExp,
)


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
        and node.func.attr == "draw"
    )


def _draw_assignment(node: ast.AST) -> tuple[str, ast.expr] | None:
    """The target name and draw call of a single-target assignment whose
    whole right-hand side is a ``draw`` call."""
    if isinstance(node, ast.Assign):
        if len(node.targets) != 1:
            return None
        target, value = node.targets[0], node.value
    elif isinstance(node, (ast.AnnAssign, ast.NamedExpr)) and node.value is not None:
        target, value = node.target, node.value
    else:
        return None
    if not _is_draw_call(value):
        return None
    name = _target_name(target)
    return None if name is None else (name, value)


def _is_hegel_test(node: ast.AST) -> bool:
    """Whether ``node`` is a function decorated with ``hegel.test`` (or
    ``test``), with or without arguments."""
    if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
        return False
    for decorator in node.decorator_list:
        if isinstance(decorator, ast.Call):
            decorator = decorator.func
        if _target_name(decorator) == "test":
            return True
    return False


@dataclass
class _Scope:
    nested: bool = False
    assignments: list[tuple[str, ast.expr, bool]] = field(default_factory=list)


class _Collector(ast.NodeVisitor):
    def __init__(self) -> None:
        self.scopes = [_Scope()]
        self.finished: list[_Scope] = []
        self.loop_depth = 0
        self.test_depth = 0

    def _function(self, node: ast.AST) -> None:
        outer_loop_depth = self.loop_depth
        nested = self.test_depth > 0
        in_test = nested or _is_hegel_test(node)
        self.scopes.append(_Scope(nested=nested))
        self.loop_depth = 0
        self.test_depth += in_test
        self.generic_visit(node)
        self.test_depth -= in_test
        self.finished.append(self.scopes.pop())
        self.loop_depth = outer_loop_depth

    visit_FunctionDef = visit_AsyncFunctionDef = visit_Lambda = _function

    def _loop(self, node: ast.AST) -> None:
        self.loop_depth += 1
        self.generic_visit(node)
        self.loop_depth -= 1

    visit_For = visit_AsyncFor = visit_While = _loop
    visit_ListComp = visit_SetComp = visit_DictComp = visit_GeneratorExp = _loop

    def generic_visit(self, node: ast.AST) -> None:
        found = _draw_assignment(node)
        if found is not None:
            name, value = found
            scope = self.scopes[-1]
            scope.assignments.append((name, value, scope.nested or self.loop_depth > 0))
        super().generic_visit(node)


def _index_file(filename: str) -> dict[int, DrawName]:
    names: dict[int, DrawName] = {}
    source = "".join(linecache.getlines(filename))
    if not source:
        return names
    try:
        tree = ast.parse(source, filename)
    except SyntaxError:
        return names
    collector = _Collector()
    collector.visit(tree)
    for scope in [*collector.finished, *collector.scopes]:
        counts: dict[str, int] = {}
        for name, _, _ in scope.assignments:
            counts[name] = counts.get(name, 0) + 1
        repeatable = {name for name, _, in_loop in scope.assignments if in_loop} | {
            name for name, count in counts.items() if count > 1
        }
        for name, value, _ in scope.assignments:
            draw_name = DrawName(name, name in repeatable)
            for line in range(value.lineno, (value.end_lineno or value.lineno) + 1):
                names.setdefault(line, draw_name)
    return names


def name_for_call(filename: str, lineno: int) -> DrawName | None:
    """The name of the draw call at ``filename:lineno``, if it is assigned
    to a single target."""
    with _lock:
        index = _cache.get(filename)
        if index is None:
            index = _index_file(filename)
            _cache[filename] = index
    return index.get(lineno)
