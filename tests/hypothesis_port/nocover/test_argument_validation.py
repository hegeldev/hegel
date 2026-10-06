# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import inspect
from inspect import Parameter

import pytest

from hegel.hypothesis import given, settings, strategies as st
from hegel.hypothesis.errors import InvalidArgument

from tests.hypothesis_port.common.arguments import e, e_to_str

BAD_ARGS = []


def adjust(ex, **kwargs):
    f, a, b = ex
    b = dict(b)
    b.update(kwargs)
    BAD_ARGS.append((f, a, b))


for ex in [
    e(st.lists, st.integers()),
    e(st.sets, st.integers()),
    e(st.frozensets, st.integers()),
    e(st.dictionaries, st.integers(), st.integers()),
    e(st.text),
    e(st.binary),
]:
    adjust(ex, min_size=-1)
    adjust(ex, max_size=-1)
    adjust(ex, min_size="no")
    adjust(ex, max_size="no")


@pytest.mark.parametrize(
    ("function", "args", "kwargs"), BAD_ARGS, ids=list(map(e_to_str, BAD_ARGS))
)
def test_raise_invalid_argument(function, args, kwargs):
    # Hegel validates arguments eagerly, so the strategy is built inside the
    # raises block (Hypothesis defers validation until the test runs).
    with pytest.raises(InvalidArgument):

        @given(function(*args, **kwargs))
        @settings(report_multiple_bugs=False)
        def test(x):
            pass

        test()


@pytest.mark.xfail(
    strict=True,
    reason="hegel reports lists(nothing(), min_size=1) as Unsatisfiable, not InvalidArgument",
)
def test_raise_invalid_argument_for_nonempty_list_of_nothing():
    with pytest.raises(InvalidArgument):

        @given(st.lists(st.nothing(), unique=True, min_size=1))
        @settings(report_multiple_bugs=False)
        def test(x):
            pass

        test()


ALL_STRATEGIES_COPY = {
    name: getattr(st, name)
    for name in st.__all__
    if callable(getattr(st, name)) and not isinstance(getattr(st, name), type)
}


@pytest.mark.parametrize("name", sorted(ALL_STRATEGIES_COPY))
def test_consistent_with_api_guide_on_kwonly_args(name):
    # Enforce our style-guide: if it has a default value, it should be
    # keyword-only, with a few exceptions.
    strategy = ALL_STRATEGIES_COPY[name]
    for arg in inspect.signature(strategy).parameters.values():
        assert (
            arg.default == Parameter.empty
            or arg.kind != Parameter.POSITIONAL_OR_KEYWORD
            or arg.name in ("min_value", "max_value", "subtype_strategy", "columns")
            or name in ("text", "range_indexes", "badly_draw_lists", "write_pattern")
        ), f"need kwonly args in {name}"
