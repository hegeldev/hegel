# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

"""
We need these test to make sure ``TypeVar('X', bound='MyType')`` works correctly.

There was a problem previously that ``bound='MyType'`` was resolved as
``ForwardRef('MyType')`` which is not a real type. And ``hypothesis`` was not able
to generate any meaningful values out of it.

Right here we test different possible outcomes for different Python versions:
- Regular case, when ``'MyType'`` can be imported
- Alias case, when we use type aliases for ``'MyType'``
- ``if TYPE_CHECKING:`` case, when ``'MyType'`` only exists during type checking
  and is not importable at all
- Dot access case, like ``'module.MyType'``
- Missing case, when there's no ``'MyType'`` at all
"""

from typing import ForwardRef, TypeVar

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import ResolutionFailed
from tests.hypothesis_port.common import utils
from tests.hypothesis_port.common.debug import check_can_generate_examples

# The dot-access case names ``utils.StringIO`` rather than Hypothesis's
# ``utils.ExcInfo``: the ported helpers have no ``ExcInfo``, and any type
# reachable through the module will do.

# Correct:

_Correct = TypeVar("_Correct", bound="CustomType")


def correct_fun(thing: _Correct) -> int:
    return thing.arg


class CustomType:
    def __init__(self, arg: int) -> None:
        self.arg = arg


@hegel.test
def test_bound_correct_forward_ref(tc):
    """Correct resolution of existing type codebase."""
    built = tc.draw(gs.builds(correct_fun))
    assert isinstance(built, int)


# Aliases:

_Alias = TypeVar("_Alias ", bound="OurAlias")


def alias_fun(thing: _Alias) -> int:
    return thing.arg


OurAlias = CustomType


@hegel.test
def test_bound_alias_forward_ref(tc):
    """Correct resolution of type aliases."""
    built = tc.draw(gs.builds(alias_fun))
    assert isinstance(built, int)


# Dot access:

_CorrectDotAccess = TypeVar("_CorrectDotAccess", bound="utils.StringIO")
_WrongDotAccess = TypeVar("_WrongDotAccess", bound="wrong.ExcInfo")  # noqa
_MissingDotAccess = TypeVar("_MissingDotAccess", bound="utils.MissingType")


def correct_dot_access_fun(thing: _CorrectDotAccess) -> int:
    return 1


def wrong_dot_access_fun(thing: _WrongDotAccess) -> int:
    return 1


def missing_dot_access_fun(thing: _MissingDotAccess) -> int:
    return 1


@hegel.test
def test_bound_correct_dot_access_forward_ref(tc):
    """Correct resolution of dot access types."""
    built = tc.draw(gs.builds(correct_dot_access_fun))
    assert isinstance(built, int)


@pytest.mark.parametrize("function", [wrong_dot_access_fun, missing_dot_access_fun])
def test_bound_missing_dot_access_forward_ref(function):
    """Resolution of missing type in dot access."""
    with pytest.raises(ResolutionFailed):
        check_can_generate_examples(gs.builds(function))


# Missing:

_Missing = TypeVar("_Missing", bound="MissingType")  # noqa


def missing_fun(thing: _Missing) -> int:
    return 1


def test_bound_missing_forward_ref():
    """We should raise proper errors on missing types."""
    with pytest.raises(ResolutionFailed):
        check_can_generate_examples(gs.builds(missing_fun))


# Type checking only:

_TypeChecking = TypeVar("_TypeChecking", bound="ExcInfo")  # noqa: F821


def typechecking_only_fun(thing: _TypeChecking) -> int:
    return 1


def test_bound_type_cheking_only_forward_ref():
    """We should fallback to registering explicit ``ForwardRef`` when we have to."""
    with utils.temp_registered(ForwardRef("ExcInfo"), gs.just(1)):
        check_can_generate_examples(gs.builds(typechecking_only_fun))


def test_bound_type_checking_only_forward_ref_wrong_type():
    """We should check ``ForwardRef`` parameter name correctly."""
    with (
        utils.temp_registered(ForwardRef("WrongType"), gs.just(1)),
        pytest.raises(ResolutionFailed),
    ):
        check_can_generate_examples(gs.builds(typechecking_only_fun))
