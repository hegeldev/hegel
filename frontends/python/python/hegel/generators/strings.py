"""Text, bytes, regex, and network-format generators."""

from __future__ import annotations

import ipaddress
import re
import uuid as _uuid
from collections.abc import Iterable
from typing import TYPE_CHECKING, Any

from hegel import _native
from hegel._labels import label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, _PrintsValue

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


def _check_max_size(min_size: int, max_size: object) -> int | None:
    """``max_size`` checked against ``min_size``; ``None`` (no maximum
    given) stays ``None``, for the engine to apply its default."""
    if max_size is None:
        return None
    max_size = _check_size("max_size", max_size)
    if max_size < min_size:
        raise InvalidArgument(f"Cannot have max_size={max_size!r} < min_size={min_size!r}")
    return max_size


def _sizes(min_size: int, max_size: int | None) -> str:
    if max_size is None:
        return f"min_size={min_size}"
    return f"min_size={min_size}, max_size={max_size}"


def _check_size(name: str, value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise InvalidArgument(f"{name}={value!r} must be a non-negative integer")
    return value


def _check_codepoint(name: str, value: object) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int):
        raise InvalidArgument(f"{name}={value!r} must be an integer")
    if value < 0 or value > 0x10FFFF:
        raise InvalidArgument(f"{name}={value!r} is out of range")
    return value


def _check_characters(name: str, value: Any) -> str | None:
    """``value`` (a string or other collection of single characters) as a
    string; ``None`` stays ``None``."""
    if value is None or isinstance(value, str):
        return value
    try:
        items = list(value)
    except TypeError:
        raise InvalidArgument(f"{name}={value!r} must be a collection of characters") from None
    for item in items:
        if not isinstance(item, str) or len(item) != 1:
            raise InvalidArgument(f"{name} must contain single characters, got {item!r}")
    return "".join(items)


def _describe_character_args(
    codec: str | None,
    min_codepoint: int | None,
    max_codepoint: int | None,
    categories: Iterable[str] | None,
    exclude_categories: Iterable[str] | None,
    include_characters: str | None,
    exclude_characters: str | None,
) -> str:
    """The non-default arguments of a ``characters()`` call, as its repr
    shows them. An ASCII codec is shown as the codepoint bound it is."""
    if codec == "ascii":
        codec = None
        if max_codepoint is None or max_codepoint > 127:
            max_codepoint = 127
    args = [
        ("codec", codec),
        ("min_codepoint", min_codepoint),
        ("max_codepoint", max_codepoint),
        ("categories", None if categories is None else tuple(categories)),
        ("exclude_categories", None if exclude_categories is None else tuple(exclude_categories)),
        ("include_characters", include_characters),
        ("exclude_characters", exclude_characters),
    ]
    return ", ".join(f"{name}={value!r}" for name, value in args if value not in (None, ""))


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
        include_characters: Iterable[str] | None = None,
        exclude_characters: Iterable[str] | None = None,
    ) -> None:
        if codec is not None and codec not in SUPPORTED_CODECS:
            raise InvalidArgument(
                f"invalid codec: {codec!r}; supported codecs are {', '.join(SUPPORTED_CODECS)}"
            )
        if categories is not None and exclude_categories is not None:
            raise InvalidArgument("Cannot pass both categories and exclude_categories")
        min_codepoint = _check_codepoint("min_codepoint", min_codepoint)
        max_codepoint = _check_codepoint("max_codepoint", max_codepoint)
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
        self.include_characters = _check_characters("include_characters", include_characters)
        self.exclude_characters = _check_characters("exclude_characters", exclude_characters)

    def build(self, min_size: int, max_size: int | None) -> _native.StringGenerator:
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


def _alphabet_spec(alphabet: Any) -> CharacterSpec:
    if isinstance(alphabet, Characters):
        return alphabet.spec
    if isinstance(alphabet, Generator):
        raise InvalidArgument(
            f"alphabet={alphabet!r} must be a string of characters, a collection of single "
            "characters or a characters() generator; to build text from an arbitrary "
            'generator of characters, use lists(alphabet).map("".join)'
        )
    if isinstance(alphabet, (bytes, bytearray)):
        raise InvalidArgument(f"alphabet={alphabet!r} must be a collection of characters")
    return CharacterSpec(categories=[], include_characters=_check_characters("alphabet", alphabet))


def _pattern_alphabet(who: str, alphabet: Any) -> _native.StringGenerator | None:
    """The engine alphabet constraining the characters a pattern draws:
    ``None`` for no constraint, else a string of characters or a
    :func:`characters` generator, which must not be empty."""
    if alphabet is None:
        return None
    spec = _alphabet_spec(alphabet)
    if not isinstance(alphabet, Characters) and not spec.include_characters:
        raise InvalidArgument(f"{who} alphabet must not be empty")
    return spec.build(0, 0)


class Text(_PrintsValue[str]):
    def __init__(
        self, spec: CharacterSpec, min_size: int, max_size: int | None, args: str = ""
    ) -> None:
        self.spec = spec
        self.min_size = min_size
        self.max_size = max_size
        self._handle = spec.build(min_size, max_size)
        self._args = args
        self.label = TEXT_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_string(self._handle)

    def __repr__(self) -> str:
        prefix = f"{self._args}, " if self._args else ""
        return f"text({prefix}{_sizes(self.min_size, self.max_size)})"


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
    include_characters: Iterable[str] | None = None,
    exclude_characters: Iterable[str] | None = None,
) -> Generator[str]:
    """Generate Unicode strings of between ``min_size`` and ``max_size``
    characters, shrinking toward shorter strings of simpler characters
    (``""``, then ``"0"``).

    Without ``max_size`` the engine applies its default maximum for text:
    100 characters, or ``min_size + 100`` when ``min_size`` is larger than
    that, whatever the alphabet. A negative size or ``max_size < min_size`` is an
    :class:`~hegel.InvalidArgument`.

    ``alphabet`` restricts the characters: a string or sequence of
    characters, or a :func:`characters` generator. Any other generator is
    an :class:`~hegel.InvalidArgument`; to build text from an arbitrary
    generator of characters, draw a list of them and join it:
    ``gs.lists(chars).map("".join)``. An empty alphabet gives
    only ``""``. The keyword filters (``codec``, codepoint bounds, Unicode
    categories, included and excluded characters) are shorthand for a
    :func:`characters` alphabet, with the same meanings, and cannot be
    combined with ``alphabet``. Surrogate code points are never generated
    unless asked for by category.

    Unicode categories follow the Unicode version of the running Python's
    :mod:`unicodedata`, so ``str.isalpha`` and friends agree with them.

    ::

        name = tc.draw(gs.text(min_size=1))
        word = tc.draw(gs.text(alphabet="abc", max_size=5))
        ascii_text = tc.draw(gs.text(codec="ascii"))
    """
    min_size = _check_size("min_size", min_size)
    max_size = _check_max_size(min_size, max_size)
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
        if not isinstance(alphabet, Characters) and not spec.include_characters:
            # An empty alphabet spells only the empty string.
            if min_size > 0:
                raise InvalidArgument(f"Cannot have min_size={min_size!r} with an empty alphabet")
            from hegel.generators.combinators import just

            return just("")
        return Text(spec, min_size, max_size, f"alphabet={alphabet!r}")
    spec = _default_spec(
        codec,
        min_codepoint,
        max_codepoint,
        categories,
        exclude_categories,
        include_characters,
        exclude_characters,
    )
    args = _describe_character_args(
        codec,
        min_codepoint,
        max_codepoint,
        categories,
        exclude_categories,
        spec.include_characters,
        spec.exclude_characters,
    )
    return Text(spec, min_size, max_size, args)


def _default_spec(
    codec: str | None,
    min_codepoint: int | None,
    max_codepoint: int | None,
    categories: Iterable[str] | None,
    exclude_categories: Iterable[str] | None,
    include_characters: Iterable[str] | None,
    exclude_characters: Iterable[str] | None,
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


class Characters(_PrintsValue[str]):
    def __init__(self, spec: CharacterSpec, args: str = "") -> None:
        self.spec = spec
        self._handle = spec.build(1, 1)
        self._args = args
        self.label = CHARACTERS_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_string(self._handle)

    def __repr__(self) -> str:
        return f"characters({self._args})"


def characters(
    *,
    codec: str | None = None,
    min_codepoint: int | None = None,
    max_codepoint: int | None = None,
    categories: Iterable[str] | None = None,
    exclude_categories: Iterable[str] | None = None,
    include_characters: Iterable[str] | None = None,
    exclude_characters: Iterable[str] | None = None,
) -> Generator[str]:
    """Generate single characters (strings of length one), shrinking toward
    ``"0"``.

    Each argument narrows the characters generated:

    - ``codec``: only characters that the codec can encode. One of
      ``"ascii"``, ``"latin-1"`` (or ``"iso-8859-1"``) and ``"utf-8"``.
    - ``min_codepoint``, ``max_codepoint``: an inclusive range of code
      points, within ``0`` to ``0x10FFFF``.
    - ``categories``: only these Unicode general categories, by two-letter
      name (``"Lu"``) or by major class (``"L"`` for every letter).
    - ``exclude_categories``: every category but these. Surrogates
      (``"Cs"``) are always excluded unless ``categories`` names them.
    - ``include_characters``: characters to allow whatever the other
      filters say.
    - ``exclude_characters``: characters never to generate.

    ``categories`` and ``exclude_categories`` cannot both be given. An
    unknown codec or category, an out-of-range code point, or
    ``max_codepoint < min_codepoint`` is an :class:`~hegel.InvalidArgument`.
    Categories follow the Unicode version of the running Python's
    :mod:`unicodedata`.

    ::

        upper = tc.draw(gs.characters(categories=["Lu"]))
        printable_ascii = tc.draw(gs.characters(min_codepoint=32, max_codepoint=126))
    """
    spec = _default_spec(
        codec,
        min_codepoint,
        max_codepoint,
        categories,
        exclude_categories,
        include_characters,
        exclude_characters,
    )
    args = _describe_character_args(
        codec,
        min_codepoint,
        max_codepoint,
        categories,
        exclude_categories,
        spec.include_characters,
        spec.exclude_characters,
    )
    return Characters(spec, args)


class Regex(_PrintsValue[str]):
    def __init__(self, pattern: str, fullmatch: bool, alphabet: Any) -> None:
        self.pattern = pattern
        self.fullmatch = fullmatch
        alphabet_handle = _pattern_alphabet("from_regex", alphabet)
        self._handle = _native.StringGenerator.regex(pattern, fullmatch, alphabet_handle)
        self.label = REGEX_LABEL

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_string(self._handle)

    def __repr__(self) -> str:
        return f"from_regex({self.pattern!r}, fullmatch={self.fullmatch})"


def from_regex(regex: Any, *, fullmatch: bool = True, alphabet: Any = None) -> Generator[str]:
    """Generate strings matching ``regex``, a pattern string or a compiled
    :class:`re.Pattern` (whose flags are honoured).

    By default the whole string matches, as :func:`re.fullmatch` checks;
    with ``fullmatch=False`` the string merely contains a match, as
    :func:`re.search` would find, and may have arbitrary text around it.
    (Hypothesis's ``from_regex`` defaults to ``fullmatch=False``.)

    ``alphabet`` restricts the characters the pattern's character classes
    and ``.`` may produce, as a string of characters or a
    :func:`characters` generator; it cannot be empty, and cannot be used
    with a bytes pattern. A bytes pattern generates ``bytes``, matching
    with the semantics of ``re`` for bytes (``\\w`` and friends are
    ASCII-only).

    The pattern is parsed and the strings generated by Hegel's engine, not
    by :mod:`re`. A pattern the engine cannot parse, or one that can match
    nothing over its alphabet, is an :class:`~hegel.InvalidArgument` when
    the generator is built.

    ::

        identifier = tc.draw(gs.from_regex(r"[a-z_][a-z0-9_]*"))
    """
    pattern = getattr(regex, "pattern", regex)
    flags = getattr(regex, "flags", 0)
    if isinstance(pattern, bytes):
        if alphabet is not None:
            raise InvalidArgument("Cannot use an alphabet with a bytes pattern")
        # Bytes patterns are ASCII-only for \\w and friends, as in Python.
        pattern = _inline_flags(flags | re.ASCII) + pattern.decode("latin-1")
        latin: Generator[Any] = Regex(pattern, bool(fullmatch), characters(max_codepoint=255))
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


class Binary(_PrintsValue[bytes]):
    def __init__(self, min_size: int, max_size: int | None) -> None:
        self.min_size = min_size
        self.max_size = max_size
        self.label = BINARY_LABEL

    def do_draw(self, tc: TestCase) -> bytes:
        return tc._generate_bytes(self.min_size, self.max_size)

    def __repr__(self) -> str:
        return f"binary({_sizes(self.min_size, self.max_size)})"


def binary(*, min_size: int = 0, max_size: int | None = None) -> Generator[bytes]:
    """Generate byte strings of between ``min_size`` and ``max_size``
    bytes, shrinking toward shorter strings of zero bytes.

    Without ``max_size`` the engine applies its default maximum: 100
    bytes, or ``min_size + 100`` when ``min_size`` is larger than that. A
    negative size or ``max_size < min_size`` is an
    :class:`~hegel.InvalidArgument`.
    """
    min_size = _check_size("min_size", min_size)
    return Binary(min_size, _check_max_size(min_size, max_size))


class EngineString(_PrintsValue[str]):
    def __init__(self, handle: _native.StringGenerator, label: int, name: str) -> None:
        self._handle = handle
        self.label = label
        self.name = name

    def do_draw(self, tc: TestCase) -> str:
        return tc._generate_string(self._handle)

    def __repr__(self) -> str:
        return self.name


def emails() -> Generator[str]:
    """Generate email addresses, as strings.

    Each is a local part of RFC 5322 ``atext`` characters and dots, ``@``,
    and a domain as :func:`domains` generates. The addresses are well
    formed, not real.
    """
    return EngineString(_native.StringGenerator.email(), EMAIL_LABEL, "emails()")


def urls() -> Generator[str]:
    """Generate RFC 3986 URLs, as strings, with an ``http`` or ``https``
    scheme and a host as :func:`domains` generates."""
    return EngineString(_native.StringGenerator.url(), URL_LABEL, "urls()")


def domains(*, max_length: int = 255) -> Generator[str]:
    """Generate fully qualified domain names of at most ``max_length``
    characters, as strings, ending in a top-level domain from the IANA
    list. ``max_length`` must be between 4 and 255 (the default), or it is
    an :class:`~hegel.InvalidArgument`."""
    if not isinstance(max_length, int) or not (4 <= max_length <= 255):
        raise InvalidArgument(f"max_length={max_length!r} must be between 4 and 255")
    return EngineString(
        _native.StringGenerator.domain(max_length),
        DOMAIN_LABEL,
        f"domains(max_length={max_length})",
    )


class IpAddresses(_PrintsValue[Any]):
    def __init__(self, v: int | None) -> None:
        self.v = v
        self.label = {None: IP_LABEL, 4: IPV4_LABEL, 6: IPV6_LABEL}[v]

    def do_draw(self, tc: TestCase) -> Any:
        v = self.v
        if v is None:
            v = 4 if tc._generate_integer(0, 1) == 0 else 6
        if v == 4:
            return ipaddress.IPv4Address(tc._generate_ipv4())
        return ipaddress.IPv6Address(tc._generate_ipv6())

    def __repr__(self) -> str:
        return "ip_addresses()" if self.v is None else f"ip_addresses(v={self.v})"


def ip_addresses(*, v: int | None = None, network: Any = None) -> Generator[Any]:
    """Generate IP addresses, as :class:`ipaddress.IPv4Address` or
    :class:`ipaddress.IPv6Address` objects.

    ``v=4`` or ``v=6`` picks one version; by default either may be
    generated. ``network`` (a string such as ``"10.0.0.0/8"``, or an
    :class:`ipaddress.IPv4Network` or :class:`ipaddress.IPv6Network`)
    restricts the addresses to that network, including its network and
    broadcast addresses. Addresses shrink toward the lowest one: ``0.0.0.0``,
    ``::``, or the network address. A ``v`` other than 4 or 6, or one that
    disagrees with ``network``, is an :class:`~hegel.InvalidArgument`.
    """
    if v is not None and (isinstance(v, bool) or not isinstance(v, int) or v not in (4, 6)):
        raise InvalidArgument(f"v={v!r} must be 4 or 6")
    if network is not None:
        if isinstance(network, str):
            net = ipaddress.ip_network(network)
        elif isinstance(network, (ipaddress.IPv4Network, ipaddress.IPv6Network)):
            net = network
        else:
            raise InvalidArgument(
                f"network={network!r} must be a string, an IPv4Network or an IPv6Network"
            )
        if v is not None and v != net.version:
            raise InvalidArgument(f"network={network!r} is IPv{net.version} but v={v!r}")
        from hegel.generators.numeric import integers

        first = int(net.network_address)
        last = int(net.broadcast_address)
        return integers(first, last).map(net.network_address.__class__)
    return IpAddresses(v)


class Uuids(_PrintsValue[_uuid.UUID]):
    def __init__(self, version: int | None, allow_nil: bool) -> None:
        self.version = version
        self.allow_nil = allow_nil
        self.label = UUID_LABEL

    def do_draw(self, tc: TestCase) -> _uuid.UUID:
        if self.allow_nil and tc._generate_boolean(0.1):
            return _uuid.UUID(int=0)
        return _uuid.UUID(bytes=tc._generate_uuid(self.version))

    def __repr__(self) -> str:
        return "uuids()" if self.version is None else f"uuids(version={self.version})"


def uuids(*, version: int | None = None, allow_nil: bool = False) -> Generator[_uuid.UUID]:
    """Generate :class:`uuid.UUID` objects.

    With ``version`` (1 to 5) every UUID is an RFC 4122 UUID of that
    version. Without it, the UUIDs are arbitrary 128-bit values, most of
    which have no RFC 4122 version. The nil UUID (all zeros) is generated
    only with ``allow_nil=True``, which cannot be combined with
    ``version``. Invalid arguments are an :class:`~hegel.InvalidArgument`.
    """
    if version is not None and version not in (1, 2, 3, 4, 5):
        raise InvalidArgument(f"version={version!r} must be between 1 and 5")
    if not isinstance(allow_nil, bool):
        raise InvalidArgument(f"allow_nil={allow_nil!r} must be a bool")
    if allow_nil and version is not None:
        raise InvalidArgument(
            f"Cannot have allow_nil=True with version={version!r}: the nil UUID has no version"
        )
    return Uuids(version, allow_nil)
