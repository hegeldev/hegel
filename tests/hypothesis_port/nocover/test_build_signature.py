# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

from inspect import signature
from typing import get_type_hints

import hegel
from hegel import generators as gs
from tests.hypothesis_port.common.debug import find_any


def use_this_signature(self, a: int, b: list | None = None, *, x: float, y: str):
    pass


class Model:
    # Emulates the implementation of Pydantic models.  See e.g.
    # https://github.com/timothycrosley/hypothesis-auto/issues/10
    __annotations__ = get_type_hints(use_this_signature)
    __signature__ = signature(use_this_signature)

    def __init__(self, **kwargs):
        # Check that we're being called with the expected arguments
        assert set(kwargs) == {"a", "x", "y"}
        assert isinstance(kwargs["a"], int)
        assert isinstance(kwargs["x"], float)
        assert isinstance(kwargs["y"], str)


@hegel.test
def test_builds_uses_signature_attribute(tc):
    val = tc.draw(gs.builds(Model))
    assert isinstance(val, Model)


class ModelForFromType(Model):
    def __init__(self, **kwargs):
        assert set(kwargs) == {"a", "b", "x", "y"}
        self.b = kwargs["b"]
        assert self.b is None or isinstance(self.b, list)


@hegel.test
def test_from_type_uses_signature_attribute(tc):
    val = tc.draw(gs.from_type(ModelForFromType))
    assert isinstance(val, ModelForFromType)


def test_from_type_can_be_default_or_annotation():
    find_any(gs.from_type(ModelForFromType), lambda m: m.b is None)
    find_any(gs.from_type(ModelForFromType), lambda m: isinstance(m.b, list))


def use_annotations(self, test_a: int, test_b: str | None = None, *, test_x: float, test_y: str):
    pass


def use_signature(self, testA: int, testB: str | None = None, *, testX: float, testY: list[str]):
    pass


class ModelWithAlias:
    __annotations__ = get_type_hints(use_annotations)
    __signature__ = signature(use_signature)

    def __init__(self, **kwargs):
        # Check that we're being called with the expected arguments
        assert set(kwargs) == {"testA", "testX", "testY"}
        assert isinstance(kwargs["testA"], int)
        assert isinstance(kwargs["testX"], float)
        assert isinstance(kwargs["testY"], list)
        assert all(isinstance(elem, str) for elem in kwargs["testY"])


@hegel.test
def test_build_using_different_signature_and_annotations(tc):
    val = tc.draw(gs.builds(ModelWithAlias))
    assert isinstance(val, ModelWithAlias)


def use_bad_signature(self, testA: 1, *, testX: float):
    pass


class ModelWithBadAliasSignature:
    __annotations__ = get_type_hints(use_annotations)
    __signature__ = signature(use_bad_signature)

    def __init__(self, **kwargs):
        assert set(kwargs) == {"testX"}
        assert isinstance(kwargs["testX"], float)


@hegel.test
def test_build_with_non_types_in_signature(tc):
    val = tc.draw(gs.builds(ModelWithBadAliasSignature))
    assert isinstance(val, ModelWithBadAliasSignature)


class UnconventionalSignature:
    def __init__(x: int = 0, self: bool = True):  # noqa
        assert not isinstance(x, int)
        x.self = self


def test_build_in_from_type_with_self_named_something_else():
    find_any(gs.from_type(UnconventionalSignature), lambda x: x.self is True)
    find_any(gs.from_type(UnconventionalSignature), lambda x: x.self is False)
