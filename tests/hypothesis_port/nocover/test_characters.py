# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import codecs
import string
from encodings.aliases import aliases

import pytest

from hegel.hypothesis import given, settings, strategies as st
from hegel.hypothesis.errors import NonRoundTrippableCharactersWarning

from tests.hypothesis_port.common.debug import find_any
from tests.hypothesis_port.common.utils import Why, xfail_on_crosshair

IDENTIFIER_CHARS = string.ascii_letters + string.digits + "_"


@given(st.characters(exclude_characters=IDENTIFIER_CHARS))
def test_large_blacklist(c):
    assert c not in IDENTIFIER_CHARS


@xfail_on_crosshair(Why.symbolic_outside_context)  # seems like a crosshair bug here
@given(st.data())
def test_arbitrary_blacklist(data):
    blacklist = data.draw(st.text(st.characters(max_codepoint=1000), min_size=1))
    ords = list(map(ord, blacklist))
    c = data.draw(
        st.characters(
            exclude_characters=blacklist,
            min_codepoint=max(0, min(ords) - 1),
            max_codepoint=max(0, max(ords) + 1),
        )
    )
    assert c not in blacklist


def _enc(cdc):
    try:
        "".encode(cdc)
        return True
    except Exception:
        return False


lots_of_encodings = sorted(x for x in set(aliases).union(aliases.values()) if _enc(x))
assert len(lots_of_encodings) > 100  # sanity-check


# Hegel has no public intervals_from_codec; find the non-round-tripping
# characters of a codec by brute force.
_NON_ROUNDTRIP_CACHE = {}


def non_roundtrip_chars(codec):
    codec = codecs.lookup(codec).name
    if codec not in _NON_ROUNDTRIP_CACHE:
        bad = []
        for i in range(0x10000):
            c = chr(i)
            try:
                if c.encode(codec).decode(codec) != c:
                    bad.append(c)
            except (UnicodeError, ValueError):
                pass
        _NON_ROUNDTRIP_CACHE[codec] = "".join(bad)
    return _NON_ROUNDTRIP_CACHE[codec]


@pytest.mark.xfail(
    strict=True, reason="hegel characters(codec=) only supports ascii, latin-1 and utf-8"
)
@pytest.mark.skipif(
    settings.get_current_profile_name() == "crosshair",
    reason="takes 2000s; large & slow symbolic strings",
)
@given(data=st.data(), codec=st.sampled_from(lots_of_encodings))
@settings(max_examples=5)
def test_can_constrain_characters_to_codec(data, codec):
    # excluding the non-round-tripping characters (if any) avoids the warning
    # and generates only strings which round-trip
    strategy = st.characters(codec=codec, exclude_characters=non_roundtrip_chars(codec))
    s = data.draw(st.text(strategy, min_size=25))
    assert s.encode(codec).decode(codec) == s


# We use OneCharStringStrategy.from_characters_args rather than st.characters
# below because @cacheable strategies only realize - and therefore warn - once
# per distinct arguments, which would make these tests order-dependent.
# Dropped test_warns_for_non_round_tripping_characters: uses OneCharStringStrategy internals


@pytest.mark.xfail(
    strict=True, reason="hegel characters(codec=) only supports ascii, latin-1 and utf-8"
)
def test_no_warning_if_other_arguments_exclude_non_round_tripping_characters():
    # "¥" (U+A5) and "‾" (U+203E) are already outside the codepoint range
    strategy = st.characters(codec="shift_jis", min_codepoint=200, max_codepoint=8000)
    strategy.validate()
    find_any(strategy)


@pytest.mark.xfail(
    strict=True, reason="hegel characters(codec=) only supports ascii, latin-1 and utf-8"
)
@pytest.mark.skipif(
    settings.get_current_profile_name() == "crosshair",
    reason="large & slow symbolic strings",
)
@pytest.mark.parametrize("codec", ["shift_jis", "cp950", "iso2022_jp", "iso2022_kr"])
@given(data=st.data())
def test_codec_strings_round_trip_if_non_round_tripping_chars_excluded(codec, data):
    chars = st.characters(codec=codec, exclude_characters=non_roundtrip_chars(codec))
    s = data.draw(st.text(chars, min_size=1))
    assert s.encode(codec).decode(codec) == s
