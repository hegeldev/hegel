"""Text, bytes, regex, and network-format generators."""

from __future__ import annotations

import ipaddress
import re
import uuid as _uuid
from collections.abc import Iterable
from typing import TYPE_CHECKING, Any

from hegel import _native
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, label_from_name

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = [
    "binary",
    "characters",
    "domains",
    "emails",
    "from_regex",
    "ip_addresses",
    "text",
    "urls",
    "uuids",
]

DEFAULT_MAX_SIZE = 100
SUPPORTED_CODECS = ("ascii", "latin-1", "iso-8859-1", "utf-8")

TEXT_LABEL = label_from_name("hegel.text")
CHARACTERS_LABEL = label_from_name("hegel.characters")
REGEX_LABEL = label_from_name("hegel.from_regex")
BINARY_LABEL = label_from_name("hegel.binary")
EMAIL_LABEL = label_from_name("hegel.emails")
URL_LABEL = label_from_name("hegel.urls")
DOMAIN_LABEL = label_from_name("hegel.domains")
IP_LABEL = label_from_name("hegel.ip_addresses")
IPV4_LABEL = label_from_name("hegel.ip_addresses.v4")
IPV6_LABEL = label_from_name("hegel.ip_addresses.v6")
UUID_LABEL = label_from_name("hegel.uuids")


def _default_max_size(min_size: int, max_size: int | None) -> int:
    if max_size is not None:
        if max_size < min_size:
            raise InvalidArgument(f"Cannot have max_size={max_size!r} < min_size={min_size!r}")
        return max_size
    return min_size + DEFAULT_MAX_SIZE if min_size > DEFAULT_MAX_SIZE else DEFAULT_MAX_SIZE


def _check_size(name: str, value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise InvalidArgument(f"{name}={value!r} must be a non-negative integer")
    return value


class CharacterSpec:
    """The character filtering parameters shared by ``text`` and
    ``characters``."""

    def __init__(
        self,
        *,
        codec: str | None = None,
        min_codepoint: int | None = None,
        max_codepoint: int | None = None,
        categories: Iterable[str] | None = None,
        exclude_categories: Iterable[str] | None = None,
        include_characters: str | None = None,
        exclude_characters: str | None = None,
    ) -> None:
        if codec is not None and codec not in SUPPORTED_CODECS:
            raise InvalidArgument(
                f"invalid codec: {codec!r}; supported codecs are {', '.join(SUPPORTED_CODECS)}"
            )
        if categories is not None and exclude_categories is not None:
            raise InvalidArgument("Cannot pass both categories and exclude_categories")
        if min_codepoint is not None and (min_codepoint < 0 or min_codepoint > 0x10FFFF):
            raise InvalidArgument(f"min_codepoint={min_codepoint!r} is out of range")
        if max_codepoint is not None and (max_codepoint < 0 or max_codepoint > 0x10FFFF):
            raise InvalidArgument(f"max_codepoint={max_codepoint!r} is out of range")
        if (
            min_codepoint is not None
            and max_codepoint is not None
            and min_codepoint > max_codepoint
        ):
            raise InvalidArgument(
                f"Cannot have max_codepoint={max_codepoint!r} < min_codepoint={min_codepoint!r}"
            )
        self.codec = codec
        self.min_codepoint = min_codepoint
        self.max_codepoint = max_codepoint
        self.categories = None if categories is None else [str(c) for c in categories]
        self.exclude_categories = (
            None if exclude_categories is None else [str(c) for c in exclude_categories]
        )
        self.include_characters = include_characters
        self.exclude_characters = exclude_characters

    def build(self, min_size: int, max_size: int) -> _native.StringGenerator:
        return _native.StringGenerator.text(
            min_size,
            max_size,
            self.codec,
            self.min_codepoint or 0,
            self.max_codepoint,
            self.categories,
            self.exclude_categories,
            self.include_characters,
            self.exclude_characters,
        )


def _alphabet_spec(alphabet: Any) -> CharacterSpec | Generator[str]:
    if isinstance(alphabet, Characters):
        return alphabet.spec
    if isinstance(alphabet, Generator):
        return alphabet
    items = list(alphabet)
    for item in items:
        if not isinstance(item, str) or len(item) != 1:
            raise InvalidArgument(f"alphabet must contain single characters, got {item!r}")
    if not items:
        raise InvalidArgument("alphabet must not be empty")
    return CharacterSpec(categories=[], include_characters="".join(items))


class Text(Generator[str]):
    def __init__(self, spec: CharacterSpec, min_size: int, max_size: int) -> None:
        self.spec = spec
        self.min_size = min_size
        self.max_size = max_size
        self._handle = spec.build(min_size, max_size)
        self.label = TEXT_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc.generate_string(self._handle)

    def __repr__(self) -> str:
        return f"text(min_size={self.min_size}, max_size={self.max_size})"


class TextFromAlphabet(Generator[str]):
    """Text whose characters come from an arbitrary generator."""

    def __init__(self, alphabet: Generator[str], min_size: int, max_size: int) -> None:
        from hegel.generators.collections import lists

        self.alphabet = alphabet
        self.min_size = min_size
        self.max_size = max_size
        self._lists = lists(alphabet, min_size=min_size, max_size=max_size)
        self.label = TEXT_LABEL

    def do_draw(self, tc: TestCase) -> str:
        chars = tc.draw_silent(self._lists)
        for char in chars:
            if not isinstance(char, str) or len(char) != 1:
                raise InvalidArgument(
                    "The alphabet generator for text() must produce single characters, "
                    f"got {char!r}"
                )
        return "".join(chars)


def text(
    alphabet: Any = None,
    *,
    min_size: int = 0,
    max_size: int | None = None,
    codec: str | None = None,
    min_codepoint: int | None = None,
    max_codepoint: int | None = None,
    categories: Iterable[str] | None = None,
    exclude_categories: Iterable[str] | None = None,
    include_characters: str | None = None,
    exclude_characters: str | None = None,
) -> Generator[str]:
    """Generate Unicode strings.

    ``alphabet`` restricts the characters: a string or sequence of
    characters, a ``characters()`` generator, or any generator of single
    characters. The keyword filters (``codec``, codepoint bounds, Unicode
    categories, included and excluded characters) are shorthand for a
    ``characters()`` alphabet and cannot be combined with ``alphabet``.
    Surrogate code points are never generated unless asked for by category.
    """
    min_size = _check_size("min_size", min_size)
    max_size = _default_max_size(
        min_size, None if max_size is None else _check_size("max_size", max_size)
    )
    char_params = any(
        p is not None
        for p in (
            codec,
            min_codepoint,
            max_codepoint,
            categories,
            exclude_categories,
            include_characters,
            exclude_characters,
        )
    )
    if alphabet is not None:
        if char_params:
            raise InvalidArgument("Cannot combine alphabet with character filtering arguments")
        spec = _alphabet_spec(alphabet)
        if isinstance(spec, Generator):
            return TextFromAlphabet(spec, min_size, max_size)
        return Text(spec, min_size, max_size)
    spec = _default_spec(
        codec,
        min_codepoint,
        max_codepoint,
        categories,
        exclude_categories,
        include_characters,
        exclude_characters,
    )
    return Text(spec, min_size, max_size)


def _default_spec(
    codec: str | None,
    min_codepoint: int | None,
    max_codepoint: int | None,
    categories: Iterable[str] | None,
    exclude_categories: Iterable[str] | None,
    include_characters: str | None,
    exclude_characters: str | None,
) -> CharacterSpec:
    if categories is None:
        excluded = list(exclude_categories) if exclude_categories is not None else []
        if "Cs" not in excluded:
            excluded.append("Cs")
        exclude_categories = excluded
    return CharacterSpec(
        codec=codec,
        min_codepoint=min_codepoint,
        max_codepoint=max_codepoint,
        categories=categories,
        exclude_categories=exclude_categories,
        include_characters=include_characters,
        exclude_characters=exclude_characters,
    )


class Characters(Generator[str]):
    def __init__(self, spec: CharacterSpec) -> None:
        self.spec = spec
        self._handle = spec.build(1, 1)
        self.label = CHARACTERS_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc.generate_string(self._handle)

    def __repr__(self) -> str:
        return "characters()"


def characters(
    *,
    codec: str | None = None,
    min_codepoint: int | None = None,
    max_codepoint: int | None = None,
    categories: Iterable[str] | None = None,
    exclude_categories: Iterable[str] | None = None,
    include_characters: str | None = None,
    exclude_characters: str | None = None,
) -> Generator[str]:
    """Generate single characters, filtered by codec, codepoint range,
    Unicode general category, and explicit inclusions and exclusions."""
    return Characters(
        _default_spec(
            codec,
            min_codepoint,
            max_codepoint,
            categories,
            exclude_categories,
            include_characters,
            exclude_characters,
        )
    )


class Regex(Generator[str]):
    def __init__(self, pattern: str, fullmatch: bool, alphabet: Any) -> None:
        self.pattern = pattern
        self.fullmatch = fullmatch
        alphabet_handle = None
        if alphabet is not None:
            spec = _alphabet_spec(alphabet)
            if isinstance(spec, Generator):
                raise InvalidArgument(
                    "from_regex alphabet must be a string of characters or characters()"
                )
            alphabet_handle = spec.build(0, 0)
        # The engine takes a NUL-terminated pattern, so spell literal NULs
        # as the escape the regex parser accepts.
        escaped = pattern.replace("\x00", "\\x00")
        self._handle = _native.StringGenerator.regex(escaped, fullmatch, alphabet_handle)
        self.label = REGEX_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc.generate_string(self._handle)

    def __repr__(self) -> str:
        return f"from_regex({self.pattern!r}, fullmatch={self.fullmatch})"


def from_regex(regex: Any, *, fullmatch: bool = False, alphabet: Any = None) -> Generator[str]:
    """Generate strings matching ``regex`` (a pattern string or compiled
    pattern). With ``fullmatch`` the whole string matches; otherwise the
    string merely contains a match, as ``re.search`` would find."""
    pattern = getattr(regex, "pattern", regex)
    flags = getattr(regex, "flags", 0)
    if isinstance(pattern, bytes):
        # Bytes patterns are ASCII-only for \\w and friends, as in Python.
        pattern = _inline_flags(flags | re.ASCII) + pattern.decode("latin-1")
        latin: Generator[Any] = Regex(
            pattern, bool(fullmatch), alphabet or characters(max_codepoint=255)
        )
        return latin.map(lambda s: s.encode("latin-1"))
    if not isinstance(pattern, str):
        raise InvalidArgument(f"regex={regex!r} must be a string or compiled pattern")
    return Regex(_inline_flags(flags) + pattern, bool(fullmatch), alphabet)


_INLINE_FLAGS = [
    (re.ASCII, "a"),
    (re.IGNORECASE, "i"),
    (re.MULTILINE, "m"),
    (re.DOTALL, "s"),
    (re.VERBOSE, "x"),
]


def _inline_flags(flags: int) -> str:
    """A compiled pattern's flags as the inline prefix the engine understands."""
    letters = "".join(letter for flag, letter in _INLINE_FLAGS if flags & flag)
    return f"(?{letters})" if letters else ""


class Binary(Generator[bytes]):
    def __init__(self, min_size: int, max_size: int) -> None:
        self.min_size = min_size
        self.max_size = max_size
        self.label = BINARY_LABEL

    def do_draw(self, tc: TestCase) -> bytes:
        return tc.generate_bytes(self.min_size, self.max_size)

    def __repr__(self) -> str:
        return f"binary(min_size={self.min_size}, max_size={self.max_size})"


def binary(*, min_size: int = 0, max_size: int | None = None) -> Generator[bytes]:
    """Generate byte strings."""
    min_size = _check_size("min_size", min_size)
    return Binary(
        min_size,
        _default_max_size(
            min_size, None if max_size is None else _check_size("max_size", max_size)
        ),
    )


class EngineString(Generator[str]):
    def __init__(self, handle: _native.StringGenerator, label: int, name: str) -> None:
        self._handle = handle
        self.label = label
        self.name = name

    def do_draw(self, tc: TestCase) -> str:
        return tc.generate_string(self._handle)

    def __repr__(self) -> str:
        return self.name


def emails() -> Generator[str]:
    """Generate email addresses."""
    return EngineString(_native.StringGenerator.email(), EMAIL_LABEL, "emails()")


def urls() -> Generator[str]:
    """Generate URLs."""
    return EngineString(_native.StringGenerator.url(), URL_LABEL, "urls()")


def domains(*, max_length: int = 255) -> Generator[str]:
    """Generate domain names of at most ``max_length`` characters."""
    if not isinstance(max_length, int) or not (4 <= max_length <= 255):
        raise InvalidArgument(f"max_length={max_length!r} must be between 4 and 255")
    return EngineString(
        _native.StringGenerator.domain(max_length),
        DOMAIN_LABEL,
        f"domains(max_length={max_length})",
    )


class IpAddresses(Generator[Any]):
    def __init__(self, v: int | None) -> None:
        self.v = v
        self.label = {None: IP_LABEL, 4: IPV4_LABEL, 6: IPV6_LABEL}[v]

    def do_draw(self, tc: TestCase) -> Any:
        v = self.v
        if v is None:
            v = 4 if tc.generate_integer(0, 1) == 0 else 6
        if v == 4:
            return ipaddress.IPv4Address(tc.generate_ipv4())
        return ipaddress.IPv6Address(tc.generate_ipv6())

    def __repr__(self) -> str:
        return "ip_addresses()" if self.v is None else f"ip_addresses(v={self.v})"


def ip_addresses(*, v: int | None = None, network: Any = None) -> Generator[Any]:
    """Generate IP addresses: IPv4 or IPv6 (``v=4`` or ``v=6`` to pick), or
    addresses within ``network``."""
    if network is not None:
        net = ipaddress.ip_network(network) if isinstance(network, str) else network
        if v is not None and v != net.version:
            raise InvalidArgument(f"network={network!r} is IPv{net.version} but v={v!r}")
        from hegel.generators.numeric import integers

        first = int(net.network_address)
        last = int(net.broadcast_address)
        return integers(first, last).map(net.network_address.__class__)
    if v not in (None, 4, 6):
        raise InvalidArgument(f"v={v!r} must be 4 or 6")
    return IpAddresses(v)


class Uuids(Generator[_uuid.UUID]):
    def __init__(self, version: int | None, allow_nil: bool) -> None:
        self.version = version
        self.allow_nil = allow_nil
        self.label = UUID_LABEL

    def do_draw(self, tc: TestCase) -> _uuid.UUID:
        if self.allow_nil and tc.generate_boolean(0.1):
            return _uuid.UUID(int=0)
        return _uuid.UUID(bytes=tc.generate_uuid(self.version))

    def __repr__(self) -> str:
        return "uuids()" if self.version is None else f"uuids(version={self.version})"


def uuids(*, version: int | None = None, allow_nil: bool = False) -> Generator[_uuid.UUID]:
    """Generate UUIDs, of any RFC 4122 version or the given one (1 to 5)."""
    if version is not None and version not in (1, 2, 3, 4, 5):
        raise InvalidArgument(f"version={version!r} must be between 1 and 5")
    return Uuids(version, bool(allow_nil))
