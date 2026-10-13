# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import hegel
from hegel import generators as gs
from hegel.generators.from_type import _BUILTINS, _registry
from tests.hypothesis_port.common.debug import find_any

TYPES = sorted(
    (
        x
        for x in {*_BUILTINS, *_registry}
        if isinstance(x, type) and x.__module__ != "typing" and x.__name__ != "ByteString"
    ),
    key=str,
)


def everything_except(excluded_types):
    """Recipe copied from the docstring of ``from_type``"""
    return (
        gs.from_type(type)
        .flat_map(gs.from_type)
        .filter(lambda x: not isinstance(x, excluded_types))
    )


@hegel.test
def test_recipe_for_everything_except(tc):
    excluded_types = tc.draw(
        gs.lists(gs.sampled_from(TYPES), min_size=1, max_size=3, unique=True).map(tuple)
    )
    value = tc.draw(everything_except(excluded_types))
    assert not isinstance(value, excluded_types)


def test_issue_4144_regression():
    find_any(everything_except(()), lambda t: t is not type)
