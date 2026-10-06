# Dropped from the Hypothesis original: test_discard_frozen, test_discard_multiple_frozen,
# test_user_error_and_frozen, test_user_error_and_stoptest, test_lone_user_error,
# test_nested_stoptest, test_frozen_and_stoptest, test_multiple_stoptest_1,
# test_multiple_stoptest_2, test_stoptest_and_hypothesisexception and
# test_multiple_hypothesisexception (raise the engine's internal Frozen/StopTest
# control-flow exceptions from a test body). FlakyBackendFailure has no Hegel
# equivalent (no alternative backends).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

from contextlib import suppress

import pytest

from hegel.hypothesis.errors import FlakyFailure


@pytest.mark.parametrize("ExceptionClass", [FlakyFailure])
def test_exceptiongroups_reconstruct_original_type(ExceptionClass):
    # contextlib.suppress uses .split, and if ExceptionClass does not implement
    # .derive, .split will return a standard ExceptionGroup instead of
    # ExceptionClass.
    # see https://github.com/python/cpython/issues/119287
    class UnrelatedException(Exception):
        pass

    with pytest.raises(ExceptionClass), suppress(UnrelatedException):
        raise ExceptionClass("exception message", [Exception()])


@pytest.mark.parametrize("ExceptionClass", [FlakyFailure])
def test_derived_exception_group(ExceptionClass):
    exception = ExceptionClass("exception message", [Exception()])
    # we don't expect a match, we just want to test .derive.
    match, rest = exception.split(())
    assert match is None
    assert isinstance(rest, ExceptionClass)
