from __future__ import annotations

import ipaddress
import re
import unicodedata
import uuid

import pytest

import hegel
from hegel import generators as gs
from hegel.generators.strings import Characters, CharacterSpec, Text, TextFromAlphabet
from tests.helpers import find_example, run_test


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
    check_all(gs.text(max_size=2**64 - 2), lambda s: isinstance(s, str), test_cases=5)
    assert repr(gs.text()) == "text(min_size=0)"
    assert find_example(gs.text()) == ""
    assert find_example(gs.text(min_size=1)) == "0"


def test_text_category_filters():
    check_all(
        gs.text(categories=["Lu"], min_size=1),
        lambda s: all(unicodedata.category(c) == "Lu" for c in s),
    )
    check_all(
        gs.text(exclude_categories=["Lu", "Ll"], min_size=1),
        lambda s: all(unicodedata.category(c) not in ("Lu", "Ll", "Cs") for c in s),
    )


def test_text_filters():
    check_all(gs.text(codec="ascii", min_size=1), lambda s: s.isascii())
    check_all(
        gs.text(min_codepoint=65, max_codepoint=70, min_size=1), lambda s: set(s) <= set("ABCDEF")
    )
    check_all(
        gs.text(categories=[], include_characters="ab", min_size=1), lambda s: set(s) <= set("ab")
    )
    check_all(gs.text(exclude_characters="a", codec="ascii"), lambda s: "a" not in s)
    check_all(gs.text(exclude_categories=["Cs"], min_size=1), lambda s: len(s) >= 1)
    with pytest.raises(hegel.InvalidArgument, match="No valid characters"):
        gs.text(categories=["Cs"])


def test_characters_follow_this_pythons_unicode_version():
    ol_onal = dict(min_codepoint=0x1E5D0, max_codepoint=0x1E5FA)
    if unicodedata.category("\U0001e5d0") == "Cn":
        with pytest.raises(hegel.InvalidArgument, match="No valid characters"):
            gs.characters(categories=["L"], **ol_onal)
        with pytest.raises(hegel.InvalidArgument):
            gs.from_regex(r"\w", alphabet=gs.characters(**ol_onal))
    else:
        check_all(
            gs.characters(categories=["L"], **ol_onal),
            lambda c: unicodedata.category(c) == "Lo",
        )
        check_all(
            gs.from_regex(r"\w", alphabet=gs.characters(**ol_onal)),
            lambda s: re.fullmatch(r"\w", s) is not None,
        )


def test_text_alphabets():
    check_all(gs.text("xyz", min_size=1), lambda s: set(s) <= set("xyz"))
    check_all(gs.text(["x", "y"], min_size=1), lambda s: set(s) <= set("xy"))
    check_all(gs.text(gs.characters(codec="ascii"), min_size=1), lambda s: s.isascii())
    check_all(gs.text(gs.sampled_from("ab"), min_size=2), lambda s: set(s) <= set("ab"))
    assert isinstance(gs.text(gs.sampled_from("ab")), TextFromAlphabet)
    assert isinstance(gs.text(gs.characters()), Text)
    assert isinstance(gs.characters(), Characters)
    assert repr(gs.characters()) == "characters()"


def test_empty_alphabet_spells_only_the_empty_string():
    check_all(gs.text(""), lambda s: s == "")
    check_all(gs.text([]), lambda s: s == "")
    with pytest.raises(hegel.InvalidArgument, match="empty alphabet"):
        gs.text("", min_size=1)


def test_text_and_characters_repr_their_arguments():
    assert repr(gs.text("abc", min_size=2)) == "text(alphabet='abc', min_size=2)"
    assert (
        repr(gs.text(gs.sampled_from("ab")))
        == "text(alphabet=sampled_from('ab'), min_size=0)"
    )
    assert (
        repr(gs.text(gs.characters(codec="ascii"), max_size=3))
        == "text(alphabet=characters(max_codepoint=127), min_size=0, max_size=3)"
    )
    assert repr(gs.text(codec="utf-8")) == "text(codec='utf-8', min_size=0)"
    assert repr(gs.characters(codec="ascii", max_codepoint=200)) == "characters(max_codepoint=127)"
    assert repr(gs.characters(codec="ascii", max_codepoint=50)) == "characters(max_codepoint=50)"
    assert (
        repr(gs.characters(exclude_categories=["Lu"])) == "characters(exclude_categories=('Lu',))"
    )
    assert (
        repr(gs.characters(categories=["Lu"], min_codepoint=65, exclude_characters="AZ"))
        == "characters(min_codepoint=65, categories=('Lu',), exclude_characters='AZ')"
    )
    assert (
        repr(gs.characters(categories=(), include_characters=["a", "b"]))
        == "characters(categories=(), include_characters='ab')"
    )


def test_text_alphabet_errors():
    with pytest.raises(hegel.InvalidArgument):
        gs.text("abc", codec="ascii")
    with pytest.raises(hegel.InvalidArgument):
        gs.text(["ab"])
    with pytest.raises(hegel.InvalidArgument):
        gs.text([1])
    with pytest.raises(hegel.InvalidArgument, match="alphabet=123 must be a collection"):
        gs.text(123)
    with pytest.raises(hegel.InvalidArgument, match="alphabet=b'abc' must be a collection"):
        gs.text(b"abc")
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
        dict(min_codepoint="1"),
        dict(max_codepoint=True),
        dict(include_characters=5),
        dict(include_characters=["ab"]),
        dict(exclude_characters=[1]),
    ],
)
def test_character_spec_errors(kwargs):
    with pytest.raises(hegel.InvalidArgument):
        CharacterSpec(**kwargs)


def test_character_collections():
    check_all(gs.characters(categories=[], include_characters=["a", "b"]), lambda c: c in "ab")
    check_all(gs.text(codec="ascii", exclude_characters={"a"}), lambda s: "a" not in s)


def test_characters():
    check_all(gs.characters(), lambda c: len(c) == 1)
    check_all(gs.characters(codec="latin-1"), lambda c: ord(c) < 256)
    check_all(gs.characters(codec="iso-8859-1"), lambda c: ord(c) < 256)
    check_all(gs.characters(codec="utf-8"), lambda c: len(c.encode("utf-8")) <= 4)
    assert find_example(gs.characters()) == "0"


def test_from_regex_with_literal_nul():
    check_all(gs.from_regex("a\x00b", fullmatch=False), lambda s: "a\x00b" in s)
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
    check_all(
        gs.from_regex(re.compile("^a$", re.MULTILINE), fullmatch=False),
        lambda s: re.search("^a$", s, re.M),
    )


def test_from_regex():
    check_all(gs.from_regex(r"\d+"), lambda s: re.fullmatch(r"\d+", s))
    check_all(gs.from_regex(r"\d+", fullmatch=False), lambda s: re.search(r"\d+", s))
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
    assert repr(gs.from_regex("a")) == "from_regex('a', fullmatch=True)"
    with pytest.raises(hegel.InvalidArgument):
        gs.from_regex(5)
    with pytest.raises(hegel.InvalidArgument):
        gs.from_regex("a", alphabet=gs.just("a"))
    with pytest.raises(hegel.InvalidArgument, match="alphabet with a bytes pattern"):
        gs.from_regex(b"a", alphabet="a")
    with pytest.raises(hegel.InvalidArgument):
        gs.from_regex("a", alphabet="")
    with pytest.raises(hegel.InvalidArgument):
        gs.from_regex("(unclosed")


def test_binary():
    check_all(gs.binary(), lambda b: isinstance(b, bytes) and len(b) <= 100)
    check_all(gs.binary(min_size=2, max_size=3), lambda b: 2 <= len(b) <= 3)
    check_all(gs.binary(min_size=150), lambda b: 150 <= len(b) <= 250, test_cases=5)
    check_all(gs.binary(max_size=2**64 - 1), lambda b: isinstance(b, bytes), test_cases=5)
    check_all(gs.binary(max_size=2**64 - 2), lambda b: isinstance(b, bytes), test_cases=5)
    assert repr(gs.binary()) == "binary(min_size=0)"
    assert repr(gs.binary(max_size=7)) == "binary(min_size=0, max_size=7)"
    assert find_example(gs.binary(min_size=2)) == b"\x00\x00"
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
    check_all(gs.ip_addresses(network=ipaddress.ip_network("::1/128")), lambda a: str(a) == "::1")
    assert repr(gs.ip_addresses()) == "ip_addresses()"
    assert repr(gs.ip_addresses(v=4)) == "ip_addresses(v=4)"
    with pytest.raises(hegel.InvalidArgument):
        gs.ip_addresses(v=5)
    with pytest.raises(hegel.InvalidArgument):
        gs.ip_addresses(v=4.0)
    with pytest.raises(hegel.InvalidArgument):
        gs.ip_addresses(v=True)
    with pytest.raises(hegel.InvalidArgument):
        gs.ip_addresses(network="10.0.0.0/8", v=6)
    with pytest.raises(hegel.InvalidArgument, match="must be a string"):
        gs.ip_addresses(network=b"10.0.0.0/8")


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
    with pytest.raises(hegel.InvalidArgument, match="must be a bool"):
        gs.uuids(allow_nil=1)
    with pytest.raises(hegel.InvalidArgument, match="nil UUID has no version"):
        gs.uuids(version=4, allow_nil=True)
