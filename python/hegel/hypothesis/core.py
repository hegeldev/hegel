"""``@given`` and the decorators that go with it, running Hypothesis-style
tests on Hegel."""

from __future__ import annotations

import functools
import hashlib
import inspect
import time
from collections.abc import Callable, Hashable
from typing import Any, TypeVar

from hegel.errors import HegelError, RunError
from hegel.explicit import ExplicitCase
from hegel.hypothesis._settings import HealthCheck, Phase, Verbosity
from hegel.hypothesis._settings import settings as Settings
from hegel.hypothesis.control import BuildContext
from hegel.hypothesis.errors import (
    DeadlineExceeded,
    FailedHealthCheck,
    Flaky,
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
    from hegel.hypothesis.strategies._base import NOTHING

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
    empty = [f"{s!r} (arg {idx})" for idx, s in enumerate(given_arguments) if s is NOTHING] + [
        f"{name}={s!r}" for name, s in given_kwargs.items() if s is NOTHING
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


class _XfailPassed(AssertionError):
    pass


def given(*_given_arguments: Any, **_given_kwargs: Any) -> Callable[[TestFunc], TestFunc]:
    """Turn a function taking strategies' values into a Hegel test."""

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

    def body(tc: TestCase) -> None:
        with BuildContext(tc, is_final=tc.is_final, runner=selfy) as context:
            if print_args:
                drawn = {
                    name: tc.draw(strategy, name=name) for name, strategy in given_kwargs.items()
                }
            else:
                drawn = {name: tc.draw_silent(strategy) for name, strategy in given_kwargs.items()}
            started = time.perf_counter()
            ex = current_example[0]
            try:
                result = inner(*arguments, **kwargs, **drawn)
            except UnsatisfiedAssumption:
                tc.reject()
            except BaseException as error:
                if ex is not None and ex.xfail:
                    if isinstance(error, ex.raises):
                        return
                    raise
                raise
            elapsed = time.perf_counter() - started
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
                allowed = deadline * (1.25 if context.is_final else 1.0)
                if elapsed > allowed:
                    raise DeadlineExceeded(elapsed, deadline)

    native = test_settings.to_hegel()
    seed_value = getattr(wrapped_test, "_hypothesis_internal_use_seed", None)
    if seed_value is not None:
        native = native.replace(seed=_seed_to_int(seed_value), database="")
    reproduce = getattr(wrapped_test, "_hypothesis_internal_use_reproduce_failure", None)
    blob = None
    if reproduce is not None:
        blob = reproduce[1]
        if isinstance(blob, bytes):
            blob = blob.decode("ascii")
    database_key = _database_key_of(inner)
    location = _location_of(inner)
    if Phase.explicit not in test_settings.phases:
        explicit = []

    def run_cases() -> None:
        output = RunOutput.resolve()
        for ex, case in explicit:
            current_example[0] = ex
            try:
                _run_explicit(body, case, output)
            except InvalidArgument as error:
                if "assumption violated" in str(error):
                    continue
                raise
        current_example[0] = None
        if blob is not None:
            hegel_run(
                body, native, database_key=database_key, test_location=location, reproduce_blob=blob
            )
            return
        if not any(p in test_settings.phases for p in (Phase.reuse, Phase.generate)):
            return
        hegel_run(body, native, database_key=database_key, test_location=location)

    try:
        if test_settings.verbosity == Verbosity.quiet:
            with with_output_override(lambda line: None):
                run_cases()
        else:
            run_cases()
    except RunError as error:
        if isinstance(error, (FailedHealthCheck, Flaky)):
            raise
        if str(error).startswith("Unsatisfiable"):
            raise Unsatisfiable(str(error)) from None
        raise


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
    """A minimal example from ``specifier`` satisfying ``condition``."""
    if settings is None:
        settings = Settings(max_examples=2000)
    check_strategy(specifier, name="specifier")
    if not callable(condition):
        raise InvalidArgument(f"Expected callable but got condition={condition!r}")
    from hegel.runner import find_example

    try:
        with with_output_override(lambda line: None):
            return find_example(specifier, condition)
    except HegelError as error:
        if "Could not find" in str(error):
            raise NoSuchExample(condition) from None
        raise
