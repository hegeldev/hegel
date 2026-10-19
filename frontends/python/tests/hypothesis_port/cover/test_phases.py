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
from hegel import InvalidArgument, Phase, Settings
from hegel import generators as gs


@hegel.test(phases=(Phase.explicit,))
@hegel.explicit_test_case(11)
def test_only_runs_explicit_examples(tc):
    i = tc.draw(gs.integers())
    assert i == 11


@hegel.test(phases=(Phase.reuse, Phase.generate, Phase.shrink))
@hegel.explicit_test_case("hello world")
def test_does_not_use_explicit_examples(tc):
    i = tc.draw(gs.booleans())
    assert isinstance(i, bool)


def test_this_would_fail_if_you_ran_it(tmp_path):
    @hegel.test(phases=(Phase.reuse, Phase.shrink), database=tmp_path)
    def test(tc):
        tc.draw(gs.booleans())
        raise AssertionError

    test()


@pytest.mark.parametrize(
    "arg,expected",
    [
        (tuple(Phase)[::-1], frozenset(Phase)),
        ([Phase.explicit, Phase.explicit], frozenset({Phase.explicit})),
    ],
)
def test_sorts_and_dedupes_phases(arg, expected):
    assert Settings(phases=arg).phases == expected


def test_phases_default_to_all():
    assert Settings().phases == frozenset(Phase)


# A failure on a value the generate phase has no realistic chance of
# producing again, so that a later run can only see it by replaying the
# database.
def rarely_generated(tc):
    x = tc.draw(gs.integers(0, 10**9))
    assert x <= 10**8
    return x


def test_does_not_reuse_saved_examples_if_reuse_not_in_phases(tmp_path):
    seen = []

    @hegel.test(database=tmp_path, test_cases=1000)
    def test_usage(tc):
        seen.append(rarely_generated(tc))

    with pytest.raises(AssertionError):
        test_usage()
    saved = seen[-1]

    seen.clear()

    @hegel.test(database=tmp_path, phases=(Phase.generate,), test_cases=20)
    def test_usage(tc):  # noqa: F811
        seen.append(rarely_generated(tc))

    try:
        test_usage()
    except AssertionError:
        pass
    assert saved not in seen


def test_will_save_when_reuse_not_in_phases(tmp_path):
    seen = []

    @hegel.test(database=tmp_path, phases=(Phase.generate,))
    def test_usage(tc):
        seen.append(tc.draw(gs.integers()))
        raise ValueError

    with pytest.raises(ValueError):
        test_usage()
    saved = seen[-1]

    seen.clear()

    @hegel.test(database=tmp_path, phases=(Phase.reuse,))
    def test_usage(tc):  # noqa: F811
        seen.append(tc.draw(gs.integers()))
        raise ValueError

    with pytest.raises(ValueError):
        test_usage()
    assert seen[0] == saved


def test_rejects_non_phases():
    with pytest.raises(InvalidArgument):
        Settings(phases=["cabbage"])
