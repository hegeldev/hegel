import contextlib
import enum
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

from hegel.generators import from_type as _hegel_from_type
from hegel.generators.from_type import _registry as _global_type_lookup
from hegel.hypothesis import Phase, settings
from hegel.hypothesis.errors import HypothesisDeprecationWarning
from hegel.hypothesis.reporting import default, with_reporter
from hegel.hypothesis.strategies import register_type_strategy

from tests.hypothesis_port.common.compat import next_down

time_sleep = time.sleep

skipif_emscripten = mark.skipif(
    sys.platform == "emscripten",
    reason="threads, processes, etc. are not available in the browser",
)

skipif_root = mark.skipif(
    getattr(os, "geteuid", lambda: -1)() == 0,
    reason="root ignores file permissions, so inaccessible paths are readable",
)

no_shrink = tuple(set(settings.default.phases) - {Phase.shrink, Phase.explain})


def proxies(target):
    """Keep the wrapped function's name, docstring and signature (so that
    ``@given`` sees the original parameters)."""

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


@contextlib.contextmanager
def capture_out():
    with capture_out_lock:
        old_out = sys.stdout
        try:
            new_out = StringIO()
            sys.stdout = new_out
            with with_reporter(default):
                yield new_out
        finally:
            sys.stdout = old_out


class ExcInfo:
    pass


def fails_with(e, *, match=None):
    def accepts(f):
        @proxies(f)
        def inverted_test(*arguments, **kwargs):
            with pytest.raises(e, match=match):
                f(*arguments, **kwargs)

        return inverted_test

    return accepts


fails = fails_with(AssertionError)


class NotDeprecated(Exception):
    pass


@contextlib.contextmanager
def validate_deprecation():
    """Hegel emits no deprecation warnings for Hypothesis's deprecated
    behaviours; the block runs with those warnings allowed."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", HypothesisDeprecationWarning)
        yield


def checks_deprecated_behaviour(func):
    """A decorator for testing deprecated behaviour."""

    @proxies(func)
    def _inner(*args, **kwargs):
        with validate_deprecation():
            return func(*args, **kwargs)

    return _inner


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
        assert f"{k} = {v!r}" in output or f"{k}={v!r}" in output, (f"{k}={v!r}", output)


def assert_failing_output(
    test,
    prefix="Falsifying example",
    *,
    expected_exception=AssertionError,
    **kwargs,
):
    with capture_out() as out:
        if expected_exception is None:
            test()
            msg = ""
        else:
            with pytest.raises(expected_exception) as exc_info:
                test()
            notes = "\n".join(getattr(exc_info.value, "__notes__", []))
            msg = str(exc_info.value) + "\n" + notes

    output = out.getvalue() + msg
    assert f"{prefix}:" in output, (prefix, output)
    assert_output_contains_failure(output, test, **kwargs)


temp_registered_lock = RLock()


@contextlib.contextmanager
def temp_registered(type_, strat_or_factory):
    """Register and un-register a type for st.from_type()."""
    with temp_registered_lock:
        prev = _global_type_lookup.get(type_)
        register_type_strategy(type_, strat_or_factory)
        try:
            yield
        finally:
            _global_type_lookup.pop(type_, None)
            if prev is not None:
                register_type_strategy(type_, prev)


@contextlib.contextmanager
def raises_warning(expected_warning, match=None):
    """Use instead of pytest.warns to check that the raised warning is handled properly"""
    with warnings.catch_warnings():
        warnings.simplefilter("error", expected_warning)
        with pytest.raises(expected_warning, match=match):
            yield


# Some Python builds flush subnormal floats to zero.
PYTHON_FTZ = next_down(sys.float_info.min) == 0.0


class Why(enum.Enum):
    # Categorizing known failures, to ease later follow-up investigation.
    symbolic_outside_context = "CrosshairInternal error (using value outside context)"
    nested_given = "nested @given decorators don't work with crosshair"
    undiscovered = "crosshair may not find the failing input"
    other = "reasons not elsewhere categorized"


def xfail_on_crosshair(why: Why, /, *, strict=True, as_marks=False):
    """No symbolic backend on Hegel: the decorator does nothing."""

    def accept(f):
        return f

    if as_marks:
        return ()
    return accept


def skipif_threading(f):
    return f


def xfail_if_gil_disabled(f):
    return f


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


def run_test_for_failing_test_case(test_fn):
    with pytest.raises(AssertionError) as err:
        test_fn()
    return "\n".join(err.value.__notes__).strip()


_ = _hegel_from_type
