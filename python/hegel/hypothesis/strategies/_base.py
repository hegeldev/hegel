"""The strategy type and shared validation helpers."""

from __future__ import annotations

import math
from collections.abc import Callable
from typing import Any, Generic, TypeVar

from hegel.generators.base import Generator, label_from_name
from hegel.generators.combinators import Nothing
from hegel.hypothesis.errors import InvalidArgument

__all__ = [
    "NOTHING",
    "LazyStrategy",
    "SearchStrategy",
    "check_strategy",
    "check_type",
    "check_valid_size",
    "defines_strategy",
    "unwrap_strategies",
]

Ex = TypeVar("Ex")
T = TypeVar("T")


class SearchStrategy(Generic[Ex]):
    """A Hypothesis strategy: a wrapper around the Hegel generator that
    draws its values, available as :attr:`generator`.

    A subclass either passes a generator to ``__init__`` or defines
    ``do_draw(self, data)``, drawing from other strategies with
    ``data.draw(...)``.
    """

    def __init__(self, generator: Generator[Any] | None = None) -> None:
        if generator is not None and not isinstance(generator, Generator):
            raise InvalidArgument(
                f"Expected a Generator but got {generator!r} (type={type(generator).__name__})"
            )
        self._generator: Generator[Any] | None = generator

    def _build(self) -> Generator[Any]:
        if type(self).do_draw is not SearchStrategy.do_draw:
            return _CustomStrategy(self)
        raise InvalidArgument(
            f"{type(self).__name__} neither wraps a generator nor defines do_draw"
        )

    @property
    def generator(self) -> Generator[Any]:
        """The Hegel generator this strategy draws from."""
        if self._generator is None:
            self._generator = self._build()
        return self._generator

    def do_draw(self, data: Any) -> Ex:
        raise NotImplementedError(f"{type(self).__name__} does not define do_draw")

    @property
    def label(self) -> int:
        return self.generator.label

    @property
    def is_empty(self) -> bool:
        return self.generator.is_empty

    def validate(self) -> None:
        """Validate the strategy's arguments (building it, if lazy)."""
        self.generator  # noqa: B018

    def map(self, pack: Callable[[Ex], T]) -> SearchStrategy[T]:
        return LazyStrategy(
            lambda: self.generator.map(pack),
            (),
            {},
            name=lambda: f"{self!r}.map({_describe(pack)})",
        )

    def flatmap(self, expand: Callable[[Ex], SearchStrategy[T]]) -> SearchStrategy[T]:
        def expand_to_generator(value: Ex) -> Generator[Any]:
            return unwrap_strategies(expand(value))

        return LazyStrategy(
            lambda: self.generator.flat_map(expand_to_generator),
            (),
            {},
            name=lambda: f"{self!r}.flatmap({_describe(expand)})",
        )

    def filter(self, condition: Callable[[Ex], Any]) -> SearchStrategy[Ex]:
        return LazyStrategy(
            lambda: self.generator.filter(condition),
            (),
            {},
            name=lambda: f"{self!r}.filter({_describe(condition)})",
        )

    def __or__(self, other: SearchStrategy[T]) -> SearchStrategy[Ex | T]:
        if not isinstance(other, SearchStrategy):
            raise ValueError(f"Cannot | a SearchStrategy with {other!r}")
        from hegel.hypothesis.strategies import OneOfStrategy, one_of

        # ``a | b | c`` is one three-way choice, as in Hypothesis.
        branches = [
            s
            for operand in (self, other)
            for s in (operand.strategies if isinstance(operand, OneOfStrategy) else (operand,))
        ]
        return one_of(*branches)

    def example(self) -> Ex:
        """The minimal value the strategy generates, for interactive use."""
        from hegel.runner import run, with_output_override
        from hegel.settings import HealthCheck, Phase, Settings, Verbosity

        generator = self.generator
        found: list[Any] = []

        def body(tc: Any) -> None:
            found.append(tc.draw_silent(generator))
            raise _ExampleFound()

        settings = Settings(
            database="",
            verbosity=Verbosity.quiet,
            phases=[Phase.generate, Phase.shrink],
            print_blob=False,
            suppress_health_check=HealthCheck.all(),
        )
        try:
            with with_output_override(lambda line: None):
                run(body, settings)
        except _ExampleFound:
            return found[-1]
        raise InvalidArgument(f"{self!r} produced no example")

    def __repr__(self) -> str:
        if self._generator is None:
            return f"{type(self).__name__}()"
        return repr(self._generator)


class _ExampleFound(Exception):
    pass


class _CustomStrategy(Generator[Any]):
    """The generator behind a strategy subclass that defines ``do_draw``."""

    def __init__(self, strategy: SearchStrategy[Any]) -> None:
        self.strategy = strategy
        self.label = label_from_name(f"hegel.hypothesis.{type(strategy).__qualname__}")

    def do_draw(self, tc: Any) -> Any:
        from hegel.hypothesis.strategies import DataObject

        return self.strategy.do_draw(DataObject(tc))

    def __repr__(self) -> str:
        return repr(self.strategy)


def unwrap_strategies(strategy: Any) -> Generator[Any]:
    """The Hegel generator behind ``strategy`` (a generator is returned as is)."""
    if isinstance(strategy, SearchStrategy):
        return strategy.generator
    if isinstance(strategy, Generator):
        return strategy
    raise InvalidArgument(
        f"Expected a SearchStrategy but got {strategy!r} (type={type(strategy).__name__})"
    )


def is_nothing(strategy: Any) -> bool:
    """Whether ``strategy`` is ``nothing()``, without building a lazy one."""
    return isinstance(strategy, SearchStrategy) and isinstance(strategy._generator, Nothing)


NOTHING: Any = SearchStrategy(Nothing())


def check_strategy(arg: object, name: str = "") -> None:
    if not isinstance(arg, SearchStrategy):
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
    assert typ is not SearchStrategy, "use check_strategy instead"
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


class LazyStrategy(SearchStrategy[Any]):
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
        super().__init__()
        self.function = function
        self.__args = args
        self.__kwargs = kwargs
        self.__name: str | Callable[[], str] = (
            name if name is not None else getattr(function, "__name__", repr(function))
        )

    def _build(self) -> Generator[Any]:
        built = self.function(*self.__args, **self.__kwargs)
        if not isinstance(built, (SearchStrategy, Generator)):
            label = self.__name() if callable(self.__name) else self.__name
            raise InvalidArgument(f"{label}() must return a SearchStrategy but returned {built!r}")
        return unwrap_strategies(built)

    @property
    def wrapped_strategy(self) -> Generator[Any]:
        return self.generator

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
    def accept(*args: Any, **kwargs: Any) -> SearchStrategy[Any]:
        signature.bind(*args, **kwargs)  # a TypeError for bad arguments, eagerly
        if try_non_lazy:
            try:
                built = function(*args, **kwargs)
            except Exception:  # noqa: BLE001 - reported lazily, as Hypothesis does
                pass
            else:
                return built if isinstance(built, SearchStrategy) else SearchStrategy(built)
        return LazyStrategy(function, args, kwargs)

    return accept
