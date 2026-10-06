# Dropped from the Hypothesis original: test_encoding_loop, test_decoding_may_fail
# and test_invalid_base_64_gives_invalid_argument (Hypothesis's choice-sequence
# blob encoding; Hegel's reproduction blobs come from the engine and are opaque,
# so the tests below obtain one from a real failure instead of encoding choices).
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import re
import zlib

import pytest

from hegel.hypothesis import (
    Verbosity,
    __version__,
    example,
    given,
    reject,
    reproduce_failure,
    settings,
    strategies as st,
)
from hegel.hypothesis.errors import DidNotReproduce, InvalidArgument

from tests.hypothesis_port.common.utils import Why, capture_out, no_shrink, xfail_on_crosshair

BLOB = re.compile(r"reproduce_failure\('[^']*', (b'[^']*')\)")


def failing_blob(strategy):
    """A reproduction blob for ``strategy``, and the value it reproduces."""
    last = []

    @settings(print_blob=True, database=None)
    @given(strategy)
    def test(x):
        last.append(x)
        raise AssertionError

    with pytest.raises(AssertionError) as err:
        test()
    match = BLOB.search("\n".join(err.value.__notes__))
    assert match is not None, err.value.__notes__
    return eval(match.group(1)), last[-1]


def test_reproduces_the_failure():
    n = 11
    blob, b = failing_blob(st.binary(min_size=n, max_size=n))

    @reproduce_failure(__version__, blob)
    @given(st.binary(min_size=n, max_size=n))
    def test_outer(x):
        assert x != b

    @given(st.binary(min_size=n, max_size=n))
    @reproduce_failure(__version__, blob)
    def test_inner(x):
        assert x != b

    with pytest.raises(AssertionError):
        test_outer()
    with pytest.raises(AssertionError):
        test_inner()


def test_errors_if_provided_example_does_not_reproduce_failure():
    n = 11
    blob, b = failing_blob(st.binary(min_size=n, max_size=n))

    @reproduce_failure(__version__, blob)
    @given(st.binary(min_size=n, max_size=n))
    def test(x):
        assert x == b

    with pytest.raises(DidNotReproduce):
        test()


def test_errors_with_did_not_reproduce_if_the_shape_changes():
    n = 11
    blob, b = failing_blob(st.binary(min_size=n, max_size=n))

    @reproduce_failure(__version__, blob)
    @given(st.binary(min_size=n, max_size=n) | st.integers())
    def test(v):
        assert v == b

    with pytest.raises(DidNotReproduce):
        test()


def test_errors_with_did_not_reproduce_if_rejected():
    n = 11
    blob, b = failing_blob(st.binary(min_size=n, max_size=n))

    @reproduce_failure(__version__, blob)
    @given(st.binary(min_size=n, max_size=n))
    def test(x):
        reject()

    with pytest.raises(DidNotReproduce):
        test()


@xfail_on_crosshair(Why.symbolic_outside_context)
def test_prints_reproduction_if_requested():
    failing_example = None

    @settings(print_blob=True, database=None, max_examples=100)
    @given(st.integers())
    def test(i):
        nonlocal failing_example
        if failing_example is None and i != 0:
            failing_example = i
        assert i != failing_example

    with pytest.raises(AssertionError) as err:
        test()
    notes = "\n".join(err.value.__notes__)
    assert "@reproduce_failure" in notes

    exp = re.compile(r"reproduce_failure\(([^)]+)\)", re.MULTILINE)
    extract = exp.search(notes)
    reproduction = eval(extract.group(0))
    test = reproduction(test)

    with pytest.raises(AssertionError):
        test()


def test_does_not_print_reproduction_for_simple_examples_by_default():
    @settings(print_blob=False)
    @given(st.integers())
    def test(i):
        raise AssertionError

    with capture_out() as o, pytest.raises(AssertionError):
        test()
    assert "@reproduce_failure" not in o.getvalue()


def test_does_not_print_reproduction_for_simple_data_examples_by_default():
    @settings(print_blob=False)
    @given(st.data())
    def test(data):
        data.draw(st.integers())
        raise AssertionError

    with capture_out() as o, pytest.raises(AssertionError):
        test()
    assert "@reproduce_failure" not in o.getvalue()


@xfail_on_crosshair(Why.undiscovered)
def test_does_not_print_reproduction_for_large_data_examples_by_default():
    @settings(phases=no_shrink, print_blob=False)
    @given(st.data())
    def test(data):
        b = data.draw(st.binary(min_size=1000, max_size=1000))
        if len(zlib.compress(b)) > 1000:
            raise ValueError

    with capture_out() as o, pytest.raises(ValueError):
        test()
    assert "@reproduce_failure" not in o.getvalue()


class Foo:
    def __repr__(self):
        return "not a valid python expression"


def test_does_not_print_reproduction_if_told_not_to():
    @settings(print_blob=False)
    @given(st.integers().map(lambda x: Foo()))
    def test(i):
        raise ValueError

    with capture_out() as o, pytest.raises(ValueError):
        test()

    assert "@reproduce_failure" not in o.getvalue()


def test_raises_invalid_if_wrong_version():
    n = 11
    blob, b = failing_blob(st.binary(min_size=n, max_size=n))

    @reproduce_failure("1.0.0", blob)
    @given(st.binary(min_size=n, max_size=n))
    def test(x):
        pass

    with pytest.raises(InvalidArgument):
        test()


def test_does_not_print_reproduction_if_verbosity_set_to_quiet():
    @given(st.data())
    @settings(verbosity=Verbosity.quiet, print_blob=False)
    def test_always_fails(data):
        assert data.draw(st.just(False))

    with capture_out() as out, pytest.raises(AssertionError):
        test_always_fails()

    assert "@reproduce_failure" not in out.getvalue()
