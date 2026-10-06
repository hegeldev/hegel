"""The strategy type and shared validation helpers."""

from __future__ import annotations

from typing import Any

from hegel.generators.base import Generator
from hegel.generators.combinators import Nothing
from hegel.hypothesis.errors import InvalidArgument

__all__ = [
    "NOTHING",
    "InvalidStrategy",
    "SearchStrategy",
    "check_strategy",
    "check_type",
    "check_valid_size",
]

#: A Hypothesis strategy is a Hegel generator.
SearchStrategy = Generator

NOTHING: Any = Nothing()


def check_strategy(arg: object, name: str = "") -> None:
    if not isinstance(arg, Generator):
        prefix = f"{name}=" if name else ""
        hint = ""
        if callable(arg) and getattr(arg, "__module__", "").startswith("hegel.hypothesis"):
            hint = ", but did you mean to call it?"
        elif isinstance(arg, (list, tuple)):
            hint = ", but did you mean a strategy such as st.sampled_from?"
        raise InvalidArgument(
            f"Expected a SearchStrategy but got {prefix}{arg!r} (type={type(arg).__name__}){hint}"
        )


def check_type(typ: type | tuple[type, ...], arg: object, name: str = "") -> None:
    assert typ is not Generator, "use check_strategy instead"
    if not isinstance(arg, typ):
        if isinstance(typ, tuple):
            typ_string = "one of " + ", ".join(t.__name__ for t in typ)
        else:
            typ_string = typ.__name__
        prefix = f"{name}=" if name else ""
        raise InvalidArgument(
            f"Expected {typ_string} but got {prefix}{arg!r} (type={type(arg).__name__})"
        )


def check_valid_size(value: Any, name: str) -> None:
    if value is None:
        return
    if isinstance(value, bool) or not isinstance(value, int):
        if isinstance(value, float) and value == int(value):
            return
        raise InvalidArgument(f"{name}={value!r} must be an integer")
    if value < 0:
        raise InvalidArgument(f"{name}={value!r} must be at least 0")


def check_valid_interval(lower: Any, upper: Any, lower_name: str, upper_name: str) -> None:
    if lower is None or upper is None:
        return
    if upper < lower:
        raise InvalidArgument(f"Cannot have {upper_name}={upper!r} < {lower_name}={lower!r}")


def check_valid_sizes(min_size: Any, max_size: Any) -> None:
    check_valid_size(min_size, "min_size")
    check_valid_size(max_size, "max_size")
    check_valid_interval(min_size, max_size, "min_size", "max_size")


class InvalidStrategy(Generator[Any]):
    """A strategy whose definition was invalid: the error is raised when it
    is validated or drawn from, not when it is constructed (as Hypothesis
    defers argument validation)."""

    def __init__(self, message: str) -> None:
        self.message = message

    def validate(self) -> None:
        raise InvalidArgument(self.message)

    def do_draw(self, tc: Any) -> Any:
        raise InvalidArgument(self.message)

    def __repr__(self) -> str:
        return f"InvalidStrategy({self.message!r})"
