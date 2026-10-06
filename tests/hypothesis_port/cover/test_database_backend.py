# Dropped from the Hypothesis original: every test of the ExampleDatabase
# save/fetch/delete/move/listen API, GitHubArtifactDatabase, MultiplexedDatabase,
# BackgroundWriteDatabase, choice serialisation (choices_to_bytes, uleb128) and
# the configuration home-directory logic. Hegel's database objects only name the
# directory the engine stores examples in, so only settings(database=...) behaviour
# is covered here.
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

from hegel.hypothesis import given, settings, strategies as st
from hegel.hypothesis.database import (
    DirectoryBasedExampleDatabase,
    ExampleDatabase,
    InMemoryExampleDatabase,
    ReadOnlyDatabase,
)
from hegel.hypothesis.errors import HypothesisDeprecationWarning


def failing_test(database, **kwargs):
    @settings(database=database, **kwargs)
    @given(st.integers())
    def test(i):
        raise AssertionError

    return test


@pytest.mark.xfail(
    strict=True, reason="Hegel's ExampleDatabase() is a bare base class, not an in-memory database"
)
def test_default_database_is_in_memory():
    with pytest.warns(HypothesisDeprecationWarning):
        assert isinstance(ExampleDatabase(), InMemoryExampleDatabase)


@pytest.mark.xfail(
    strict=True, reason="Hegel's ExampleDatabase does not take a path to become directory-based"
)
def test_default_on_disk_database_is_dir(tmp_path):
    with pytest.warns(HypothesisDeprecationWarning):
        assert isinstance(ExampleDatabase(tmp_path.joinpath("foo")), DirectoryBasedExampleDatabase)


def test_does_not_error_when_fetching_when_not_exist(tmp_path):
    db = DirectoryBasedExampleDatabase(tmp_path / "examples")

    @settings(database=db)
    @given(st.integers())
    def test(i):
        pass

    test()


@pytest.fixture(scope="function", params=["memory", "directory"])
def exampledatabase(request, tmp_path):
    if request.param == "memory":
        return InMemoryExampleDatabase()
    assert request.param == "directory"
    return DirectoryBasedExampleDatabase(tmp_path / "examples")


def test_a_fresh_database_is_empty(exampledatabase):
    assert exampledatabase.data == {}


def test_failing_examples_are_saved(exampledatabase):
    with pytest.raises(AssertionError):
        failing_test(exampledatabase)()
    assert exampledatabase.data


def test_class_name_is_in_repr(exampledatabase):
    assert type(exampledatabase).__name__ in repr(exampledatabase)


def test_two_directory_databases_can_interact(tmp_path):
    seen = []

    def make(db):
        @settings(database=db, max_examples=200)
        @given(st.integers())
        def test(i):
            seen.append(i)
            assert i < 1000

        return test

    with pytest.raises(AssertionError):
        make(DirectoryBasedExampleDatabase(tmp_path))()
    assert seen[0] != 1000

    seen.clear()
    with pytest.raises(AssertionError):
        make(DirectoryBasedExampleDatabase(tmp_path))()
    # The failure saved by the first database is replayed first by the second.
    assert seen[0] == 1000


@pytest.mark.xfail(
    strict=True,
    reason="Hegel's ReadOnlyDatabase shares the wrapped directory and does not block writes",
)
def test_readonly_db_is_not_writable(tmp_path):
    inner = DirectoryBasedExampleDatabase(tmp_path / "examples")
    wrapped = ReadOnlyDatabase(inner)
    with pytest.raises(AssertionError):
        failing_test(wrapped)()
    assert not inner.data


def test_readonly_db_replays_from_the_wrapped_database(tmp_path):
    inner = DirectoryBasedExampleDatabase(tmp_path / "examples")
    seen = []

    def make(db):
        @settings(database=db, max_examples=200)
        @given(st.integers())
        def test(i):
            seen.append(i)
            assert i < 1000

        return test

    with pytest.raises(AssertionError):
        make(inner)()
    seen.clear()
    with pytest.raises(AssertionError):
        make(ReadOnlyDatabase(inner))()
    assert seen[0] == 1000


@pytest.mark.parametrize(
    "db1, db2",
    [
        (
            DirectoryBasedExampleDatabase("foo"),
            DirectoryBasedExampleDatabase("foo"),
        ),
        pytest.param(
            ReadOnlyDatabase(DirectoryBasedExampleDatabase("foo")),
            ReadOnlyDatabase(DirectoryBasedExampleDatabase("foo")),
            marks=pytest.mark.xfail(
                strict=True, reason="Hegel's ReadOnlyDatabase does not define __eq__"
            ),
        ),
    ],
)
def test_database_equal(db1, db2):
    assert db1 == db2


@pytest.mark.parametrize(
    "db1, db2",
    [
        (DirectoryBasedExampleDatabase("foo"), DirectoryBasedExampleDatabase("bar")),
        (DirectoryBasedExampleDatabase("foo"), InMemoryExampleDatabase()),
        (InMemoryExampleDatabase(), InMemoryExampleDatabase()),
    ],
)
def test_database_not_equal(db1, db2):
    assert db1 != db2
