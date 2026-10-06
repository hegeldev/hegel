"""The strategy type and shared validation helpers."""

from __future__ import annotations

import math
from collections.abc import Callable
from typing import Any

from hegel.generators.base import Generator
from hegel.generators.combinators import Nothing
from hegel.hypothesis.errors import InvalidArgument

__all__ = [
    "NOTHING",
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
        name: str | Callable[[], str] | None = None,
    ) -> None:
        self.function = function
        self.__args = args
        self.__kwargs = kwargs
        self.__name: str | Callable[[], str] = (
            name if name is not None else getattr(function, "__name__", repr(function))
        )
        self.__wrapped: Generator[Any] | None = None

    def __or__(self, other: Any) -> Generator[Any]:
        if not isinstance(other, Generator):
            raise ValueError(f"Cannot | a SearchStrategy with {other!r}")
        return super().__or__(other)

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

    def map(self, pack: Any) -> Generator[Any]:
        return LazyStrategy(
            lambda: self.wrapped_strategy.map(pack),
            (),
            {},
            name=lambda: f"{self!r}.map({_describe(pack)})",
        )

    def flat_map(self, f: Any) -> Generator[Any]:
        return LazyStrategy(
            lambda: self.wrapped_strategy.flat_map(f),
            (),
            {},
            name=lambda: f"{self!r}.flatmap({_describe(f)})",
        )

    flatmap = flat_map

    def filter(self, f: Any) -> Generator[Any]:
        return LazyStrategy(
            lambda: self.wrapped_strategy.filter(f),
            (),
            {},
            name=lambda: f"{self!r}.filter({_describe(f)})",
        )

    def __repr__(self) -> str:
        label = self.__name() if callable(self.__name) else self.__name
        if not self.__args and not self.__kwargs:
            if label.endswith(")"):
                return label
            return f"{label}()"
        try:
            import inspect

            signature = inspect.signature(self.function)
            bound = signature.bind(*self.__args, **self.__kwargs)
            parts: list[str] = []
            for name, value in bound.arguments.items():
                param = signature.parameters[name]
                if param.kind is param.VAR_POSITIONAL:
                    parts.extend(_arg_repr(v) for v in value)
                elif param.kind is param.VAR_KEYWORD:
                    parts.extend(f"{k}={_arg_repr(v)}" for k, v in value.items())
                elif param.kind is param.POSITIONAL_ONLY or (
                    param.kind is param.POSITIONAL_OR_KEYWORD
                    and name in [p for p in signature.parameters][: len(self.__args)]
                ):
                    parts.append(_arg_repr(value))
                elif (
                    param.default is param.empty
                    or value is not param.default
                    and value != param.default
                ):
                    parts.append(f"{name}={_arg_repr(value)}")
        except (TypeError, ValueError):
            parts = [_arg_repr(a) for a in self.__args] + [
                f"{k}={_arg_repr(v)}" for k, v in self.__kwargs.items()
            ]
        return f"{label}({', '.join(parts)})"


def _arg_repr(value: Any) -> str:
    """``repr(value)``, except that functions show their name or source."""
    import inspect

    if inspect.isfunction(value) or inspect.isbuiltin(value) or inspect.ismethod(value):
        return _describe(value)
    return repr(value)


def _describe(f: Any) -> str:
    """A function's name, or a lambda's source (``lambda x: x + 1``) as
    Hypothesis prints it; never a repr with a memory address."""
    name = getattr(f, "__name__", None)
    if name is None:
        return repr(f)
    if name != "<lambda>":
        return name
    source = _lambda_source(f)
    if source is not None:
        return source
    try:
        import inspect

        params = ", ".join(str(p) for p in inspect.signature(f).parameters.values())
    except (TypeError, ValueError):
        params = ""
    return f"lambda {params}: <unknown>" if params else "lambda: <unknown>"


def _lambda_source(f: Any) -> str | None:
    import ast
    import inspect
    import textwrap

    try:
        source = textwrap.dedent(inspect.getsource(f))
        params = list(inspect.signature(f).parameters)
    except (OSError, TypeError, ValueError):
        return None
    for candidate in _lambda_nodes(source):
        if candidate.args.posonlyargs or candidate.args.kwonlyargs or candidate.args.vararg:
            names = [a.arg for a in candidate.args.posonlyargs + candidate.args.args]
            names += [candidate.args.vararg.arg] if candidate.args.vararg else []
            names += [a.arg for a in candidate.args.kwonlyargs]
            names += [candidate.args.kwarg.arg] if candidate.args.kwarg else []
        else:
            names = [a.arg for a in candidate.args.args]
            names += [candidate.args.kwarg.arg] if candidate.args.kwarg else []
        if names != params:
            continue
        segment = ast.get_source_segment(source, candidate)
        if segment is not None:
            return " ".join(segment.split())
    return None


def _lambda_nodes(source: str) -> list[Any]:
    """The lambda expressions in ``source``, which may be a fragment of a
    larger expression (``inspect.getsource`` returns whole lines)."""
    import ast

    for text in (source, source.strip().rstrip(","), "(" + source + ")"):
        try:
            tree: Any = ast.parse(text)
        except SyntaxError:
            continue
        nodes = [node for node in ast.walk(tree) if isinstance(node, ast.Lambda)]
        if nodes:
            return nodes
    # A fragment that does not parse: cut the text at each ``lambda`` and
    # take the longest prefix that parses as a lambda expression.
    nodes = []
    index = source.find("lambda")
    while index >= 0:
        rest = source[index:]
        for end in range(len(rest), 6, -1):
            try:
                tree = ast.parse(rest[:end], mode="eval")
            except SyntaxError:
                continue
            if isinstance(tree.body, ast.Lambda):
                nodes.append(tree.body)
                break
        index = source.find("lambda", index + 6)
    return nodes


def defines_strategy(function: Any = None, *, try_non_lazy: bool = False) -> Any:
    """Make a strategy-returning function lazy: calling it returns a
    :class:`LazyStrategy` that builds the real strategy on first draw.
    With ``try_non_lazy`` the strategy is built eagerly when that does not
    raise, so that e.g. ``sampled_from`` copies its elements at once."""
    import functools
    import inspect

    if function is None:
        return functools.partial(defines_strategy, try_non_lazy=try_non_lazy)

    signature = inspect.signature(function)

    @functools.wraps(function)
    def accept(*args: Any, **kwargs: Any) -> Generator[Any]:
        signature.bind(*args, **kwargs)  # a TypeError for bad arguments, eagerly
        if try_non_lazy:
            try:
                return function(*args, **kwargs)
            except Exception:  # noqa: BLE001 - reported lazily, as Hypothesis does
                pass
        return LazyStrategy(function, args, kwargs)

    return accept


def unwrap_strategies(s: Any) -> Any:
    """The eager generator behind a lazy strategy (or ``s`` itself)."""
    while isinstance(s, LazyStrategy):
        s = s.wrapped_strategy
    return s
