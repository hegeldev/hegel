# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original:
# test_conservative_regex_are_correct_by_construction (base_regex_strategy
# internals).
#
# Hegel's from_regex defaults to fullmatch=True where Hypothesis's defaults
# to False, so tests that relied on Hypothesis's default pass fullmatch=False.

import re
import string
import sys
from functools import reduce

import pytest

import hegel
from hegel import generators as gs


@hegel.composite
def charset(tc):
    negated = tc.draw(gs.booleans())
    chars = tc.draw(gs.text(string.ascii_letters + string.digits, min_size=1))
    if negated:
        return f"[^{chars}]"
    else:
        return f"[{chars}]"


COMBINED_MATCHER = re.compile("[?+*]{2}")


@hegel.composite
def conservative_regex(tc):
    result = tc.draw(
        gs.one_of(
            gs.just("."),
            gs.sampled_from([re.escape(c) for c in string.printable]),
            charset(),
            CONSERVATIVE_REGEX.map(lambda s: f"({s})"),
            CONSERVATIVE_REGEX.map(lambda s: s + "+"),
            CONSERVATIVE_REGEX.map(lambda s: s + "?"),
            CONSERVATIVE_REGEX.map(lambda s: s + "*"),
            gs.lists(CONSERVATIVE_REGEX, min_size=1, max_size=3).map("|".join),
            gs.lists(CONSERVATIVE_REGEX, min_size=1, max_size=3).map("".join),
        )
    )
    tc.assume(COMBINED_MATCHER.search(result) is None)
    control = sum(result.count(c) for c in "?+*")
    tc.assume(control <= 3)
    tc.assume(I_WITH_DOT not in result)  # known to be weird
    return result


CONSERVATIVE_REGEX = conservative_regex()
FLAGS = gs.sets(gs.sampled_from([re.ASCII, re.IGNORECASE, re.MULTILINE, re.DOTALL])).map(
    lambda flag_set: reduce(int.__or__, flag_set, 0)
)


@hegel.test
def test_fuzz_stuff(tc):
    pattern = tc.draw(
        gs.text(min_size=1, max_size=5)
        | gs.binary(min_size=1, max_size=5)
        | CONSERVATIVE_REGEX.filter(bool)
    )
    flags = tc.draw(FLAGS)

    try:
        regex = re.compile(pattern, flags=flags)
    except (re.error, FutureWarning):
        # Possible nested sets, e.g. "[[", trigger a FutureWarning
        tc.reject()

    ex = tc.draw(gs.from_regex(regex, fullmatch=False))

    assert regex.search(ex)


@pytest.mark.skipif(sys.version_info[:2] < (3, 11), reason="new syntax")
@hegel.test
def test_regex_atomic_group(tc):
    pattern = "a(?>bc|b)c"
    ex = tc.draw(gs.from_regex(pattern, fullmatch=False))
    assert re.search(pattern, ex)


@pytest.mark.skipif(sys.version_info[:2] < (3, 11), reason="new syntax")
@hegel.test
def test_regex_possessive(tc):
    pattern = '"[^"]*+"'
    ex = tc.draw(gs.from_regex(pattern, fullmatch=False))
    assert re.search(pattern, ex)


# Some preliminaries, to establish what's happening:
I_WITH_DOT = "\u0130"
assert I_WITH_DOT.swapcase() == "i\u0307"  # note: string of length two!
assert re.compile(I_WITH_DOT, flags=re.IGNORECASE).match(I_WITH_DOT.swapcase())


@pytest.mark.xfail(
    strict=True,
    reason="Engine: for [^\u0130]+ under IGNORECASE libhegel generates U+0307 (combining dot "
    "above), a character of \u0130's multi-character case fold; seed 8 draws it",
)
@hegel.test(seed=8)
def test_case_insensitive_not_literal_never_constructs_multichar_match(tc):
    # So our goal is to confirm that we can never accidentally create a non-matching
    # string by assembling individually allowed characters.
    pattern = re.compile(f"[^{I_WITH_DOT}]+", flags=re.IGNORECASE)
    strategy = gs.from_regex(pattern, fullmatch=True)
    for _ in range(5):
        s = tc.draw(strategy)
        assert pattern.fullmatch(s) is not None
        # And to be on the safe side, we implement this stronger property:
        assert set(s).isdisjoint(I_WITH_DOT.swapcase())


@hegel.test
def test_no_error_converting_negated_sets_to_strategy(tc):
    # CharactersBuilder no longer triggers an internal error converting sets
    # or negated sets to a strategy when multi-char strings are whitelisted.
    tc.draw(gs.from_regex(re.compile(f"[^{I_WITH_DOT}_]", re.IGNORECASE), fullmatch=True))
