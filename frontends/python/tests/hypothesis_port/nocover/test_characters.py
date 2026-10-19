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
# test_warns_for_non_round_tripping_characters (OneCharStringStrategy
# internals and NonRoundTrippableCharactersWarning);
# test_can_constrain_characters_to_codec,
# test_no_warning_if_other_arguments_exclude_non_round_tripping_characters,
# test_codec_strings_round_trip_if_non_round_tripping_chars_excluded
# (characters(codec=) beyond ascii, latin-1 and utf-8; hegel supports no
# other codecs).

import string

import hegel
from hegel import generators as gs

IDENTIFIER_CHARS = string.ascii_letters + string.digits + "_"


@hegel.test
def test_large_blacklist(tc):
    c = tc.draw(gs.characters(exclude_characters=IDENTIFIER_CHARS))
    assert c not in IDENTIFIER_CHARS


@hegel.test
def test_arbitrary_blacklist(tc):
    blacklist = tc.draw(gs.text(gs.characters(max_codepoint=1000), min_size=1))
    ords = list(map(ord, blacklist))
    c = tc.draw(
        gs.characters(
            exclude_characters=blacklist,
            min_codepoint=max(0, min(ords) - 1),
            max_codepoint=max(0, max(ords) + 1),
        )
    )
    assert c not in blacklist
