# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.
#
# Dropped from the Hypothesis original:
# test_list_of_nothing, test_set_of_nothing, test_validates_min_size,
# test_function_composition, test_tuples_detect_empty_elements,
# test_fixed_dictionaries_detect_empty_values, test_empty (Hegel generators
# have no is_empty; nothing() only rejects when drawn).

from hegel import generators as gs
from tests.hypothesis_port.common.debug import assert_no_examples, minimal


def test_resampling():
    x = minimal(
        gs.lists(gs.integers(), min_size=1).flat_map(lambda x: gs.lists(gs.sampled_from(x))),
        lambda x: len(x) >= 10 and len(set(x)) == 1,
    )
    assert x == [0] * 10


def test_no_examples():
    assert_no_examples(gs.nothing())
