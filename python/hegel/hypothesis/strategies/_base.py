"""The strategy type and shared validation helpers."""

from __future__ import annotations

import math
from typing import Any

from hegel.generators.base import Generator
from hegel.generators.combinators import Nothing
from hegel.hypothesis.errors import InvalidArgument

__all__ = ["NOTHING", "SearchStrategy", "check_strategy", "check_type", "check_valid_size"]

#: A Hypothesis strategy is a Hegel generator.
SearchStrategy = Generator

NOTHING: Any = Nothing()


def check_strategy(arg: object, name: str = "") -> None:
    if not isinstance(arg, Generator):
        prefix = f"{name}=" if name else ""
        hint = ""
        if callable(arg) and getattr(arg, "__module__", "").startswith("hegel.hypothesis"):
            hint = ", but did you mean to call it?"
        raise InvalidArgument(
            f"Expected a SearchStrategy but got {prefix}{arg!r} (type={type(arg).__name__}){hint}"
        )


def check_type(typ: type | tuple[type, ...], arg: object, name: str = "") -> None:
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
        if isinstance(value, float) and math.isfinite(value) and value == int(value):
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


class LazyStrategy(Generator[Any]):
    """A strategy built from ``function(*args, **kwargs)`` on first use.

    Hypothesis validates strategy arguments when a strategy is first drawn
    from (or ``validate``d), not when it is constructed, and reprs a
    strategy as the call that made it; this wrapper gives Hegel's eager
    generators the same behaviour.
    """

    def __init__(
        self,
        function: Any,
        args: tuple[Any, ...],
        kwargs: dict[str, Any],
        *,
        name: str | None = None,
    ) -> None:
        self.function = function
        self.__args = args
        self.__kwargs = kwargs
        self.__name = name or getattr(function, "__name__", repr(function))
        self.__wrapped: Generator[Any] | None = None

    @property
    def wrapped_strategy(self) -> Generator[Any]:
        if self.__wrapped is None:
            built = self.function(*self.__args, **self.__kwargs)
            while isinstance(built, LazyStrategy):
                built = built.wrapped_strategy
            if not isinstance(built, Generator):
                raise InvalidArgument(
                    f"{self.__name}() must return a SearchStrategy but returned {built!r}"
                )
            self.__wrapped = built
        return self.__wrapped

    @property
    def label(self) -> int:  # type: ignore[override]
        return self.wrapped_strategy.label

    def validate(self) -> None:
        self.wrapped_strategy.validate()

    @property
    def is_empty(self) -> bool:
        return self.wrapped_strategy.is_empty

    def do_draw(self, tc: Any) -> Any:
        return self.wrapped_strategy.do_draw(tc)

    def do_draw_and_print(self, tc: Any, printer: Any) -> Any:
        return self.wrapped_strategy.do_draw_and_print(tc, printer)

    def map(self, f: Any) -> Generator[Any]:
        return LazyStrategy(
            lambda: self.wrapped_strategy.map(f), (), {}, name=f"{self!r}.map({_describe(f)})"
        )

    def flat_map(self, f: Any) -> Generator[Any]:
        return LazyStrategy(
            lambda: self.wrapped_strategy.flat_map(f),
            (),
            {},
            name=f"{self!r}.flatmap({_describe(f)})",
        )

    flatmap = flat_map

    def filter(self, f: Any) -> Generator[Any]:
        return LazyStrategy(
            lambda: self.wrapped_strategy.filter(f),
            (),
            {},
            name=f"{self!r}.filter({_describe(f)})",
        )

    def __repr__(self) -> str:
        if not self.__args and not self.__kwargs:
            if self.__name.endswith(")"):
                return self.__name
            return f"{self.__name}()"
        try:
            import inspect

            signature = inspect.signature(self.function)
            bound = signature.bind(*self.__args, **self.__kwargs)
            parts = []
            for name, value in bound.arguments.items():
                param = signature.parameters[name]
                if param.kind is param.VAR_POSITIONAL:
                    parts.extend(repr(v) for v in value)
                elif param.kind is param.VAR_KEYWORD:
                    parts.extend(f"{k}={v!r}" for k, v in value.items())
                elif param.kind is param.POSITIONAL_ONLY or (
                    param.kind is param.POSITIONAL_OR_KEYWORD
                    and name in [p for p in signature.parameters][: len(self.__args)]
                ):
                    parts.append(repr(value))
                elif (
                    param.default is param.empty
                    or value is not param.default
                    and value != param.default
                ):
                    parts.append(f"{name}={value!r}")
        except (TypeError, ValueError):
            parts = [repr(a) for a in self.__args] + [
                f"{k}={v!r}" for k, v in self.__kwargs.items()
            ]
        return f"{self.__name}({', '.join(parts)})"


def _describe(f: Any) -> str:
    name = getattr(f, "__name__", None)
    if name is None or name == "<lambda>":
        try:
            import inspect

            source = inspect.getsource(f).strip()
            if source.count("lambda") == 1:
                return _trim_lambda(source[source.index("lambda") :])
        except (OSError, TypeError):
            pass
        return repr(f) if name is None else name
    return name


def _trim_lambda(source: str) -> str:
    """The lambda expression at the start of ``source``, cut at the first
    unbalanced closing bracket or top-level comma."""
    depth = 0
    for index, char in enumerate(source):
        if char in "([{":
            depth += 1
        elif char in ")]}":
            if depth == 0:
                return source[:index].strip()
            depth -= 1
        elif char == "," and depth == 0:
            return source[:index].strip()
    return source.strip()


def defines_strategy(function: Any) -> Any:
    """Make a strategy-returning function lazy: calling it returns a
    :class:`LazyStrategy` that builds the real strategy on first draw."""
    import functools
    import inspect

    signature = inspect.signature(function)

    @functools.wraps(function)
    def accept(*args: Any, **kwargs: Any) -> LazyStrategy:
        signature.bind(*args, **kwargs)  # a TypeError for bad arguments, eagerly
        return LazyStrategy(function, args, kwargs)

    return accept


def unwrap_strategies(s: Any) -> Any:
    """The eager generator behind a lazy strategy (or ``s`` itself)."""
    while isinstance(s, LazyStrategy):
        s = s.wrapped_strategy
    return s
