# Dropped from the Hypothesis original: test_given_twice_is_same,
# test_data_supports_find, test_errors_when_normal_strategy_functions_are_used,
# test_nice_repr (the st.data() strategy object).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

from pytest import raises

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.utils import notes


@hegel.test
def test_conditional_draw(tc):
    x = tc.draw(gs.integers())
    y = tc.draw(gs.integers(min_value=x))
    assert y >= x


def test_prints_on_failure():
    @hegel.test
    def test(tc):
        x = tc.draw(gs.lists(gs.integers(0, 10), min_size=2))
        y = tc.draw(gs.sampled_from(x))
        x.remove(y)
        if y in x:
            raise ValueError

    with raises(ValueError) as err:
        test()
    text = "\n".join(notes(err.value))
    assert "x = [0, 0]" in text
    assert "y = 0" in text


def test_prints_labels_if_given_on_failure():
    @hegel.test
    def test(tc):
        x = tc.draw(gs.lists(gs.integers(0, 10), min_size=2), name="Some numbers")
        y = tc.draw(gs.sampled_from(x), name="A number")
        assert y in x
        x.remove(y)
        assert y not in x

    with raises(AssertionError) as err:
        test()
    # Hegel prints an explicitly named draw as "label_N = value".
    text = "\n".join(notes(err.value))
    assert "Some numbers_1 = [0, 0]" in text
    assert "A number_1 = 0" in text
