"""Explicit test cases: a test body run with values given in advance."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from hegel.errors import InvalidArgument
from hegel.pretty import pretty_repr

if TYPE_CHECKING:
    from hegel.generators.base import Generator
    from hegel.test_case import TestCase

__all__ = ["ExplicitCase", "ExplicitTestCase"]


class ExplicitCase:
    """The values of one explicit test case, as declared with
    ``@hegel.explicit_test_case(...)``: positional values are consumed by
    draws in order, keyword values by the draw's name."""

    __slots__ = ("args", "kwargs")

    def __init__(self, args: tuple[Any, ...], kwargs: dict[str, Any]) -> None:
        self.args = args
        self.kwargs = kwargs

    def __repr__(self) -> str:
        parts = [repr(a) for a in self.args] + [f"{k}={v!r}" for k, v in self.kwargs.items()]
        return f"explicit_test_case({', '.join(parts)})"


class _NoHandle:
    """Stands in for the engine handle of an explicit test case, where
    there is none: any engine operation is an error."""

    def __getattr__(self, name: str) -> Any:
        raise InvalidArgument(f"{name} is not supported in explicit test cases")


NO_HANDLE: Any = _NoHandle()


class ExplicitTestCase:
    """Serves a test body's draws from an :class:`ExplicitCase`."""

    def __init__(self, case: ExplicitCase) -> None:
        self.case = case
        self.positional = list(case.args)
        self.named = dict(case.kwargs)
        self.consumed = 0
        self.used: set[str] = set()
        self.lines: list[str] = []

    def draw(self, tc: TestCase, generator: Generator[Any], name: str | None) -> Any:
        display = tc._allocate_display_name(name)
        key = None if display is None else display.removesuffix(" = ")
        if key is not None and key in self.named and key not in self.used:
            self.used.add(key)
            value = self.named[key]
        elif self.consumed < len(self.positional):
            value = self.positional[self.consumed]
            self.consumed += 1
        else:
            what = (
                f"draw {key!r}"
                if key is not None and not key.startswith("Draw ")
                else "an unnamed draw"
            )
            raise InvalidArgument(
                f"Explicit test case {self.case!r} has no value for {what}: pass it by "
                "name, or add a positional value"
            )
        if display is not None:
            self.lines.append(f"{display}{pretty_repr(value)}")
        return value

    def draw_silent(self, tc: TestCase, generator: Generator[Any]) -> Any:
        raise InvalidArgument("draw_silent is not supported in explicit test cases")

    def note(self, message: str) -> None:
        self.lines.extend(str(message).split("\n"))

    def reject(self) -> None:
        raise InvalidArgument("Explicit test case: assumption violated (tc.assume / tc.reject)")

    def check_consumed(self) -> None:
        unused = [k for k in self.named if k not in self.used]
        leftover = len(self.positional) - self.consumed
        if unused or leftover:
            parts = []
            if unused:
                parts.append("named values not drawn: " + ", ".join(repr(k) for k in unused))
            if leftover:
                parts.append(f"{leftover} positional value(s) not drawn")
            raise InvalidArgument(
                f"Explicit test case {self.case!r} was not fully used: " + "; ".join(parts)
            )
