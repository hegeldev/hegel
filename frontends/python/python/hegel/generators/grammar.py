"""Generators of strings from grammars."""

from __future__ import annotations

import os
from collections.abc import Mapping
from pathlib import Path
from typing import TYPE_CHECKING, Any

from hegel import _native
from hegel._labels import combine_labels, label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, _PrintsValue
from hegel.generators.strings import _pattern_alphabet

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["from_lark"]

LARK_LABEL = label_from_name("hegel.from_lark")


def _read_import(directory: Path | None, path: str) -> str | None:
    """The text of the imported grammar file ``path`` under ``directory``,
    or ``None`` when there is no such file."""
    if directory is None:
        return None
    file = directory / path
    try:
        return file.read_text(encoding="utf-8")
    except (FileNotFoundError, NotADirectoryError):
        return None
    except (OSError, UnicodeDecodeError) as e:
        raise InvalidArgument(f"Could not read grammar {str(file)!r}: {e}") from None


def _compile(
    text: str,
    start: str | None,
    max_size: int,
    max_depth: int,
    alphabet: _native.StringGenerator | None,
    names: list[str],
    import_path: Path | None,
) -> _native.Grammar:
    builder = _native.GrammarBuilder(text)
    index = 0
    while (path := builder.file(index)) is not None:
        builder.provide(index, _read_import(import_path, path))
        index += 1
    return _native.Grammar(builder, start, max_size, max_depth, alphabet, names)


def _check_bound(name: str, value: object, bits: int) -> int:
    """A cap as the engine takes it, an unsigned ``bits``-bit integer:
    ``None`` (none given) is 0, for the engine's default."""
    if value is None:
        return 0
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise InvalidArgument(f"{name}={value!r} must be a positive integer")
    if value >= 1 << bits:
        raise InvalidArgument(f"{name}={value!r} must be less than 2**{bits}")
    return value


class Lark(_PrintsValue[str]):
    def __init__(
        self,
        text: str,
        start: str | None,
        explicit: dict[str, Generator[str]],
        alphabet: Any,
        max_size: int | None,
        max_depth: int | None,
        import_path: Path | None,
    ) -> None:
        self.text = text
        self.start = start
        self._explicit = list(explicit.items())
        self._options = [
            (name, value)
            for name, value in [
                ("alphabet", alphabet),
                ("max_size", max_size),
                ("max_depth", max_depth),
                ("import_path", None if import_path is None else str(import_path)),
            ]
            if value is not None
        ]
        self._grammar = _compile(
            text,
            start,
            _check_bound("max_size", max_size, 64),
            _check_bound("max_depth", max_depth, 32),
            _pattern_alphabet("from_lark", alphabet),
            [name for name, _ in self._explicit],
            import_path,
        )
        labels = [LARK_LABEL, label_from_name(text), label_from_name(start or "start")]
        for name, generator in self._explicit:
            labels += [label_from_name(name), generator.label]
        for name, value in self._options:
            labels.append(label_from_name(f"{name}={value!r}"))
        self.label = combine_labels(labels)

    def do_draw(self, tc: TestCase) -> str:
        def value(terminal: int) -> str:
            name, generator = self._explicit[terminal]
            drawn = tc.draw_silent(generator)
            if not isinstance(drawn, str):
                raise InvalidArgument(
                    f"The generator for explicit terminal {name!r} must produce strings, "
                    f"got {drawn!r}"
                )
            return drawn

        return tc._generate_grammar(self._grammar, value)

    def __repr__(self) -> str:
        args = [repr(self.text)]
        if self.start is not None:
            args.append(f"start={self.start!r}")
        if self._explicit:
            args.append(f"explicit={dict(self._explicit)!r}")
        args += [f"{name}={value!r}" for name, value in self._options]
        return f"from_lark({', '.join(args)})"


def from_lark(
    grammar: str,
    *,
    start: str | None = None,
    explicit: Mapping[str, Generator[str]] | None = None,
    alphabet: Any = None,
    max_size: int | None = None,
    max_depth: int | None = None,
    import_path: str | os.PathLike[str] | None = None,
) -> Generator[str]:
    """Generate strings matching ``grammar``, a grammar written in the
    syntax of the `Lark <https://lark-parser.readthedocs.io/en/stable/grammar.html>`_
    parsing library, starting from its ``start`` rule (or the rule named by
    ``start``).

    The whole grammar language is supported: rules, terminals with string
    and regex patterns and priorities, ``%import`` (including Lark's
    bundled ``common``, ``python``, ``unicode`` and ``lark`` grammars),
    ``%ignore``, ``%declare``, ``%override``, ``%extend`` and templates.
    Generated strings are ones Lark's Earley parser accepts for the
    grammar, with ``%ignore``\\ d terminals such as whitespace and comments
    inserted wherever a lexer would skip them; a string Lark's basic lexer
    would read differently is avoided where the grammar allows it. A draw
    that cannot produce such a string after several attempts rejects the
    test case, as a failed :meth:`~hegel.TestCase.assume` does. The
    grammar is parsed and the strings generated by Hegel's engine: Lark
    itself need not be installed.

    ``explicit`` maps terminal names to generators of strings, which supply
    those terminals' values instead of their patterns. A terminal the
    grammar ``%declare``\\ s without defining needs one; any other may be
    overridden. The values are beyond the lexing checks made on the rest of
    the string: it is the generator's part to draw values that read as the
    terminal where they land (a value under a quoted-string terminal should
    contain no quote, say). A value that is not a string is an
    :class:`~hegel.InvalidArgument` when drawn.

    ``alphabet`` restricts the characters the terminals' patterns may
    produce, as for :func:`from_regex`: a string of characters or a
    :func:`characters` generator.

    ``max_size`` caps the length of a string in characters, explicit
    terminals' values included (default 8192, at most 1048576); a string
    whose values push it over is drawn again with less room for the
    grammar's own text. ``max_depth`` caps how deeply rules nest (default
    200, at most 500). ``import_path`` is the directory ``%import``\\ ed
    grammars are read from: ``%import a.b.X`` reads ``a/b.lark`` from it,
    and Lark's bundled grammars stand in when it has no such file. A
    relative import (``%import .lib.X``) in ``a/b.lark`` reads
    ``a/lib.lark``, and in ``grammar`` itself ``lib.lark``.

    A grammar Lark would reject, an undefined start rule, a cap below the
    smallest string the grammar can produce, or an ``explicit`` generator
    for a terminal the grammar does not have is an
    :class:`~hegel.InvalidArgument` when the generator is built.

    ::

        EXPRESSIONS = r'''
            ?start: sum
            ?sum: product | sum "+" product
            ?product: atom | product "*" atom
            ?atom: NUMBER | "(" sum ")"
            %import common.NUMBER
            %import common.WS
            %ignore WS
        '''

        expression = tc.draw(gs.from_lark(EXPRESSIONS))
    """
    if not isinstance(grammar, str):
        raise InvalidArgument(f"grammar={grammar!r} must be a string in Lark's grammar syntax")
    if start is not None and not isinstance(start, str):
        raise InvalidArgument(f"start={start!r} must be a rule name")
    terminals: dict[str, Generator[str]] = {}
    for name, generator in (explicit or {}).items():
        if not isinstance(name, str):
            raise InvalidArgument(f"explicit terminal name {name!r} must be a string")
        if not isinstance(generator, Generator):
            raise InvalidArgument(
                f"explicit[{name!r}]={generator!r} must be a generator of strings"
            )
        terminals[name] = generator
    if import_path is not None and not isinstance(import_path, (str, os.PathLike)):
        raise InvalidArgument(f"import_path={import_path!r} must be a directory path")
    return Lark(
        grammar,
        start,
        terminals,
        alphabet,
        max_size,
        max_depth,
        None if import_path is None else Path(import_path),
    )
