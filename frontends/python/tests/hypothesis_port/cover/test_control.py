# Dropped from the Hypothesis original: test_can_convert_non_weakref_types_to_event_strings
# (tests the internal _serialize_event helper); test_cannot_cleanup_with_no_context,
# test_cannot_event_with_no_context, test_cleanup_executes_on_leaving_build_context,
# test_can_nest_build_context, test_does_not_suppress_exceptions,
# test_suppresses_exceptions_in_teardown, test_runs_multiple_cleanup_with_teardown,
# test_raises_error_if_cleanup_fails_but_block_does_not, test_raises_if_note_out_of_context,
# test_raises_if_current_build_context_out_of_context, test_current_build_context_is_current,
# test_not_currently_in_hypothesis, test_currently_in_hypothesis,
# test_currently_in_stateful_test (BuildContext, cleanup and currently_in_test_context);
# test_deprecation_warning_if_assume_out_of_context,
# test_deprecation_warning_if_reject_out_of_context (deprecation warnings).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

from dataclasses import dataclass

import pytest

import hegel
from hegel.generators import integers
from tests.helpers import notes
from tests.hypothesis_port.common.utils import capture_out


def test_prints_all_notes_in_verbose_mode():
    messages = set()

    @hegel.test(verbosity=hegel.Verbosity.debug, database="")
    def test(tc):
        x = tc.draw(integers(1, 10))
        msg = f"x -> {x}"
        tc.note(msg)
        messages.add(msg)
        assert x < 5

    with capture_out() as out, pytest.raises(AssertionError):
        test()
    v = out.getvalue()
    for x in sorted(messages):
        assert x in v


@dataclass
class CanBePrettyPrinted:
    n: int


def test_note_pretty_prints():
    # Hegel has no reporter to install; a note is shown with the falsifying
    # example instead.
    @hegel.test
    def test(tc):
        n = tc.draw(integers(1, 10))
        tc.note(CanBePrettyPrinted(n))
        assert n != 5

    with pytest.raises(AssertionError) as err:
        test()

    assert "\n".join(notes(err.value)) == "n = 5\nCanBePrettyPrinted(n=5)"
