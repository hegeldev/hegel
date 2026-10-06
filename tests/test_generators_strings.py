from __future__ import annotations

import ipaddress
import re
import unicodedata
import uuid

import pytest

import hegel
from hegel import generators as gs
from hegel.generators.strings import Characters, CharacterSpec, Text, TextFromAlphabet
from tests.helpers import run_test


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


def test_text_defaults():
    check_all(gs.text(), lambda s: isinstance(s, str) and len(s) <= 100)
    check_all(gs.text(), lambda s: all(unicodedata.category(c) != "Cs" for c in s))
    check_all(gs.text(min_size=3, max_size=5), lambda s: 3 <= len(s) <= 5)
    check_all(gs.text(min_size=150), lambda s: 150 <= len(s) <= 250, test_cases=5)
    assert repr(gs.text()) == "text(min_size=0, max_size=100)"
    assert hegel.find_example(gs.text()) == ""
    assert hegel.find_example(gs.text(min_size=1)) == "0"


def test_text_filters():
    check_all(gs.text(codec="ascii", min_size=1), lambda s: s.isascii())
    check_all(
        gs.text(min_codepoint=65, max_codepoint=70, min_size=1), lambda s: set(s) <= set("ABCDEF")
    )
    check_all(
        gs.text(categories=["Lu"], min_size=1),
        lambda s: all(unicodedata.category(c) == "Lu" for c in s),
    )
    check_all(
        gs.text(exclude_categories=["Lu", "Ll"], min_size=1),
        lambda s: all(unicodedata.category(c) not in ("Lu", "Ll", "Cs") for c in s),
    )
    check_all(
        gs.text(categories=[], include_characters="ab", min_size=1), lambda s: set(s) <= set("ab")
    )
    check_all(gs.text(exclude_characters="a", codec="ascii"), lambda s: "a" not in s)
    check_all(gs.text(exclude_categories=["Cs"], min_size=1), lambda s: len(s) >= 1)
    with pytest.raises(hegel.InvalidArgument, match="No valid characters"):
        gs.text(categories=["Cs"])


def test_text_alphabets():
    check_all(gs.text("xyz", min_size=1), lambda s: set(s) <= set("xyz"))
    check_all(gs.text(["x", "y"], min_size=1), lambda s: set(s) <= set("xy"))
    check_all(gs.text(gs.characters(codec="ascii"), min_size=1), lambda s: s.isascii())
    check_all(gs.text(gs.sampled_from("ab"), min_size=2), lambda s: set(s) <= set("ab"))
    assert isinstance(gs.text(gs.sampled_from("ab")), TextFromAlphabet)
    assert isinstance(gs.text(gs.characters()), Text)
    assert isinstance(gs.characters(), Characters)
    assert repr(gs.characters()) == "characters()"


def test_text_alphabet_errors():
    with pytest.raises(hegel.InvalidArgument):
        gs.text("abc", codec="ascii")
    with pytest.raises(hegel.InvalidArgument):
        gs.text(["ab"])
    with pytest.raises(hegel.InvalidArgument):
        gs.text("")
    with pytest.raises(hegel.InvalidArgument):
        gs.text([1])
    with pytest.raises(hegel.InvalidArgument):
        gs.text(min_size=-1)
    with pytest.raises(hegel.InvalidArgument):
        gs.text(max_size=True)
    with pytest.raises(hegel.InvalidArgument):
        gs.text(min_size=3, max_size=2)

    def body(tc):
        with pytest.raises(hegel.InvalidArgument):
            tc.draw(gs.text(gs.just("ab"), min_size=1))

    run_test(body, test_cases=1)


@pytest.mark.parametrize(
    "kwargs",
    [
        dict(codec="utf-16"),
        dict(categories=["Lu"], exclude_categories=["Ll"]),
        dict(min_codepoint=-1),
        dict(max_codepoint=0x110000),
        dict(min_codepoint=5, max_codepoint=4),
    ],
)
def test_character_spec_errors(kwargs):
    with pytest.raises(hegel.InvalidArgument):
        CharacterSpec(**kwargs)


def test_characters():
    check_all(gs.characters(), lambda c: len(c) == 1)
    check_all(gs.characters(codec="latin-1"), lambda c: ord(c) < 256)
    check_all(gs.characters(codec="iso-8859-1"), lambda c: ord(c) < 256)
    check_all(gs.characters(codec="utf-8"), lambda c: len(c.encode("utf-8")) <= 4)
    assert hegel.find_example(gs.characters()) == "0"


def test_from_regex_with_literal_nul():
    check_all(gs.from_regex("a\x00b"), lambda s: "a\x00b" in s)
    check_all(gs.from_regex("[\x00]", fullmatch=True), lambda s: s == "\x00")


def test_from_regex_honours_compiled_flags():
    check_all(
        gs.from_regex(re.compile("ab", re.IGNORECASE), fullmatch=True), lambda s: s.lower() == "ab"
    )
    check_all(
        gs.from_regex(re.compile(b"\\w", re.ASCII), fullmatch=True),
        lambda s: s.isalnum() or s == b"_",
    )
    check_all(gs.from_regex(re.compile(" a b ", re.VERBOSE), fullmatch=True), lambda s: s == "ab")
    check_all(gs.from_regex(re.compile("^a$", re.MULTILINE)), lambda s: re.search("^a$", s, re.M))


def test_from_regex():
    check_all(gs.from_regex(r"\d+"), lambda s: re.search(r"\d+", s))
    check_all(
        gs.from_regex(r"^[a-z]{2,4}$", fullmatch=True), lambda s: re.fullmatch(r"[a-z]{2,4}", s)
    )
    check_all(gs.from_regex(re.compile(r"a|b"), fullmatch=True), lambda s: s in "ab")
    check_all(
        gs.from_regex(rb"[a-c]+", fullmatch=True),
        lambda s: isinstance(s, bytes) and re.fullmatch(rb"[a-c]+", s),
    )
    check_all(gs.from_regex(r".+", fullmatch=True, alphabet="xy"), lambda s: set(s) <= set("xy"))
    check_all(
        gs.from_regex(r".+", fullmatch=True, alphabet=gs.characters(codec="ascii")),
        lambda s: s.isascii(),
    )
    assert repr(gs.from_regex("a")) == "from_regex('a', fullmatch=False)"
    with pytest.raises(hegel.InvalidArgument):
        gs.from_regex(5)
    with pytest.raises(hegel.InvalidArgument):
        gs.from_regex("a", alphabet=gs.just("a"))
    with pytest.raises(hegel.InvalidArgument):
        gs.from_regex("(unclosed")


def test_binary():
    check_all(gs.binary(), lambda b: isinstance(b, bytes) and len(b) <= 100)
    check_all(gs.binary(min_size=2, max_size=3), lambda b: 2 <= len(b) <= 3)
    assert repr(gs.binary()) == "binary(min_size=0, max_size=100)"
    assert hegel.find_example(gs.binary(min_size=2)) == b"\x00\x00"
    with pytest.raises(hegel.InvalidArgument):
        gs.binary(min_size=2, max_size=1)


def test_network_generators():
    check_all(gs.emails(), lambda s: "@" in s)
    check_all(gs.urls(), lambda s: "://" in s)
    check_all(gs.domains(), lambda s: "." in s and len(s) <= 255)
    check_all(gs.domains(max_length=10), lambda s: len(s) <= 10)
    assert repr(gs.emails()) == "emails()"
    assert repr(gs.urls()) == "urls()"
    assert repr(gs.domains(max_length=10)) == "domains(max_length=10)"
    with pytest.raises(hegel.InvalidArgument):
        gs.domains(max_length=3)


def test_ip_addresses():
    check_all(gs.ip_addresses(), lambda a: a.version in (4, 6))
    check_all(gs.ip_addresses(v=4), lambda a: isinstance(a, ipaddress.IPv4Address))
    check_all(gs.ip_addresses(v=6), lambda a: isinstance(a, ipaddress.IPv6Address))
    net = ipaddress.ip_network("10.0.0.0/30")
    check_all(gs.ip_addresses(network="10.0.0.0/30"), lambda a: a in net)
    check_all(gs.ip_addresses(network=net, v=4), lambda a: a in net)
    check_all(gs.ip_addresses(network="::1/128"), lambda a: str(a) == "::1")
    assert repr(gs.ip_addresses()) == "ip_addresses()"
    assert repr(gs.ip_addresses(v=4)) == "ip_addresses(v=4)"
    with pytest.raises(hegel.InvalidArgument):
        gs.ip_addresses(v=5)
    with pytest.raises(hegel.InvalidArgument):
        gs.ip_addresses(network="10.0.0.0/8", v=6)


def test_uuids():
    check_all(gs.uuids(), lambda u: isinstance(u, uuid.UUID))
    check_all(gs.uuids(version=4), lambda u: u.version == 4)
    check_all(gs.uuids(version=1), lambda u: u.version == 1)
    seen = set()
    check_all(gs.uuids(allow_nil=True), lambda u: seen.add(u.int) or True, test_cases=200)
    assert 0 in seen
    assert repr(gs.uuids()) == "uuids()"
    assert repr(gs.uuids(version=3)) == "uuids(version=3)"
    with pytest.raises(hegel.InvalidArgument):
        gs.uuids(version=7)
