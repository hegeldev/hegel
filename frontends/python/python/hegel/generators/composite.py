"""Generators defined by user functions: ``composite`` and ``builds``."""

from __future__ import annotations

import functools
import inspect
from collections.abc import Callable
from typing import TYPE_CHECKING, Any, TypeVar

from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, _PrintsValue
from hegel.generators.combinators import as_generator
from hegel.pretty import _SILENT_PRINTER, PrettyPrinter

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["builds", "composite"]

T = TypeVar("T")

COMPOSITE_LABEL = label_from_name("hegel.composite")
BUILDS_LABEL = label_from_name("hegel.builds")


class Composite(_PrintsValue[T]):
    def __init__(
        self, function: Callable[..., T], args: tuple[Any, ...], kwargs: dict[str, Any]
    ) -> None:
        self.function = function
        self.args = args
        self.kwargs = kwargs
        name = getattr(function, "__qualname__", repr(function))
        components = [a.label for a in args if isinstance(a, Generator)]
        components += [v.label for v in kwargs.values() if isinstance(v, Generator)]
        self.label = combine_labels([COMPOSITE_LABEL, label_from_name(name), *components])

    def do_draw(self, tc: TestCase) -> T:
        return self.function(tc, *self.args, **self.kwargs)

    def __repr__(self) -> str:
        parts = [repr(a) for a in self.args] + [f"{k}={v!r}" for k, v in self.kwargs.items()]
        return f"{getattr(self.function, '__name__', 'composite')}({', '.join(parts)})"


def composite(function: Callable[..., T]) -> Callable[..., Generator[T]]:
    """Define a generator as a function taking a :class:`~hegel.TestCase`
    first.

    Calling the decorated function with the remaining arguments returns a
    generator; each draw runs the function body with the test case and
    those arguments, and the generated value is what the body returns.
    The body draws with ``tc.draw`` like a test does, so later draws can
    depend on earlier ones. Its own draws are not printed separately: the
    failure report prints the composite's value.

    ::

        @hegel.composite
        def sorted_pairs(tc, limit):
            a = tc.draw(gs.integers(0, limit))
            b = tc.draw(gs.integers(a, limit))
            return (a, b)

        @hegel.test
        def test_pairs(tc):
            a, b = tc.draw(sorted_pairs(100))
            assert a <= b

    Decorating a class, or a function without a positional first
    parameter for the test case, is an :class:`~hegel.InvalidArgument`.
    ``hegel.composite`` and ``hegel.generators.composite`` are the same
    function.
    """
    if not callable(function):
        raise InvalidArgument("composite() takes a function")
    if inspect.isclass(function):
        raise InvalidArgument(
            f"composite() takes a function, not the class {function.__name__}. To generate "
            "instances, use builds()."
        )
    signature = inspect.signature(function)
    parameters = list(signature.parameters.values())
    if not parameters or "POSITIONAL" not in parameters[0].kind.name:
        raise InvalidArgument(
            f"{getattr(function, '__name__', function)} must take the test case "
            "as its first positional parameter"
        )

    @functools.wraps(function)
    def accept(*args: Any, **kwargs: Any) -> Generator[T]:
        return Composite(function, args, kwargs)

    accept.__signature__ = signature.replace(  # type: ignore[attr-defined]
        parameters=list(signature.parameters.values())[1:]
    )
    return accept


class Builds(Generator[T]):
    def __init__(
        self, target: Callable[..., T], args: tuple[Any, ...], kwargs: dict[str, Any]
    ) -> None:
        self.target = target
        self.args = args
        self.kwargs = kwargs
        self.label = combine_labels(
            [
                BUILDS_LABEL,
                label_from_name(getattr(target, "__qualname__", repr(target))),
                *(g.label for g in args),
                *(g.label for g in kwargs.values()),
            ]
        )

    def _draw(self, tc: TestCase, printer: PrettyPrinter) -> T:
        name = str(
            getattr(self.target, "__qualname__", None) or getattr(self.target, "__name__", "builds")
        )
        printer.begin_group(len(name) + 1, name + "(")
        first = True
        args = []
        for generator in self.args:
            if not first:
                printer.text(",")
                printer.breakable(" ")
            first = False
            args.append(tc.draw_and_print(generator, printer))
        kwargs = {}
        for key, generator in self.kwargs.items():
            if not first:
                printer.text(",")
                printer.breakable(" ")
            first = False
            printer.text(key + "=")
            kwargs[key] = tc.draw_and_print(generator, printer)
        printer.end_group(")")
        return self.target(*args, **kwargs)

    def do_draw(self, tc: TestCase) -> T:
        return self._draw(tc, _SILENT_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        parts = [repr(a) for a in self.args] + [f"{k}={v!r}" for k, v in self.kwargs.items()]
        return f"builds({getattr(self.target, '__name__', self.target)!r}, {', '.join(parts)})"


def builds(target: Callable[..., T], /, *args: Any, **kwargs: Any) -> Generator[T]:
    """Generate values by calling ``target`` with arguments drawn from the
    given generators: ``builds(f, gs.integers(), name=gs.text())`` draws
    ``f(n, name=s)``.

    Required parameters of ``target`` that are not given, and that have a
    type annotation, are filled in from :func:`from_type` of the
    annotation; parameters with defaults are left to their defaults. The
    value is printed as the call, ``f(0, name='')``. A ``target`` that is
    not callable, or an argument that is not a generator, is an
    :class:`~hegel.InvalidArgument`.

    ::

        @dataclass
        class Point:
            x: int
            y: int

        point = tc.draw(gs.builds(Point))
        origin_row = tc.draw(gs.builds(Point, y=gs.just(0)))
    """
    if not callable(target):
        raise InvalidArgument(f"builds() target {target!r} must be callable")
    arg_gens = tuple(as_generator(a) for a in args)
    kw_gens = {k: as_generator(v) for k, v in kwargs.items()}
    from hegel.generators.from_type import infer_missing_arguments

    arg_gens, kw_gens = infer_missing_arguments(target, arg_gens, kw_gens)
    return Builds(target, arg_gens, kw_gens)
