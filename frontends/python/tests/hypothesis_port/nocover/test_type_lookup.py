# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_non_runtime_type_cannot_be_resolved[Any]
# (from_type resolves typing.Any by design); test_non_runtime_type_cannot_be_registered and
# the register_type_generator assertions of test_callable_with_concatenate,
# test_callable_with_paramspec and test_callable_return_typegard_type
# (register_type_generator does not reject non-runtime typing constructs or parametrised
# Callables as keys); test_binary_type_resolution (from_type(Collection[int]) never yields
# bytes).

import typing
from collections.abc import Callable
from typing import Concatenate, ParamSpec

import pytest

from hegel import generators as gs
from hegel.errors import InvalidArgument
from tests.hypothesis_port.common.debug import check_can_generate_examples

try:
    from typing import TypeGuard  # new in 3.10
except ImportError:
    TypeGuard = None

try:
    from typing import TypeIs  # new in 3.13
except ImportError:
    TypeIs = None

NON_RUNTIME_TYPES = (
    typing.Annotated,
    typing.Concatenate,
    typing.ParamSpec,
    typing.TypeGuard,
    typing.ClassVar,
    typing.Final,
    typing.NoReturn,
    typing.Self,
    typing.Required,
    typing.NotRequired,
    typing.Never,
    typing.LiteralString,
    typing.TypeAlias,
    typing.Unpack,
)

_P = ParamSpec("_P")


@pytest.mark.parametrize("non_runtime_type", NON_RUNTIME_TYPES)
def test_non_runtime_type_cannot_be_resolved(non_runtime_type):
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.from_type(non_runtime_type))


def test_callable_with_concatenate():
    func_type = Callable[Concatenate[int, _P], None]
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.from_type(func_type))


def test_callable_with_paramspec():
    func_type = Callable[_P, None]
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.from_type(func_type))


@pytest.mark.parametrize("typ", [TypeGuard, TypeIs])
def test_callable_return_typegard_type(typ):
    if typ is None:
        pytest.skip("Requires modern typing")

    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.from_type(Callable[[], typ[int]]))
