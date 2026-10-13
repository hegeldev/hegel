# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import asyncio
from collections.abc import Callable

import pytest

import hegel
from hegel import generators as gs

_filter_too_much = pytest.mark.xfail(
    strict=True,
    reason="engine: rejecting every input after a draw fails the FilterTooMuch health "
    "check rather than raising Unsatisfiable (docs/engine-issues.md)",
)


@_filter_too_much
def test_exceptiongroup_discard_frozen():
    """Basic test that raises Frozen+Unsatisfiable.
    Frozen is thrown out, and Unsatisfiable is raised"""

    @hegel.test
    def test_function(tc):
        async def task(pred):
            return tc.draw(gs.booleans().filter(pred))

        async def _main():
            async with asyncio.TaskGroup() as tg:
                tg.create_task(task(bool))
                tg.create_task(task(lambda _: False))

        asyncio.run(_main())

    with pytest.raises(hegel.Unsatisfiable):
        test_function()


@_filter_too_much
def test_exceptiongroup_nested() -> None:
    @hegel.test
    def test_function(tc: hegel.TestCase) -> None:
        async def task(pred: Callable[[bool], bool]) -> None:
            tc.draw(gs.booleans().filter(pred))

        async def _main() -> None:
            async with asyncio.TaskGroup(), asyncio.TaskGroup() as tg2:
                tg2.create_task(task(bool))
                tg2.create_task(task(lambda _: False))

        asyncio.run(_main())

    with pytest.raises(hegel.Unsatisfiable):
        test_function()


def test_exceptiongroup_user_originated() -> None:
    @hegel.test
    def test_function(tc):
        raise ExceptionGroup("foo", [ValueError(), ValueError()])

    with pytest.raises(ExceptionGroup) as exc_info:
        test_function()
    e = exc_info.value
    assert e.message == "foo"
    assert isinstance(e, ExceptionGroup)
    assert len(e.exceptions) == 2
    assert all(isinstance(child_e, ValueError) for child_e in e.exceptions)

    @hegel.test
    def test_single_exc_group(tc):
        raise ExceptionGroup("important message for user", [ValueError()])

    with pytest.raises(ExceptionGroup) as exc_info:
        test_single_exc_group()
    e = exc_info.value
    assert e.message == "important message for user"
    assert isinstance(e, ExceptionGroup)
    assert len(e.exceptions) == 1
    assert isinstance(e.exceptions[0], ValueError)


def test_exceptiongroup_multiple_stop() -> None:
    @hegel.test
    def test_function(tc):
        async def task(tc: hegel.TestCase) -> None:
            tc.reject()

        async def _main(tc: hegel.TestCase) -> None:
            async with asyncio.TaskGroup() as tg:
                tg.create_task(task(tc))
                tg.create_task(task(tc))

        asyncio.run(_main(tc))

    # multiple rejections -> single rejection -> unsatisfiable
    with pytest.raises(hegel.Unsatisfiable):
        test_function()


def test_exceptiongroup_stop_and_hypothesisexception() -> None:
    # group with rejection+invalidargument -> invalidargument
    @hegel.test
    def test_function(tc):
        async def task_reject(tc: hegel.TestCase) -> None:
            # only reject some runs to not raise Unsatisfiable
            if tc.draw(gs.integers(min_value=0, max_value=1)) == 1:
                tc.reject()

        async def task_invalid_argument(tc: hegel.TestCase) -> None:
            tc.draw(gs.integers(max_value=2, min_value=3))

        async def _main(tc: hegel.TestCase) -> None:
            async with asyncio.TaskGroup() as tg:
                tg.create_task(task_reject(tc))
                tg.create_task(task_invalid_argument(tc))

        asyncio.run(_main(tc))

    with pytest.raises(hegel.InvalidArgument):
        test_function()


def test_exceptiongroup_multiple_hypothesisexception() -> None:
    # multiple AssumeFailed => first one is reraised => engine suppresses it

    @hegel.test
    def test_function(tc: hegel.TestCase) -> None:
        val = tc.draw(gs.integers(min_value=0, max_value=3))

        async def task(value: int) -> None:
            if value == 0:
                tc.reject()

        async def _main(value: int) -> None:
            async with asyncio.TaskGroup() as tg:
                tg.create_task(task(value))
                tg.create_task(task(value))

        asyncio.run(_main(val))

    test_function()


def test_exceptiongroup_multiple_InvalidArgument() -> None:
    # multiple InvalidArgument => only first one is reraised... which seems bad.
    # But raising a group might break ghostwriter(?)

    @hegel.test
    def test_function(tc: hegel.TestCase) -> None:
        async def task1(tc: hegel.TestCase) -> None:
            tc.draw(gs.integers(max_value=1, min_value=3))

        async def task2(tc: hegel.TestCase) -> None:
            tc.draw(gs.integers(max_value=2, min_value=3))

        async def _main(tc: hegel.TestCase) -> None:
            async with asyncio.TaskGroup() as tg:
                tg.create_task(task1(tc))
                tg.create_task(task2(tc))

        asyncio.run(_main(tc))

    with pytest.raises(hegel.InvalidArgument):
        test_function()
