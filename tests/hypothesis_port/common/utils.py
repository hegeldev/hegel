import contextlib
import math
import os
import sys
import time
import warnings
from functools import wraps
from io import StringIO
from threading import Barrier, Lock, RLock, Thread

import pytest
from pytest import mark

import hegel
from hegel.generators.from_type import _registry as _global_type_lookup
from hegel.generators.from_type import register_type_generator
from hegel.runner import with_output_override
from tests.helpers import notes
from tests.hypothesis_port.common.compat import next_down

__all__ = [
    "PYTHON_FTZ",
    "assert_failing_output",
    "assert_output_contains_failure",
    "capture_out",
    "counts_calls",
    "fails",
    "fails_with",
    "flaky",
    "no_shrink",
    "notes",
    "proxies",
    "raises_warning",
    "restore_recursion_limit",
    "run_concurrently",
    "run_test_for_failing_test_case",
    "skipif_emscripten",
    "skipif_root",
    "temp_registered",
    "wait_for",
]

time_sleep = time.sleep

skipif_emscripten = mark.skipif(
    sys.platform == "emscripten",
    reason="threads, processes, etc. are not available in the browser",
)

skipif_root = mark.skipif(
    getattr(os, "geteuid", lambda: -1)() == 0,
    reason="root ignores file permissions, so inaccessible paths are readable",
)

# Every phase but shrinking, for a run that only needs to find a failure.
no_shrink = frozenset(hegel.Phase) - {hegel.Phase.shrink}


def proxies(target):
    """Keep the wrapped function's name, docstring and signature."""

    def accept(proxy):
        proxy = wraps(target)(proxy)
        try:
            import inspect

            proxy.__signature__ = inspect.signature(target)
        except (TypeError, ValueError):  # pragma: no cover
            pass
        return proxy

    return accept


def flaky(max_runs, min_passes):
    assert isinstance(max_runs, int)
    assert isinstance(min_passes, int)
    assert 0 < min_passes <= max_runs <= 50  # arbitrary cap

    def accept(func):
        @proxies(func)
        def inner(*args, **kwargs):
            runs = passes = 0
            while passes < min_passes:
                runs += 1
                try:
                    func(*args, **kwargs)
                    passes += 1
                except BaseException:
                    if runs >= max_runs:
                        raise

        return inner

    return accept


capture_out_lock = Lock()


class _Captured(StringIO):
    """Everything Hegel prints during the block (its falsifying example,
    verbose trace and so on), together with anything written to stdout."""

    def __call__(self, line: str) -> None:
        self.write(line + "\n")


@contextlib.contextmanager
def capture_out():
    with capture_out_lock:
        old_out = sys.stdout
        try:
            new_out = _Captured()
            sys.stdout = new_out
            with with_output_override(new_out):
                yield new_out
        finally:
            sys.stdout = old_out


def fails_with(e, *, match=None):
    def accepts(f):
        @proxies(f)
        def inverted_test(*arguments, **kwargs):
            with pytest.raises(e, match=match):
                f(*arguments, **kwargs)

        return inverted_test

    return accepts


fails = fails_with(AssertionError)


def counts_calls(func):
    """Count how many times a function was called, in its ``.calls``."""
    assert not hasattr(func, "calls")

    @proxies(func)
    def _inner(*args, **kwargs):
        _inner.calls += 1
        return func(*args, **kwargs)

    _inner.calls = 0
    return _inner


def assert_output_contains_failure(output, test, **kwargs):
    for k, v in kwargs.items():
        assert f"{k} = {v!r}" in output, (f"{k} = {v!r}", output)


def assert_failing_output(
    test,
    prefix="Falsifying example",
    *,
    expected_exception=AssertionError,
    **kwargs,
):
    """Run ``test`` (a Hegel test) expecting it to fail, and check that the
    failure output names each ``kwargs`` draw as ``name = value``."""
    with capture_out() as out:
        if expected_exception is None:
            test()
            msg = ""
        else:
            with pytest.raises(expected_exception) as exc_info:
                test()
            msg = str(exc_info.value) + "\n" + "\n".join(notes(exc_info.value))

    output = out.getvalue() + msg
    assert f"{prefix}:" in output, (prefix, output)
    assert_output_contains_failure(output, test, **kwargs)


temp_registered_lock = RLock()


@contextlib.contextmanager
def temp_registered(type_, generator_or_factory):
    """Register and un-register a type for gs.from_type()."""
    with temp_registered_lock:
        prev = _global_type_lookup.get(type_)
        register_type_generator(type_, generator_or_factory)
        try:
            yield
        finally:
            _global_type_lookup.pop(type_, None)
            if prev is not None:
                register_type_generator(type_, prev)


@contextlib.contextmanager
def raises_warning(expected_warning, match=None):
    """Use instead of pytest.warns to check that the raised warning is handled properly"""
    with warnings.catch_warnings():
        warnings.simplefilter("error", expected_warning)
        with pytest.raises(expected_warning, match=match):
            yield


# Some Python builds flush subnormal floats to zero.
PYTHON_FTZ = next_down(sys.float_info.min) == 0.0


_restore_recursion_limit_lock = RLock()


@contextlib.contextmanager
def restore_recursion_limit():
    with _restore_recursion_limit_lock:
        original_limit = sys.getrecursionlimit()
        try:
            yield
        finally:
            sys.setrecursionlimit(original_limit)


def run_concurrently(function, *, n: int) -> None:
    def run():
        barrier.wait()
        function()

    threads = [Thread(target=run) for _ in range(n)]
    barrier = Barrier(n)

    for thread in threads:
        thread.start()

    for thread in threads:
        thread.join(timeout=10)


def wait_for(condition, *, timeout=1, interval=0.01):
    for _ in range(math.ceil(timeout / interval)):
        if condition():
            return
        time_sleep(interval)
    raise Exception(f"timing out after waiting {timeout}s for condition {condition!r}")


def run_test_for_failing_test_case(test_fn, exception=AssertionError):
    """Run ``test_fn`` expecting it to fail; the text of its failure notes
    (the falsifying example and any caveat)."""
    with pytest.raises(exception) as err:
        test_fn()
    return "\n".join(notes(err.value)).strip()
