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
# test_raise_invalid_argument_for_nonempty_list_of_nothing (lists(nothing(),
# min_size=1) is not an error in Hegel).

import inspect
from inspect import Parameter

import pytest

from hegel import generators as gs
from tests.hypothesis_port.common.arguments import argument_validation_test, e

BAD_ARGS = []


def adjust(ex, **kwargs):
    f, a, b = ex
    b = dict(b)
    b.update(kwargs)
    BAD_ARGS.append((f, a, b))


for ex in [
    e(gs.lists, gs.integers()),
    e(gs.sets, gs.integers()),
    e(gs.frozensets, gs.integers()),
    e(gs.dicts, gs.integers(), gs.integers()),
    e(gs.text),
    e(gs.binary),
]:
    adjust(ex, min_size=-1)
    adjust(ex, max_size=-1)
    adjust(ex, min_size="no")
    adjust(ex, max_size="no")


test_raise_invalid_argument = argument_validation_test(BAD_ARGS)


ALL_GENERATORS_COPY = {
    name: getattr(gs, name)
    for name in gs.__all__
    if callable(getattr(gs, name)) and not isinstance(getattr(gs, name), type)
}


@pytest.mark.parametrize("name", sorted(ALL_GENERATORS_COPY))
def test_consistent_with_api_guide_on_kwonly_args(name):
    # Enforce the style-guide: if it has a default value, it should be
    # keyword-only, with a few exceptions.
    generator = ALL_GENERATORS_COPY[name]
    for arg in inspect.signature(generator).parameters.values():
        assert (
            arg.default == Parameter.empty
            or arg.kind != Parameter.POSITIONAL_OR_KEYWORD
            or arg.name in ("min_value", "max_value")
            or name == "text"
        ), f"need kwonly args in {name}"
