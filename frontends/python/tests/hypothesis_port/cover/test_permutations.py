# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_cannot_permute_non_sequence_types
# (permutations rejecting non-sequences; hegel accepts any iterable).

import hegel
from hegel.generators import permutations
from tests.hypothesis_port.common.debug import minimal


def test_can_find_non_trivial_permutation():
    x = minimal(permutations(list(range(5))), lambda x: x[0] != 0)

    assert x == [1, 0, 2, 3, 4]


@hegel.test
def test_permutation_values_are_permutations(tc):
    perm = tc.draw(permutations(list("abcd")))
    assert len(perm) == 4
    assert set(perm) == set("abcd")


@hegel.test
def test_empty_permutations_are_empty(tc):
    xs = tc.draw(permutations([]))
    assert xs == []
