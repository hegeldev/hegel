# Dropped from the Hypothesis original: test_shows_target_scores_with_multiple_failures
# (Hypothesis's "Highest target score" report line); test_garbage_collects_the_secondary_key
# (example database internals).
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
from tests.hypothesis_port.common.utils import assert_output_contains_failure, capture_out


def capture_reports(test):
    with capture_out() as o, pytest.raises(ExceptionGroup) as err:
        test()

    return o.getvalue() + "\n\n".join(
        f"{e!r}\n" + "\n".join(notes(e)) for e in (err.value, *err.value.exceptions)
    )


def test_raises_multiple_failures_with_varying_type():
    target = None

    @hegel.test(database="", test_cases=100, report_multiple_failures=True)
    def test(tc):
        nonlocal target
        i = tc.draw(gs.integers())
        if abs(i) < 1000:
            return
        if target is None:
            # Ensure that we have some space to shrink into, so we can't
            # trigger an minimal example and mask the other exception type.
            tc.assume(1003 < abs(i))
            target = i
        exc_class = TypeError if target == i else ValueError
        raise exc_class

    output = capture_reports(test)
    assert "TypeError" in output
    assert "ValueError" in output


def test_raises_multiple_failures_when_position_varies():
    target = None

    @hegel.test(test_cases=100, report_multiple_failures=True)
    def test(tc):
        nonlocal target
        i = tc.draw(gs.integers())
        if abs(i) < 1000:
            return
        if target is None:
            target = i
        if target == i:
            raise ValueError("loc 1")
        else:
            raise ValueError("loc 2")

    output = capture_reports(test)
    assert "loc 1" in output
    assert "loc 2" in output


def test_replays_both_failing_values(tmp_path):
    target = None

    @hegel.test(database=str(tmp_path), test_cases=500, report_multiple_failures=True)
    def test(tc):
        nonlocal target
        i = tc.draw(gs.integers())
        if abs(i) < 1000:
            return
        if target is None:
            target = i
        exc_class = TypeError if target == i else ValueError
        raise exc_class

    with pytest.raises(ExceptionGroup):
        test()

    with pytest.raises(ExceptionGroup):
        test()


@pytest.mark.parametrize("fix", [TypeError, ValueError])
def test_replays_slipped_examples_once_initial_bug_is_fixed(fix, tmp_path):
    target = []
    bug_fixed = False

    @hegel.test(database=str(tmp_path), test_cases=500, report_multiple_failures=True)
    def test(tc):
        i = tc.draw(gs.integers())
        if abs(i) < 1000:
            return
        if not target:
            target.append(i)
        if i == target[0]:
            if bug_fixed and fix is TypeError:
                return
            raise TypeError
        if len(target) == 1:
            target.append(i)
        if bug_fixed and fix is ValueError:
            return
        if i == target[1]:
            raise ValueError

    with pytest.raises(ExceptionGroup):
        test()

    bug_fixed = True

    with pytest.raises(ValueError if fix is TypeError else TypeError):
        test()


def test_shrinks_both_failures():
    first_has_failed = False
    duds = set()
    second_target = None

    @hegel.test(database="", test_cases=1000, report_multiple_failures=True)
    def test(tc):
        nonlocal first_has_failed, duds, second_target
        i = tc.draw(gs.integers(min_value=0))

        if i >= 10000:
            first_has_failed = True
            raise AssertionError

        assert i < 10000
        if first_has_failed:
            if second_target is None:
                for j in range(10000):
                    if j not in duds:
                        second_target = j
                        break
            # to avoid flaky errors, don't error on an input that we previously
            # passed.
            if i not in duds:
                assert i < second_target
        else:
            duds.add(i)

    output = capture_reports(test)
    assert_output_contains_failure(output, test, i=10000)
    assert_output_contains_failure(output, test, i=second_target)


def _is_flaky(exception):
    # Hegel reports a failure it could not reproduce reliably with a caveat
    # note, rather than as a distinct exception type.
    return any("nondeterministic" in note for note in notes(exception))


@pytest.mark.xfail(
    strict=True,
    reason="Hegel reports only the stable failure (TypeError, with a nondeterminism caveat) "
    "instead of a group that also holds the flaky one",
)
def test_handles_flaky_tests_where_only_one_is_flaky(tmp_path):
    flaky_fixed = False

    target = []
    flaky_failed_once = False

    @hegel.test(database=str(tmp_path), test_cases=1000, report_multiple_failures=True)
    def test(tc):
        nonlocal flaky_failed_once
        i = tc.draw(gs.integers())
        if abs(i) < 1000:
            return
        if not target:
            target.append(i)
        if i == target[0]:
            raise TypeError
        if flaky_failed_once and not flaky_fixed:
            return
        if len(target) == 1:
            target.append(i)
        if i == target[1]:
            flaky_failed_once = True
            raise ValueError

    with pytest.raises(ExceptionGroup) as err:
        test()
    assert any(_is_flaky(e) for e in err.value.exceptions)

    flaky_fixed = True

    with pytest.raises(ExceptionGroup) as err:
        test()
    assert not any(_is_flaky(e) for e in err.value.exceptions)


@pytest.mark.parametrize("allow_multi", [True, False])
def test_can_disable_multiple_error_reporting(allow_multi):
    seen = set()

    @hegel.test(database="", derandomize=True, report_multiple_failures=allow_multi)
    def test(tc):
        i = tc.draw(gs.integers(min_value=0))
        # We will pass on the minimal i=0, then fail with a large i, and eventually
        # slip to i=1 and a different error.  We check both seen and raised errors.
        if i == 1:
            seen.add(TypeError)
            raise TypeError
        elif i >= 2:
            seen.add(ValueError)
            raise ValueError

    with pytest.raises(ExceptionGroup if allow_multi else TypeError):
        test()
    assert seen == {TypeError, ValueError}


def test_finds_multiple_failures_in_generation():
    special = None
    seen = set()

    @hegel.test(
        phases=[hegel.Phase.generate, hegel.Phase.shrink],
        test_cases=100,
        report_multiple_failures=True,
    )
    def test(tc):
        """Constructs a test so the 10th largeish example we've seen is a
        special failure, and anything new we see after that point that
        is larger than it is a different failure. This demonstrates that we
        can keep generating larger examples and still find new bugs after that
        point."""
        nonlocal special
        x = tc.draw(gs.integers(min_value=0))
        if not special:
            # don't mark duplicate inputs as special and thus erroring, to avoid
            # flakiness where we passed the input the first time but failed it the
            # second.
            if len(seen) >= 10 and x <= 1000 and x not in seen:
                special = x
            else:
                seen.add(x)

        if special:
            assert x in seen or x <= special
        assert x != special

    with pytest.raises(ExceptionGroup):
        test()


def test_stops_immediately_if_not_report_multiple_bugs():
    seen = set()

    @hegel.test(phases=[hegel.Phase.generate], report_multiple_failures=False)
    def test(tc):
        x = tc.draw(gs.integers())
        seen.add(x)
        raise AssertionError

    with pytest.raises(AssertionError):
        test()
    assert len(seen) == 1


def test_stops_immediately_on_replay(tmp_path):
    seen = set()

    @hegel.test(database=str(tmp_path), phases=tuple(hegel.Phase)[:-1])
    def test(tc):
        x = tc.draw(gs.integers())
        seen.add(x)
        assert x

    # On the first run, we find the failure (Hypothesis would keep looking
    # for up to ten more examples; Hegel stops at once):
    with pytest.raises(AssertionError):
        test()
    assert seen

    # With failing test cases in the database, we stop at one.
    seen.clear()
    with pytest.raises(AssertionError):
        test()
    assert len(seen) == 1
