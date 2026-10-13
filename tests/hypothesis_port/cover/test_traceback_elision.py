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


def _shown_frames(tb):
    """The frames a test runner shows: those not marked ``__tracebackhide__``."""
    frames = []
    while tb is not None:
        if not tb.tb_frame.f_locals.get("__tracebackhide__"):
            frames.append(tb.tb_frame.f_code.co_name)
        tb = tb.tb_next
    return frames


# Hegel trims its own frames from the failure's traceback whatever the
# verbosity, and hides the frames it re-raises through (there is no
# HYPOTHESIS_NO_TRACEBACK_TRIM equivalent).
@pytest.mark.parametrize("verbosity", [hegel.Verbosity.normal, hegel.Verbosity.debug])
def test_tracebacks_omit_hypothesis_internals(verbosity):
    @hegel.test(verbosity=verbosity)
    def simplest_failure(tc):
        tc.draw(gs.just(False))
        raise ValueError

    try:
        simplest_failure()
    except ValueError as e:
        # The least possible: this frame and the test body.
        assert _shown_frames(e.__traceback__) == [
            "test_tracebacks_omit_hypothesis_internals",
            "simplest_failure",
        ]
