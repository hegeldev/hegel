# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_self_referential_forward_ref,
# test_self_referential_forward_ref_nested, test_mutually_recursive_types_with_typevar,
# test_mutually_recursive_types_with_typevar_alternate (resolving forward references
# from the caller's local namespace); test_recursive_forward_ref_resolution_is_bounded
# (patches from_typing_type internals).

from typing import Dict as _Dict

import pytest

from hegel import generators as gs
from hegel.errors import ResolutionFailed
from tests.hypothesis_port.common.debug import find_any

# error only occurs with typing variants
# ruff: noqa: UP006, UP035, UP007

# Mutually-recursive types
# See https://github.com/HypothesisWorks/hypothesis/issues/2722


def test_forward_ref_to_undefined_still_fails():
    # Forward references to undefined names should still fail
    A = _Dict[bool, "UndefinedType"]  # noqa: F821

    with pytest.raises(ResolutionFailed, match=r"Could not resolve"):
        find_any(gs.from_type(A))
