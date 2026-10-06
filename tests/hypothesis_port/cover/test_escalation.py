# Dropped from the Hypothesis original: test_is_hypothesis_file_not_confused_by_prefix,
# test_is_hypothesis_file_does_not_error_on_invalid_paths_issue_2319,
# test_handles_null_traceback, test_handles_context, test_handles_groups and
# test_handles_cycles (test the internal escalation module and InterestingOrigin).
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

from hegel.hypothesis import errors


def test_multiplefailures_deprecation():
    with pytest.warns(errors.HypothesisDeprecationWarning):
        exc = errors.MultipleFailures
    assert exc is BaseExceptionGroup


def test_errors_attribute_error():
    with pytest.raises(AttributeError):
        errors.ThisIsNotARealAttributeDontCreateSomethingWithThisName
