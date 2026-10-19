# Dropped from the Hypothesis original: test_notes_hard_to_satisfy,
# test_flaky_exit, test_stops_after_x_shrinks (the run's stop reason);
# test_formats_are_evaluated_only_once (event label caching);
# test_exact_timing, test_apparently_instantaneous_tests, test_draw_timing
# (phase and draw timings); test_has_lambdas_in_output,
# test_assume_adds_event_with_function_origin,
# test_reject_adds_event_with_function_origin (filter/assume/reject
# events); test_explain_phase_is_separate_from_shrink_phase (Phase.explain);
# test_stateful_states_are_deduped,
# test_stateful_with_one_of_bundles_states_are_deduped (Bundle events);
# test_statistics_for_threshold_problem, test_statistics_with_events_and_target's
# 'Highest target score' assertion (libhegel reports no target scores).
# Hegel has no statistics API: the tests read the ``show_statistics`` output.
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import re

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.utils import capture_out


def call_for_statistics(test_function):
    """The ``Statistics (over N test cases):`` block printed for a run."""
    with capture_out() as out:
        test_function()
    output = out.getvalue()
    assert "Statistics (over " in output, output
    return output[output.index("Statistics (over ") :]


def unique_events(stats):
    return {m.group(1) for m in re.finditer(r"^  \* (.*): [\d.]+% of test cases$", stats, re.M)}


def test_can_callback_with_a_string():
    @hegel.test(show_statistics=True)
    def test(tc):
        tc.draw(gs.integers())
        tc.event("hi")

    stats = call_for_statistics(test)
    assert any("hi" in s for s in unique_events(stats))


def test_does_not_report_on_examples():
    @hegel.explicit_test_case("hi")
    @hegel.test(show_statistics=True)
    def test(tc):
        i = tc.draw(gs.integers())
        if isinstance(i, str):
            tc.event("boo")

    stats = call_for_statistics(test)
    assert not unique_events(stats)


def test_statistics_with_events():
    @hegel.test(show_statistics=True)
    def test(tc):
        value = tc.draw(gs.integers(0, 10_000))
        tc.event(value)
        tc.event_value("value", value)
        tc.target(float(value), label="a target")

    stats = call_for_statistics(test)
    assert re.search(r"^  \* \d+: [\d.]+% of test cases$", stats, re.M), stats
    assert re.search(r"^  \* value: count \d+, min \d+", stats, re.M), stats


@hegel.test
def test_event_with_non_weakrefable_keys(tc):
    b = tc.draw(gs.booleans())
    tc.event((b,))
