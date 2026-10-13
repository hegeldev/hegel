# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_encoding_loop, test_decoding_may_fail,
# test_invalid_base_64_gives_invalid_argument (Hypothesis's choice-sequence blob
# encoding; Hegel's blobs come from the engine and are opaque, so the tests below
# obtain one from a real failure instead); test_raises_invalid_if_wrong_version
# (reproduce_failure takes no version).

import re
import zlib

import pytest

import hegel
from hegel import DidNotReproduce, Verbosity
from hegel import generators as gs
from tests.hypothesis_port.common.utils import capture_out, no_shrink, notes

BLOB = re.compile(r'reproduce_failure\("([^"]*)"\)')


def failing_blob(generator):
    """A reproduction blob for ``generator``, and the value it reproduces."""
    last = []

    @hegel.test(print_blob=True, database="")
    def test(tc):
        x = tc.draw(generator)
        last.append(x)
        raise AssertionError

    with pytest.raises(AssertionError) as err:
        test()
    match = BLOB.search("\n".join(notes(err.value)))
    assert match is not None, notes(err.value)
    return match.group(1), last[-1]


def test_reproduces_the_failure():
    n = 11
    blob, b = failing_blob(gs.binary(min_size=n, max_size=n))

    @hegel.reproduce_failure(blob)
    @hegel.test
    def test_outer(tc):
        x = tc.draw(gs.binary(min_size=n, max_size=n))
        assert x != b

    @hegel.test
    @hegel.reproduce_failure(blob)
    def test_inner(tc):
        x = tc.draw(gs.binary(min_size=n, max_size=n))
        assert x != b

    with pytest.raises(AssertionError):
        test_outer()
    with pytest.raises(AssertionError):
        test_inner()


def test_errors_if_provided_example_does_not_reproduce_failure():
    n = 11
    blob, b = failing_blob(gs.binary(min_size=n, max_size=n))

    @hegel.reproduce_failure(blob)
    @hegel.test
    def test(tc):
        x = tc.draw(gs.binary(min_size=n, max_size=n))
        assert x == b

    with pytest.raises(DidNotReproduce):
        test()


@pytest.mark.xfail(
    strict=True,
    reason="Hegel replays a blob with fresh draws past a divergence, so a changed "
    "generator shape that still fails is reported as the failure (an integer from the "
    "new branch), not as DidNotReproduce",
)
def test_errors_with_did_not_reproduce_if_the_shape_changes():
    n = 11
    blob, b = failing_blob(gs.binary(min_size=n, max_size=n))

    @hegel.reproduce_failure(blob)
    @hegel.test
    def test(tc):
        v = tc.draw(gs.binary(min_size=n, max_size=n) | gs.integers())
        assert v == b

    with pytest.raises(DidNotReproduce):
        test()


def test_errors_with_did_not_reproduce_if_rejected():
    n = 11
    blob, b = failing_blob(gs.binary(min_size=n, max_size=n))

    @hegel.reproduce_failure(blob)
    @hegel.test
    def test(tc):
        tc.draw(gs.binary(min_size=n, max_size=n))
        tc.reject()

    with pytest.raises(DidNotReproduce):
        test()


def test_prints_reproduction_if_requested():
    failing_example = None

    @hegel.test(print_blob=True, database="", test_cases=100)
    def test(tc):
        nonlocal failing_example
        i = tc.draw(gs.integers())
        if failing_example is None and i != 0:
            failing_example = i
        assert i != failing_example

    with pytest.raises(AssertionError) as err:
        test()
    report = "\n".join(notes(err.value))
    assert "@hegel.reproduce_failure" in report

    extract = BLOB.search(report)
    test = hegel.reproduce_failure(extract.group(1))(test)

    with pytest.raises(AssertionError):
        test()


def _report(out, err):
    """Everything a failure printed, and the notes it carries."""
    return out.getvalue() + "\n".join(notes(err.value))


def test_does_not_print_reproduction_for_simple_examples_by_default():
    @hegel.test(print_blob=False)
    def test(tc):
        tc.draw(gs.integers())
        raise AssertionError

    with capture_out() as o, pytest.raises(AssertionError) as err:
        test()
    assert "@hegel.reproduce_failure" not in _report(o, err)


def test_does_not_print_reproduction_for_simple_data_examples_by_default():
    @hegel.test(print_blob=False)
    def test(tc):
        tc.draw(gs.integers(), name="data")
        raise AssertionError

    with capture_out() as o, pytest.raises(AssertionError) as err:
        test()
    assert "@hegel.reproduce_failure" not in _report(o, err)


def test_does_not_print_reproduction_for_large_data_examples_by_default():
    @hegel.test(phases=no_shrink, print_blob=False)
    def test(tc):
        b = tc.draw(gs.binary(min_size=1000, max_size=1000))
        if len(zlib.compress(b)) > 1000:
            raise ValueError

    with capture_out() as o, pytest.raises(ValueError) as err:
        test()
    assert "@hegel.reproduce_failure" not in _report(o, err)


class Foo:
    def __repr__(self):
        return "not a valid python expression"


def test_does_not_print_reproduction_if_told_not_to():
    @hegel.test(print_blob=False)
    def test(tc):
        tc.draw(gs.integers().map(lambda x: Foo()))
        raise ValueError

    with capture_out() as o, pytest.raises(ValueError) as err:
        test()

    assert "@hegel.reproduce_failure" not in _report(o, err)


def test_does_not_print_reproduction_if_verbosity_set_to_quiet():
    @hegel.test(verbosity=Verbosity.quiet, print_blob=False)
    def test_always_fails(tc):
        assert tc.draw(gs.just(False))

    with capture_out() as out, pytest.raises(AssertionError) as err:
        test_always_fails()

    assert "@hegel.reproduce_failure" not in _report(out, err)
