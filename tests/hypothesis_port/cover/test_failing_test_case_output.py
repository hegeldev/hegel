# Dropped from the Hypothesis original: test_vararg_output (@given's binding of positional
# arguments).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import notes

# Hegel reports a failing example as one "name = value" line per draw,
# whatever the length, rather than Hypothesis's call-style rendering.
OUTPUT_WITH_BREAK = """
Falsifying example:
x = {0!r}
y = {0!r}
"""


@pytest.mark.parametrize("n", [10, 100])
def test_inserts_line_breaks_only_at_appropriate_lengths(n):
    @hegel.test
    @hegel.explicit_test_case("0" * n, "0" * n)
    def test(tc):
        x = tc.draw(gs.text())
        y = tc.draw(gs.text())
        assert x < y

    with pytest.raises(AssertionError) as err:
        test()

    assert OUTPUT_WITH_BREAK.format("0" * n).strip() == "\n".join(notes(err.value))
