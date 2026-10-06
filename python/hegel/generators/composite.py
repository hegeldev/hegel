"""Generators defined by user functions: ``composite`` and ``builds``."""

from __future__ import annotations

import functools
import inspect
from collections.abc import Callable
from typing import TYPE_CHECKING, Any, TypeVar

from hegel.errors import InvalidArgument
from hegel.generators.base import NOOP_PRINTER, Generator, combine_labels, label_from_name
from hegel.generators.combinators import as_generator
from hegel.pretty import PrettyPrinter

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["builds", "composite"]

T = TypeVar("T")

COMPOSITE_LABEL = label_from_name("hegel.composite")
BUILDS_LABEL = label_from_name("hegel.builds")


class Composite(Generator[T]):
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
    """Define a generator as a function taking a :class:`TestCase` first.

    Calling the decorated function with the remaining arguments returns a
    generator; each draw runs the function body, whose own draws are
    silent (they are reported as part of the composite's value).
    """
    if not callable(function):
        raise InvalidArgument("composite() takes a function")
    signature = inspect.signature(function)
    if not signature.parameters:
        raise InvalidArgument(
            f"{getattr(function, '__name__', function)} must take the test case "
            "as its first argument"
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
        return self._draw(tc, NOOP_PRINTER)

    def do_draw_and_print(self, tc: TestCase, printer: PrettyPrinter) -> T:
        return self._draw(tc, printer)

    def __repr__(self) -> str:
        parts = [repr(a) for a in self.args] + [f"{k}={v!r}" for k, v in self.kwargs.items()]
        return f"builds({getattr(self.target, '__name__', self.target)!r}, {', '.join(parts)})"


def builds(target: Callable[..., T], /, *args: Any, **kwargs: Any) -> Generator[T]:
    """Generate values by calling ``target`` with arguments drawn from the
    given generators. Missing required arguments with type annotations are
    inferred with ``from_type``."""
    if not callable(target):
        raise InvalidArgument(f"builds() target {target!r} must be callable")
    arg_gens = tuple(as_generator(a) for a in args)
    kw_gens = {k: as_generator(v) for k, v in kwargs.items()}
    from hegel.generators.from_type import infer_missing_arguments

    kw_gens = infer_missing_arguments(target, len(arg_gens), kw_gens)
    return Builds(target, arg_gens, kw_gens)
