# Dropped from the Hypothesis original: test_does_not_print_debug_in_verbose,
# test_does_print_debug_in_debug, test_does_print_verbose_in_debug (the reporter API:
# debug_report/verbose_report).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import os
import sys

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import notes


def test_prints_output_by_default():
    @hegel.test
    def test_int(tc):
        tc.draw(gs.integers())
        raise AssertionError

    with pytest.raises(AssertionError) as err:
        test_int()
    assert "draw_1 = " in "\n".join(notes(err.value))


def test_can_report_when_system_locale_is_ascii(monkeypatch):
    # Hegel's default output goes to stderr; a failing example that cannot be
    # encoded there must still be reported, not turned into a UnicodeEncodeError.
    read, write = os.pipe()
    with (
        open(read, encoding="ascii") as read,
        open(write, "w", encoding="ascii") as write,
    ):
        monkeypatch.setattr(sys, "stderr", write)

        @hegel.test
        def test_snowman(tc):
            x = tc.draw(gs.just("☃"))  # noqa: F841 (names the draw)
            raise AssertionError

        with pytest.raises(AssertionError) as err:
            test_snowman()
        assert "x = '☃'" in "\n".join(notes(err.value))
