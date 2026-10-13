# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import uuid

import pytest

from hegel import InvalidArgument
from hegel import generators as gs
from tests.hypothesis_port.common.debug import (
    assert_no_examples,
    check_can_generate_examples,
    find_any,
)


def test_no_nil_uuid_by_default():
    assert_no_examples(gs.uuids(), lambda x: x == uuid.UUID(int=0))


def test_can_generate_nil_uuid():
    find_any(gs.uuids(allow_nil=True), lambda x: x == uuid.UUID(int=0))


def test_can_allow_nil_uuid_with_none_version():
    check_can_generate_examples(gs.uuids(version=None, allow_nil=True))


@pytest.mark.xfail(
    strict=True,
    reason="hegel uuids() accepts allow_nil=True with a version (the nil UUID has no "
    "version) and coerces allow_nil to bool rather than validating it",
)
def test_can_only_allow_nil_uuid_with_none_version():
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.uuids(version=4, allow_nil=True))
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.uuids(version=None, allow_nil="not a bool"))
