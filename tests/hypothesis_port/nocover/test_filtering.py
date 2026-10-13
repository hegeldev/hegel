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
from hegel import generators as gs
from hegel.generators import integers, lists


@pytest.mark.parametrize(
    ("specifier", "condition"),
    [(integers(), lambda x: x > 1), (lists(integers()), bool)],
)
def test_filter_correctly(specifier, condition):
    def test_is_filtered(tc):
        x = tc.draw(specifier.filter(condition))
        assert condition(x)

    hegel.run(test_is_filtered)


# A variety of strategies that generate the integers 1-20 inclusive, but might
# differ in their support for special-case filtering.
one_to_twenty_strategies = [
    gs.integers(1, 20),
    gs.integers(0, 19).map(lambda x: x + 1),
    gs.sampled_from(range(1, 21)),
    gs.sampled_from(range(20)).map(lambda x: x + 1),
]


@pytest.mark.parametrize("base", one_to_twenty_strategies)
@hegel.test
def test_chained_filters_agree(tc, base):
    forbidden_values = tc.draw(gs.lists(gs.integers(1, 20), max_size=19, unique=True))

    def forbid(s, forbidden):
        """Helper function to avoid Python variable scoping issues."""
        return s.filter(lambda x: x != forbidden)

    s = base
    for forbidden in forbidden_values:
        s = forbid(s, forbidden)

    x = tc.draw(s)
    assert 1 <= x <= 20
    assert x not in forbidden_values


@pytest.mark.parametrize("base", one_to_twenty_strategies)
def test_chained_filters_repr(base):
    def foo(x):
        return x != 0

    def bar(x):
        return x != 2

    filtered = base.filter(foo)
    chained = filtered.filter(bar)
    assert repr(chained) == f"{base!r}.filter(foo).filter(bar)"
    assert repr(filtered) == f"{base!r}.filter(foo)"
