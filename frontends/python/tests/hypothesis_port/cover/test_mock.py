# Dropped from the Hypothesis original: test_can_mock_inside_given_without_fixture
# (@given's argument binding of a mock.patch applied under the test decorator).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

"""Checks that @hegel.test, @mock.patch, and pytest fixtures work as expected."""

import math
from unittest import mock

from pytest import Config

import hegel
from hegel import generators as gs


@mock.patch("math.atan")
@hegel.test
def test_can_mock_outside_given_with_fixture(tc, atan, pytestconfig):
    tc.draw(gs.text())
    assert isinstance(atan, mock.MagicMock)
    assert isinstance(math.atan, mock.MagicMock)
    assert isinstance(pytestconfig, Config)


@hegel.test
def test_can_mock_within_test_with_fixture(tc, pytestconfig):
    tc.draw(gs.text())
    assert isinstance(pytestconfig, Config)
    assert not isinstance(math.atan, mock.MagicMock)
    with mock.patch("math.atan") as atan:
        assert isinstance(atan, mock.MagicMock)
        assert isinstance(math.atan, mock.MagicMock)
    assert not isinstance(math.atan, mock.MagicMock)
