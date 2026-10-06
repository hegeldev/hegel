"""``@given`` and the decorators that go with it, running Hypothesis-style
tests on Hegel."""

from __future__ import annotations

import contextlib
import functools
import gc
import hashlib
import inspect
import re
import threading
import time
import types
import unittest
import warnings
from collections.abc import Callable, Hashable
from typing import Any, TypeVar

from hegel.errors import Flaky, RunError
from hegel.explicit import ExplicitCase
from hegel.hypothesis._settings import HealthCheck, Phase, Verbosity, local_settings
from hegel.hypothesis._settings import settings as Settings
from hegel.hypothesis.control import BuildContext, currently_in_test_context
from hegel.hypothesis.errors import (
    DeadlineExceeded,
    FailedHealthCheck,
    FlakyFailure,
    HypothesisWarning,
    InvalidArgument,
    NoSuchExample,
    Unsatisfiable,
    UnsatisfiedAssumption,
)
from hegel.hypothesis.strategies._base import SearchStrategy, check_strategy
from hegel.runner import (
    RunOutput,
    _database_key_of,
    _location_of,
    _run_explicit,
    _skip_exceptions,
    with_output_override,
)
from hegel.runner import (
    run as hegel_run,
)
from hegel.test_case import TestCase

__all__ = [
    "HypothesisHandle",
    "example",
    "find",
    "given",
    "infer",
    "is_hypothesis_test",
    "reproduce_failure",
    "seed",
]

TestFunc = TypeVar("TestFunc", bound=Callable[..., Any])

#: ``@given(x=infer)`` infers the strategy from the type annotation.
infer: Any = ...


class Example:
    __slots__ = ("args", "kwargs", "raises", "reason", "xfail")

    def __init__(self, args: tuple[Any, ...], kwargs: dict[str, Any]) -> None:
        self.args = args
        self.kwargs = kwargs
        self.xfail = False
        self.raises: Any = BaseException
        self.reason = ""


class example:
    """Add an explicit input, run before any generated ones."""

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        if args and kwargs:
            raise InvalidArgument("Cannot mix positional and keyword arguments for examples")
        if not (args or kwargs):
            raise InvalidArgument("An example must provide at least one argument")
        self.hypothesis_explicit_examples: list[Example] = []
        self._this_example = Example(tuple(args), kwargs)

    def __call__(self, test: TestFunc) -> TestFunc:
        if not hasattr(test, "hypothesis_explicit_examples"):
            test.hypothesis_explicit_examples = self.hypothesis_explicit_examples  # type: ignore[attr-defined]
        test.hypothesis_explicit_examples.append(self._this_example)  # type: ignore[attr-defined]
        return test

    def xfail(
        self,
        condition: bool = True,
        *,
        reason: str = "",
        raises: type[BaseException] | tuple[type[BaseException], ...] = BaseException,
    ) -> example:
        """Mark this example as an expected failure."""
        if not isinstance(condition, bool):
            raise InvalidArgument(f"condition={condition!r} must be a bool")
        if not isinstance(reason, str):
            raise InvalidArgument(f"reason={reason!r} must be a str")
        if not (isinstance(raises, type) and issubclass(raises, BaseException)) and not (
            isinstance(raises, tuple)
            and raises
            and all(isinstance(r, type) and issubclass(r, BaseException) for r in raises)
        ):
            raise InvalidArgument(
                f"raises={raises!r} must be an exception type or tuple of exception types"
            )
        if condition:
            self._this_example.xfail = True
            self._this_example.raises = raises
            self._this_example.reason = reason
        return self

    def via(self, whence: str, /) -> example:
        """Attach a note on where the example came from (kept for
        compatibility; it has no effect here)."""
        if not isinstance(whence, str):
            raise InvalidArgument(".via() must be passed a string")
        return self


def seed(seed: Hashable) -> Callable[[TestFunc], TestFunc]:
    """Fix the seed of generation for this test (and disable the database)."""

    def accept(test: TestFunc) -> TestFunc:
        test._hypothesis_internal_use_seed = seed  # type: ignore[attr-defined]
        current = getattr(test, "_hypothesis_internal_use_settings", None)
        test._hypothesis_internal_use_settings = Settings(current, database=None)  # type: ignore[attr-defined]
        return test

    return accept


def reproduce_failure(version: str, blob: bytes | str) -> Callable[[TestFunc], TestFunc]:
    """Replay the Hegel blob ``blob``; ``version`` is accepted for
    compatibility and must be a string."""
    if not isinstance(version, str):
        raise InvalidArgument(f"version={version!r} must be a string")

    def accept(test: TestFunc) -> TestFunc:
        test._hypothesis_internal_use_reproduce_failure = (version, blob)  # type: ignore[attr-defined]
        return test

    return accept


def is_hypothesis_test(test: object) -> bool:
    """Whether ``test`` is decorated with ``@given``."""
    if hasattr(test, "hypothesis"):
        return getattr(test.hypothesis, "is_hypothesis_test", False)  # type: ignore[attr-defined]
    return getattr(test, "is_hypothesis_test", False)


class HypothesisHandle:
    """``test.hypothesis``: the inner test and friends."""

    def __init__(
        self,
        inner_test: Callable[..., Any],
        given_kwargs: dict[str, Any],
        get_fuzz_target: Callable[[], Callable[..., Any]],
    ) -> None:
        self.inner_test = inner_test
        self._given_kwargs = given_kwargs
        self._get_fuzz_target = get_fuzz_target
        self.is_hypothesis_test = True

    def fuzz_one_input(self, buffer: bytes) -> None:  # nocov
        raise NotImplementedError("fuzz_one_input is not supported on Hegel")


def _seed_to_int(value: Hashable) -> int:
    if isinstance(value, int) and not isinstance(value, bool):
        return value % (1 << 64)
    digest = hashlib.sha256(repr(value).encode("utf-8", "surrogatepass")).digest()
    return int.from_bytes(digest[:8], "little")


def _invalid(message: str, *, exc: type[Exception], test: Any, given_kwargs: Any) -> Any:
    @functools.wraps(test)
    def wrapped_test(*arguments: Any, **kwargs: Any) -> Any:  # nocov
        raise exc(message)

    wrapped_test.is_hypothesis_test = True  # type: ignore[attr-defined]
    wrapped_test.hypothesis = HypothesisHandle(test, given_kwargs, lambda: wrapped_test)  # type: ignore[attr-defined]
    return wrapped_test


def _is_invalid_test(
    test: Any,
    original_sig: inspect.Signature,
    given_arguments: tuple[Any, ...],
    given_kwargs: dict[str, Any],
) -> Any:
    from hegel.hypothesis._settings import all_settings

    invalid = functools.partial(_invalid, exc=InvalidArgument, test=test, given_kwargs=given_kwargs)
    if not (given_arguments or given_kwargs):
        return invalid("given must be called with at least one argument")
    params = list(original_sig.parameters.values())
    pos_params = [p for p in params if p.kind is p.POSITIONAL_OR_KEYWORD]
    kwonly_params = [p for p in params if p.kind is p.KEYWORD_ONLY]
    if given_arguments and params != pos_params:
        return invalid(
            "positional arguments to @given are not supported with varargs, "
            "varkeywords, positional-only, or keyword-only arguments"
        )
    if len(given_arguments) > len(pos_params):
        return invalid(
            f"Too many positional arguments for {test.__name__}() were passed to "
            f"@given - expected at most {len(pos_params)} "
            f"arguments, but got {len(given_arguments)} {given_arguments!r}"
        )
    if ... in given_arguments:
        return invalid(
            "... was passed as a positional argument to @given, but may only be "
            "passed as a keyword argument or as the sole argument of @given"
        )
    if given_arguments and given_kwargs:
        return invalid("cannot mix positional and keyword arguments to @given")
    extra_kwargs = [
        k for k in given_kwargs if k not in {p.name for p in pos_params + kwonly_params}
    ]
    if extra_kwargs and (params == [] or params[-1].kind is not params[-1].VAR_KEYWORD):
        arg = extra_kwargs[0]
        extra = ""
        if arg in all_settings:
            extra = f". Did you mean @settings({arg}={given_kwargs[arg]!r})?"
        return invalid(
            f"{test.__name__}() got an unexpected keyword argument {arg!r}, "
            f"from `{arg}={given_kwargs[arg]!r}` in @given{extra}"
        )
    if any(p.default is not p.empty for p in params):
        return invalid("Cannot apply @given to a function with defaults.")
    empty = [f"{s!r} (arg {idx})" for idx, s in enumerate(given_arguments) if _is_empty(s)] + [
        f"{name}={s!r}" for name, s in given_kwargs.items() if _is_empty(s)
    ]
    if empty:
        strats = "strategies" if len(empty) > 1 else "strategy"
        return invalid(
            f"Cannot generate test cases from empty {strats}: " + ", ".join(empty),
            exc=Unsatisfiable,
        )
    return None


def _explicit_cases(
    wrapped_test: Any, original_sig: inspect.Signature, given_kwargs: dict[str, Any]
) -> list[tuple[Example, ExplicitCase]]:
    posargs = [
        p.name for p in original_sig.parameters.values() if p.kind is p.POSITIONAL_OR_KEYWORD
    ]
    cases = []
    for ex in reversed(getattr(wrapped_test, "hypothesis_explicit_examples", ())):
        if ex.args:
            if any(p.kind is p.POSITIONAL_ONLY for p in original_sig.parameters.values()):
                raise InvalidArgument(
                    "Cannot pass positional arguments to @example() when decorating "
                    "a test function which has positional-only parameters."
                )
            if len(ex.args) > len(posargs):
                raise InvalidArgument(
                    "example has too many arguments for test. Expected at most "
                    f"{len(posargs)} but got {len(ex.args)}"
                )
            example_kwargs = dict(zip(posargs[-len(ex.args) :], ex.args, strict=True))
        else:
            example_kwargs = dict(ex.kwargs)
        given_kws = ", ".join(repr(k) for k in sorted(given_kwargs))
        example_kws = ", ".join(repr(k) for k in sorted(example_kwargs))
        if given_kws != example_kws:
            raise InvalidArgument(
                f"Inconsistent args: @given() got strategies for {given_kws}, "
                f"but @example() got arguments for {example_kws}"
            )
        cases.append((ex, ExplicitCase((), example_kwargs)))
    return cases


def _is_empty(strategy: Any) -> bool:
    return isinstance(strategy, SearchStrategy) and strategy.is_empty


class _XfailPassed(AssertionError):
    pass


_gc_state = {"start": 0.0, "total": 0.0}


def _gc_callback(phase: str, info: dict[str, Any]) -> None:
    now = time.perf_counter()
    if phase == "start":
        _gc_state["start"] = now
    else:
        _gc_state["total"] += now - _gc_state["start"]


if hasattr(gc, "callbacks"):  # CPython
    gc.callbacks.append(_gc_callback)


def _gc_cumulative_time() -> float:
    """Seconds spent in garbage collection so far, excluded from deadlines."""
    return _gc_state["total"]


running_under_pytest = False
global_force_seed: Hashable | None = None


class _Outcomes:
    """The last outcomes of the test body, for explaining a flaky result."""

    def __init__(self) -> None:
        self.last_failure: BaseException | None = None
        self.previous_failure: BaseException | None = None
        self.last_was_failure = False

    def record(self, error: BaseException | None) -> None:
        if error is None:
            self.last_was_failure = False
            return
        self.previous_failure = self.last_failure
        self.last_failure = error
        self.last_was_failure = True


def _flaky_failure(
    error: Flaky,
    name: str,
    outcomes: _Outcomes,
    timing: list[float | None],
    deadline: float | None,
) -> BaseException:
    """The Flaky error from the engine as Hypothesis's FlakyFailure group
    around the failure(s) involved, noting deadline timings."""
    failure = outcomes.last_failure
    if failure is None:
        return error
    if outcomes.last_was_failure and outcomes.previous_failure is not None:
        previous = outcomes.previous_failure
        involved = [previous, failure]
        message = (
            "Inconsistent results from replaying a failing test case!\n"
            f"  last: {type(previous).__name__}\n  this: {type(failure).__name__}"
        )
    else:
        involved = [failure]
        message = (
            f"Hypothesis {name} produces unreliable results: Failed on the first call "
            "but did not on a subsequent one"
        )
    # An exception group cannot hold BaseExceptions such as Hegel's
    # UnsatisfiedAssumption; those are described in a note instead.
    nested = [e for e in involved if isinstance(e, Exception)]
    if not nested:
        return error
    group = FlakyFailure(message, nested)
    for e in involved:
        if not isinstance(e, Exception):
            group.add_note(f"One of the runs raised {type(e).__name__}: {e}")
    group.add_note(str(error))
    if isinstance(failure, DeadlineExceeded) and deadline is not None and timing[1] is not None:
        group.add_note(
            "Unreliable test timing! On an initial run, this test took "
            f"{failure.runtime * 1000:.2f}ms, which exceeded the deadline of "
            f"{deadline * 1000:.2f}ms, but on a subsequent run it took "
            f"{timing[1] * 1000:.2f}ms, which did not. If you expect this sort of "
            "variability in your test timings, consider turning deadlines off for "
            "this test by setting deadline=None."
        )
    return group


def _fail_health_check(test_settings: Settings, message: str, check: HealthCheck) -> None:
    """Raise FailedHealthCheck for ``check`` unless the settings suppress it."""
    __tracebackhide__ = True
    if check in test_settings.suppress_health_check:
        return
    raise FailedHealthCheck(message)


_NOT_SEEN = object()


@contextlib.contextmanager
def _fake_subTest(self: Any, msg: Any = None, **__: Any) -> Any:
    """Replaces unittest's subTest while a @given test runs: per-test-case
    reporting would otherwise show every failing example as a separate
    failing test."""
    warnings.warn(
        "subTest per-test-case reporting interacts badly with Hypothesis "
        "trying hundreds of test cases, so we disable it for the duration of "
        "any test that uses `@given`.",
        HypothesisWarning,
        stacklevel=2,
    )
    yield


def given(*_given_arguments: Any, **_given_kwargs: Any) -> Callable[[TestFunc], TestFunc]:
    """Turn a function taking strategies' values into a Hegel test."""
    if currently_in_test_context():
        _fail_health_check(
            Settings.default,
            "Nesting @given tests results in quadratic generation and shrinking "
            "behavior, and can usually be more cleanly expressed by replacing the "
            "inner function with an st.data() parameter on the outer @given."
            "\n\n"
            "If it is difficult or impossible to refactor this test to remove the "
            "nested @given, you can disable this health check with "
            "@settings(suppress_health_check=[HealthCheck.nested_given]) on the "
            "outer @given. See "
            "https://hypothesis.readthedocs.io/en/latest/reference/api.html"
            "#hypothesis.HealthCheck "
            "for details.",
            HealthCheck.nested_given,
        )

    def run_test_as_given(test: Any) -> Any:
        if inspect.isclass(test):
            raise InvalidArgument("@given cannot be applied to a class")
        given_arguments = tuple(_given_arguments)
        given_kwargs = dict(_given_kwargs)
        original_sig = inspect.signature(test)
        if given_arguments == (Ellipsis,) and not given_kwargs:
            given_kwargs = {
                p.name: Ellipsis
                for p in original_sig.parameters.values()
                if p.kind in (p.POSITIONAL_OR_KEYWORD, p.KEYWORD_ONLY)
            }
            given_arguments = ()
        check_invalid = _is_invalid_test(test, original_sig, given_arguments, given_kwargs)
        if check_invalid is not None:
            return check_invalid
        if given_arguments:
            posargs = [
                p.name
                for p in original_sig.parameters.values()
                if p.kind is p.POSITIONAL_OR_KEYWORD
            ]
            given_kwargs = dict(list(zip(posargs[::-1], given_arguments[::-1], strict=False))[::-1])
        del given_arguments
        new_signature = original_sig.replace(
            parameters=[
                p
                for p in original_sig.parameters.values()
                if not (
                    p.name in given_kwargs and p.kind in (p.POSITIONAL_OR_KEYWORD, p.KEYWORD_ONLY)
                )
            ],
            return_annotation=None,
        )
        if ... in given_kwargs.values():
            from typing import get_type_hints

            from hegel.hypothesis.strategies import from_type

            hints = get_type_hints(test)
            for name in [name for name, value in given_kwargs.items() if value is ...]:
                if name not in hints:
                    return _invalid(
                        f"passed {name}=... for {test.__name__}, but {name} has no type annotation",
                        exc=InvalidArgument,
                        test=test,
                        given_kwargs=given_kwargs,
                    )
                given_kwargs[name] = from_type(hints[name])
        # Draw in the test function's parameter order.
        ordered = [p.name for p in original_sig.parameters.values() if p.name in given_kwargs]
        ordered += [k for k in given_kwargs if k not in ordered]
        given_kwargs = {k: given_kwargs[k] for k in ordered}

        executors_seen = threading.local()

        @functools.wraps(test)
        def wrapped_test(*arguments: Any, **kwargs: Any) -> Any:
            __tracebackhide__ = True
            inner = wrapped_test.hypothesis.inner_test  # type: ignore[attr-defined]
            if getattr(inner, "is_hypothesis_test", False):
                raise InvalidArgument(
                    f"You have applied @given to the test {inner.__name__} more than "
                    "once, which wraps the test several times and is extremely slow. "
                    "A similar effect can be gained by combining the arguments "
                    "of the two calls to given. For example, instead of "
                    "@given(booleans()) @given(integers()), you could write "
                    "@given(booleans(), integers())"
                )
            test_settings: Settings = (
                getattr(wrapped_test, "_hypothesis_internal_use_settings", None) or Settings.default
            )
            for name, strategy in given_kwargs.items():
                check_strategy(strategy, name=name)
                strategy.validate()
            if inspect.iscoroutinefunction(inner):
                raise InvalidArgument(
                    "Hypothesis doesn't know how to run async test functions like "
                    f"{inner.__name__}.  You'll need to write a custom executor, "
                    "or use a library like pytest-asyncio or pytest-trio which can "
                    "handle the translation for you."
                )
            selfy = None
            posnames = [
                p.name
                for p in new_signature.parameters.values()
                if p.kind is p.POSITIONAL_OR_KEYWORD
            ]
            if posnames:
                selfy = kwargs.get(posnames[0], arguments[0] if arguments else None)
            elif arguments:
                selfy = arguments[0]
            # A mock passed as the first argument (e.g. by @patch) is not self,
            # and must not be mistaken for an executor.
            if hasattr(selfy, "hypothesis_internal_is_this_a_mock_check"):
                selfy = None
            if isinstance(selfy, unittest.TestCase) and test.__name__ in dir(unittest.TestCase):
                _fail_health_check(
                    test_settings,
                    f"You have applied @given to the method {test.__name__}, which is "
                    "used by the unittest runner but is not itself a test. "
                    "This is not useful in any way.",
                    HealthCheck.not_a_test_method,
                )
            # Check selfy really is self (not e.g. a mock) before health-checking
            # that the method is always called on the same instance.
            cur_self = selfy if getattr(type(selfy), test.__name__, None) is wrapped_test else None
            prev_self = getattr(executors_seen, "prev_self", _NOT_SEEN)
            if prev_self is _NOT_SEEN:
                executors_seen.prev_self = cur_self
            elif cur_self is not prev_self:
                _fail_health_check(
                    test_settings,
                    f"The method {test.__qualname__} was called from multiple "
                    "different executors. This may lead to flaky tests and "
                    "nonreproducible errors when replaying from database."
                    "\n\n"
                    "Unlike most health checks, HealthCheck.differing_executors "
                    "warns about a correctness issue with your test. We "
                    "therefore recommend fixing the underlying issue, rather "
                    "than suppressing this health check. However, if you are "
                    "confident this health check can be safely disabled, you can "
                    "do so with "
                    "@settings(suppress_health_check=[HealthCheck.differing_executors]). "
                    "See "
                    "https://hypothesis.readthedocs.io/en/latest/reference/api.html"
                    "#hypothesis.HealthCheck "
                    "for details.",
                    HealthCheck.differing_executors,
                )
            _check_fixtures(inner, kwargs, test_settings)
            explicit = _explicit_cases(wrapped_test, original_sig, given_kwargs)
            _run_given(
                inner,
                wrapped_test,
                given_kwargs,
                arguments,
                kwargs,
                selfy,
                test_settings,
                explicit,
            )

        wrapped_test.is_hypothesis_test = True  # type: ignore[attr-defined]
        wrapped_test.hypothesis = HypothesisHandle(test, given_kwargs, lambda: wrapped_test)  # type: ignore[attr-defined]
        wrapped_test.hypothesis_explicit_examples = getattr(
            test, "hypothesis_explicit_examples", []
        )  # type: ignore[attr-defined]
        wrapped_test._hypothesis_internal_use_settings = getattr(
            test, "_hypothesis_internal_use_settings", None
        )  # type: ignore[attr-defined]
        if getattr(test, "_hypothesis_internal_settings_applied", False):
            wrapped_test._hypothesis_internal_settings_applied = True  # type: ignore[attr-defined]
        wrapped_test._hypothesis_internal_use_seed = getattr(
            test, "_hypothesis_internal_use_seed", None
        )  # type: ignore[attr-defined]
        wrapped_test._hypothesis_internal_use_reproduce_failure = getattr(  # type: ignore[attr-defined]
            test, "_hypothesis_internal_use_reproduce_failure", None
        )
        wrapped_test.__signature__ = new_signature  # type: ignore[attr-defined]
        return wrapped_test

    return run_test_as_given


def _check_fixtures(inner: Any, kwargs: dict[str, Any], test_settings: Settings) -> None:
    """The function_scoped_fixture health check: pytest fixtures with
    function scope are set up once per test, not per example."""
    if HealthCheck.function_scoped_fixture in test_settings.suppress_health_check:
        return
    request = kwargs.get("request")
    fixturedef_lookup = getattr(getattr(request, "_fixture_defs", None), "get", None)
    if fixturedef_lookup is None:
        return
    for name in kwargs:
        if name == "request":
            continue
        fixturedef = fixturedef_lookup(name)
        if fixturedef is not None and getattr(fixturedef, "scope", None) == "function":
            raise FailedHealthCheck(
                f"{inner.__name__} uses the {name!r} fixture, which is reset "
                "between function calls but not between test cases generated by "
                "`@given(...)`.  Most uses of fixtures have no effect on a function "
                "scoped fixture, but if the fixture is pytest's `tmp_path` or "
                "similar, you should instead use `tmp_path_factory` or the "
                "equivalent session-scoped fixture.  See the "
                "HealthCheck.function_scoped_fixture documentation for details.\n"
                "See https://hypothesis.readthedocs.io/en/latest/healthchecks.html "
                "for more information about this. If you want to disable just this "
                "health check, add HealthCheck.function_scoped_fixture to the "
                "suppress_health_check settings for this test."
            )


def _executor(runner: Any) -> Callable[[Callable[[], Any]], Any]:
    """How to call the test body: through ``runner.execute_example`` or
    between ``setup_example`` and ``teardown_example`` when the test's
    instance defines them, as in Hypothesis."""
    execute_example = getattr(runner, "execute_example", None)
    if execute_example is not None:
        return execute_example
    if hasattr(runner, "setup_example") or hasattr(runner, "teardown_example"):
        setup = getattr(runner, "setup_example", None) or (lambda: None)
        teardown = getattr(runner, "teardown_example", None) or (lambda token: None)

        def execute(function: Callable[[], Any]) -> Any:
            token = None
            try:
                token = setup()
                return function()
            finally:
                teardown(token)

        return execute
    return lambda function: function()


def _run_given(
    inner: Any,
    wrapped_test: Any,
    given_kwargs: dict[str, Any],
    arguments: tuple[Any, ...],
    kwargs: dict[str, Any],
    selfy: Any,
    test_settings: Settings,
    explicit: list[tuple[Example, ExplicitCase]],
) -> None:
    deadline = test_settings.deadline.total_seconds() if test_settings.deadline else None
    check_return = HealthCheck.return_value not in test_settings.suppress_health_check
    current_example: list[Example | None] = [None]
    print_args = getattr(wrapped_test, "_hypothesis_internal_print_given_args", True)
    executed = [False]
    valid = [False]
    outcomes = _Outcomes()
    timing: list[float | None] = [None, None]
    execute = _executor(selfy)

    def body(tc: TestCase) -> None:
        executed[0] = True
        with BuildContext(tc, is_final=tc.is_final, runner=selfy) as context:
            started: list[float] = []

            def run() -> Any:
                # Drawn under the executor too, so setup_example precedes it.
                drawn = {}
                for name, strategy in given_kwargs.items():
                    try:
                        if print_args:
                            drawn[name] = tc.draw(strategy, name=name)
                        else:
                            drawn[name] = tc.draw_silent(strategy)
                    except Exception as error:
                        error.add_note(f"while generating {name!r} from {strategy!r}")
                        raise
                started.append(time.perf_counter())
                return inner(*arguments, **kwargs, **drawn)

            gc_started = _gc_cumulative_time()
            ex = current_example[0]
            try:
                result = execute(run)
            except UnsatisfiedAssumption as error:
                outcomes.record(error)
                tc.reject()
            except BaseException as error:
                valid[0] = True
                if ex is not None and ex.xfail:
                    if isinstance(error, ex.raises):
                        return
                    raise
                outcomes.record(error)
                raise
            valid[0] = True
            # Time in garbage collection and in data.draw() does not count.
            elapsed = (
                (time.perf_counter() - started[0] if started else 0.0)
                - (_gc_cumulative_time() - gc_started)
                - context.draw_time
            )
            if ex is not None and ex.xfail:
                raise _XfailPassed(
                    f"Expected an exception from @example({_format_example(ex)}), but no "
                    f"exception was raised{': ' + ex.reason if ex.reason else ''}"
                )
            if result is not None and check_return:
                raise FailedHealthCheck(
                    f"Tests run under @given should return None, but {inner.__name__} "
                    f"returned {result!r} instead.\nSee "
                    "https://hypothesis.readthedocs.io/en/latest/healthchecks.html for "
                    "more information about this. If you want to disable just this "
                    "health check, add HealthCheck.return_value to the "
                    "suppress_health_check settings for this test."
                )
            if deadline is not None:
                # Only clear deadline failures count while generating and
                # shrinking; the final replay is held to the deadline itself.
                allowed = deadline if context.is_final else deadline * 1.25
                if elapsed > allowed:
                    error = DeadlineExceeded(elapsed, deadline)
                    timing[0] = elapsed
                    outcomes.record(error)
                    raise error
                if timing[0] is not None:
                    timing[1] = elapsed
            outcomes.record(None)

    native = test_settings.to_hegel()
    seed_value = getattr(wrapped_test, "_hypothesis_internal_use_seed", None)
    if seed_value is None:
        seed_value = global_force_seed
    if seed_value is not None:
        native = native.replace(seed=_seed_to_int(seed_value), database="")
    reproduce = getattr(wrapped_test, "_hypothesis_internal_use_reproduce_failure", None)
    blob = None
    if reproduce is not None:
        from hegel.hypothesis import __version__

        if reproduce[0] != __version__:
            raise InvalidArgument(
                "Attempting to reproduce a failure from a different version of "
                f"Hypothesis. This failure is from {reproduce[0]}, but you are "
                f"currently running {__version__!r}. Please change your Hypothesis "
                "version to work with this example."
            )
        blob = reproduce[1]
        if isinstance(blob, bytes):
            blob = blob.decode("ascii")
    database_key = _database_key_of(inner)
    location = _location_of(inner)
    if Phase.explicit not in test_settings.phases:
        explicit = []

    verbose = test_settings.verbosity >= Verbosity.verbose

    def run_cases() -> None:
        output = RunOutput.resolve()
        failures: list[tuple[str, BaseException]] = []
        for ex, case in explicit:
            current_example[0] = ex
            call = "{}({})".format(
                inner.__name__, ", ".join(f"{k}={v!r}" for k, v in case.kwargs.items())
            )
            if verbose:
                output.line(f"Trying explicit example: {call}")
            try:
                _run_explicit(body, case, output)
            except InvalidArgument as error:
                if "assumption violated" in str(error):
                    continue
                raise
            except (KeyboardInterrupt, SystemExit):
                raise
            except _skip_exceptions():
                raise
            except BaseException as error:
                if isinstance(error, Exception) and any(
                    isinstance(arg, SearchStrategy) for arg in ex.args + tuple(ex.kwargs.values())
                ):
                    new = HypothesisWarning(
                        "The @example() decorator expects to be passed values, but "
                        "you passed strategies instead.  See https://hypothesis."
                        "readthedocs.io/en/latest/reference/api.html#hypothesis"
                        ".example for details."
                    )
                    new.__cause__ = error
                    error = new
                failures.append((call, error))
                if not test_settings.report_multiple_bugs or not isinstance(error, Exception):
                    break
        current_example[0] = None
        if failures:
            _raise_explicit_failures(failures, simplify=not verbose)
        if blob is not None:
            _run_rewriting_reproducer(
                body, native, database_key=database_key, test_location=location, reproduce_blob=blob
            )
            return
        if not any(p in test_settings.phases for p in (Phase.reuse, Phase.generate)):
            return
        _run_rewriting_reproducer(body, native, database_key=database_key, test_location=location)

    try:
        with local_settings(test_settings), _patched_subtest(selfy):
            if test_settings.verbosity == Verbosity.quiet:
                with with_output_override(lambda line: None):
                    run_cases()
            else:
                run_cases()
    except RunError as error:
        if isinstance(error, FailedHealthCheck) and "across the full search space" in str(error):
            raise Unsatisfiable(
                f"Unable to satisfy assumptions of {inner.__name__}. Hypothesis tried "
                "every possible input, and all of them failed a .filter() or assume()"
            ) from None
        if isinstance(error, Flaky):
            raise _flaky_failure(error, inner.__name__, outcomes, timing, deadline) from error
        if isinstance(error, FailedHealthCheck):
            raise
        if str(error).startswith("Unsatisfiable"):
            raise Unsatisfiable(str(error)) from None
        raise
    if not executed[0]:
        raise unittest.SkipTest("Hypothesis has been told to run no examples for this test.")
    if not valid[0]:
        raise Unsatisfiable(
            f"Unable to satisfy assumptions of {inner.__name__}. Every test case failed a "
            ".filter() or assume() (or none was generated)."
        )


_HEGEL_REPRODUCER = re.compile(r'@hegel\.reproduce_failure\("([^"]*)"\)')


def _run_rewriting_reproducer(*args: Any, **kwargs: Any) -> None:
    """Run the engine, rewriting the reproducer note on a failure into the
    Hypothesis form ``@reproduce_failure(version, blob)``."""
    try:
        hegel_run(*args, **kwargs)
    except BaseException as error:
        notes = getattr(error, "__notes__", None)
        if notes:
            from hegel.hypothesis import __version__

            def rewrite(match: re.Match[str]) -> str:
                return f"@reproduce_failure({__version__!r}, {match.group(1).encode('ascii')!r})"

            error.__notes__ = [
                _HEGEL_REPRODUCER.sub(rewrite, note).replace("above @hegel.test", "above @given")
                for note in notes
            ]
        raise


@contextlib.contextmanager
def _patched_subtest(runner: Any) -> Any:
    if isinstance(runner, unittest.TestCase) and hasattr(runner, "subTest"):
        original = runner.subTest
        runner.subTest = types.MethodType(_fake_subTest, runner)  # type: ignore[method-assign]
        try:
            yield
        finally:
            runner.subTest = original  # type: ignore[method-assign]
    else:
        yield


def _raise_explicit_failures(failures: list[tuple[str, BaseException]], *, simplify: bool) -> None:
    """Raise the explicit examples' failures: grouped by origin (keeping the
    simplest with a note counting the others) unless ``simplify`` is off,
    and as an exception group when more than one remains."""
    groups: dict[Any, list[tuple[str, BaseException]]] = {}
    for call, error in failures:
        key = (type(error), _origin_of(error)) if simplify else id(error)
        groups.setdefault(key, []).append((call, error))
    chosen: list[BaseException] = []
    for group in groups.values():
        group.sort(key=lambda item: (len(item[0]), item[0]))
        error = group[0][1]
        if len(group) > 1:
            other_count = len(group) - 1
            error.add_note(
                f"(note: {other_count} other explicit example{'s' * (other_count > 1)} "
                "also failed with this error; use Verbosity.verbose to view)"
            )
        chosen.append(error)
    if len(chosen) == 1:
        raise chosen[0]
    raise BaseExceptionGroup(f"Hypothesis found {len(chosen)} distinct failures.", chosen)


def _origin_of(error: BaseException) -> tuple[str, int] | None:
    tb = error.__traceback__
    last = None
    while tb is not None:
        last = (tb.tb_frame.f_code.co_filename, tb.tb_lineno)
        tb = tb.tb_next
    return last


def _format_example(ex: Example) -> str:
    parts = [repr(a) for a in ex.args] + [f"{k}={v!r}" for k, v in ex.kwargs.items()]
    return ", ".join(parts)


def find(
    specifier: SearchStrategy[Any],
    condition: Callable[[Any], bool],
    *,
    settings: Settings | None = None,
    random: Any = None,
    database_key: bytes | None = None,
) -> Any:
    """A minimal example from ``specifier`` satisfying ``condition``.

    Raises ``NoSuchExample`` when no generated value satisfies it, and
    ``Unsatisfiable`` when the condition rejects too many values."""
    if settings is None:
        settings = Settings(max_examples=2000)
    check_strategy(specifier, name="specifier")
    if not callable(condition):
        raise InvalidArgument(f"Expected callable but got condition={condition!r}")
    settings = Settings(
        settings,
        database=None,
        deadline=None,
        suppress_health_check=[h for h in HealthCheck if h is not HealthCheck.filter_too_much],
        report_multiple_bugs=False,
        phases=[p for p in settings.phases if p in (Phase.generate, Phase.shrink)]
        or [Phase.generate],
    )
    found: list[Any] = []
    user_rejected = [False]

    def body(tc: TestCase) -> None:
        with BuildContext(tc, is_final=tc.is_final):
            value = tc.draw(specifier, name="Example")
            try:
                satisfied = condition(value)
            except UnsatisfiedAssumption:
                user_rejected[0] = True
                tc.reject()
            if satisfied:
                found.append(value)
                raise _Found()

    native = settings.to_hegel()
    if random is not None:
        native = native.replace(seed=random.getrandbits(64), database="")
    try:
        if settings.verbosity >= Verbosity.verbose:
            hegel_run(body, native, database_key=None)
        else:
            with with_output_override(lambda line: None):
                hegel_run(body, native, database_key=None)
    except _Found:
        return found[-1]
    except FailedHealthCheck as error:
        if user_rejected[0]:
            raise Unsatisfiable(
                f"Unable to satisfy assumptions of {getattr(condition, '__name__', condition)}"
            ) from error
    except RunError as error:
        if user_rejected[0]:
            raise Unsatisfiable(str(error)) from error
        if "Unsatisfiable" not in str(error):
            raise
    raise NoSuchExample(condition)


class _Found(BaseException):
    pass
