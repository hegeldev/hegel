from __future__ import annotations

import inspect
import re
import sys
import threading

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import LoopDone
from hegel.runner import (
    RunOutput,
    _location_of,
    _origin,
    _user_location,
    with_output_override,
)
from hegel.settings import NondeterminismStrictness, Phase, Verbosity
from tests.helpers import failing, find_example, notes, run_test


def test_passing_test_runs_configured_number_of_cases():
    calls = []

    @hegel.test(test_cases=12, print_blob=False)
    def test_it(tc):
        calls.append(tc.draw(gs.integers()))

    test_it()
    assert len(calls) == 12
    assert test_it.is_hegel_test
    assert test_it.hegel_inner_test is not test_it
    assert list(inspect.signature(test_it).parameters) == []


def test_decorator_forms_and_validation():
    @hegel.test
    def plain(tc):
        pass

    plain()

    @hegel.test(settings=hegel.Settings(test_cases=2), seed=1)
    def with_settings(tc):
        pass

    with_settings()
    with pytest.raises(hegel.InvalidArgument):
        hegel.test(lambda tc: None, test_cases=1)
    with pytest.raises(hegel.InvalidArgument):
        hegel.test(lambda: None)
    with pytest.raises(hegel.InvalidArgument):
        hegel.test(3)  # type: ignore[arg-type]
    with pytest.raises(hegel.InvalidArgument):
        hegel.run(3)  # type: ignore[arg-type]


def test_extra_arguments_pass_through_like_fixtures():
    seen = []

    @hegel.test(test_cases=3)
    def test_it(tc, fixture, *, named):
        seen.append((fixture, named, tc.draw(gs.booleans())))

    assert list(inspect.signature(test_it).parameters) == ["fixture", "named"]
    test_it("f", named="n")
    assert len(seen) == 3 and all(s[:2] == ("f", "n") for s in seen)


def test_failure_is_shrunk_and_annotated(output):
    @hegel.test(print_blob=True)
    def test_it(tc):
        xs = tc.draw(gs.lists(gs.integers()))
        assert sum(xs) < 3

    with pytest.raises(AssertionError) as info:
        test_it()
    assert notes(info.value)[0] == "Falsifying example:\nxs = [3]"
    assert notes(info.value)[1].startswith(
        "To reproduce this failure, add this decorator below @hegel.test:\n"
        '    @hegel.reproduce_failure("'
    )
    # The report travels with the exception, so nothing else is printed.
    assert output.lines == []
    # The traceback of the failing call is trimmed to start in the test,
    # not in Hegel's runner calling it.
    frames = []
    tb = info.value.__traceback__
    while tb is not None:
        frames.append(tb.tb_frame.f_code.co_name)
        tb = tb.tb_next
    assert frames[-1] == "test_it"
    assert "_run_test_case" not in frames


def test_reproduce_failure_replays_blob(output):
    blobs = []

    @hegel.test(print_blob=True)
    def test_it(tc):
        xs = tc.draw(gs.lists(gs.integers()))
        assert len(xs) < 2

    with pytest.raises(AssertionError) as info:
        test_it()
    blob = notes(info.value)[1].split('"')[1]
    blobs.append(blob)

    seen = []

    @hegel.reproduce_failure(blob)
    @hegel.test
    def replay(tc):
        xs = tc.draw(gs.lists(gs.integers()))
        seen.append(xs)
        assert len(xs) < 2

    assert replay.hegel_reproduce_blob == blob
    with pytest.raises(AssertionError) as info:
        replay()
    assert seen == [[0, 0]]
    assert notes(info.value) == ["Falsifying example:\nxs = [0, 0]"]

    @hegel.reproduce_failure(blob)
    @hegel.test
    def fixed(tc):
        tc.draw(gs.lists(gs.integers()))

    with pytest.raises(hegel.DidNotReproduce):
        fixed()
    with pytest.raises(hegel.InvalidArgument):
        hegel.reproduce_failure("")


def test_reproduce_failure_with_invalid_blob():
    @hegel.reproduce_failure("not a blob")
    @hegel.test
    def test_it(tc):
        pass

    with pytest.raises(hegel.InvalidArgument):
        test_it()


def test_explicit_test_cases_run_first(output):
    seen = []

    @hegel.explicit_test_case(1, "one")
    @hegel.explicit_test_case(x=2, s="two")
    @hegel.test(test_cases=3)
    def test_it(tc):
        x = tc.draw(gs.integers(0, 100))
        s = tc.draw(gs.text())
        seen.append((x, s))
        tc.note("noted")
        tc.target(1.0)
        tc.event("e")
        tc.event_value("v", 1.0)

    assert [repr(c) for c in test_it.hegel_explicit_cases] == [
        "explicit_test_case(1, 'one')",
        "explicit_test_case(x=2, s='two')",
    ]
    test_it()
    assert seen[:2] == [(1, "one"), (2, "two")]
    assert len(seen) == 5


def test_explicit_test_case_failure_reports_values(output):
    @hegel.explicit_test_case(x=7)
    @hegel.test
    def test_it(tc):
        x = tc.draw(gs.integers())
        tc.note("line")
        assert x != 7

    with pytest.raises(AssertionError) as info:
        test_it()
    assert notes(info.value) == ["Falsifying example:\nx = 7\nline"]
    assert output.lines == []


def test_explicit_test_case_errors():
    @hegel.explicit_test_case(1)
    @hegel.test
    def unused(tc):
        pass

    with pytest.raises(hegel.InvalidArgument, match="not fully used"):
        unused()

    @hegel.explicit_test_case(y=1)
    @hegel.test
    def missing(tc):
        x = tc.draw(gs.integers())  # noqa: F841

    with pytest.raises(hegel.InvalidArgument, match="has no value for draw 'x'"):
        missing()

    @hegel.explicit_test_case()
    @hegel.test
    def unnamed(tc):
        tc.draw(gs.integers())

    with pytest.raises(hegel.InvalidArgument, match="has no value for draw 'draw_1'"):
        unnamed()

    @hegel.explicit_test_case(1)
    @hegel.test
    def silent(tc):
        tc.draw_silent(gs.integers())

    with pytest.raises(hegel.InvalidArgument, match="draw_silent"):
        silent()

    @hegel.explicit_test_case(1)
    @hegel.test
    def rejects(tc):
        x = tc.draw(gs.integers())
        tc.assume(x > 5)

    with pytest.raises(hegel.InvalidArgument, match="assumption violated"):
        rejects()

    @hegel.explicit_test_case(1)
    @hegel.test
    def engine_op(tc):
        tc._generate_boolean()

    with pytest.raises(hegel.InvalidArgument, match="not supported in explicit"):
        engine_op()

    @hegel.explicit_test_case(1)
    @hegel.test
    def done(tc):
        x = tc.draw(gs.integers())  # noqa: F841
        raise hegel.errors.LoopDone()

    done()


def test_explicit_cases_skipped_without_explicit_phase():
    seen = []

    @hegel.explicit_test_case(1)
    @hegel.test(phases=[Phase.generate], test_cases=2)
    def test_it(tc):
        seen.append(tc.draw(gs.integers(5, 9)))

    test_it()
    assert 1 not in seen


def test_find_example_shrinks():
    assert find_example(gs.integers(), lambda x: x > 100) == 101
    assert find_example(gs.lists(gs.integers()), lambda xs: len(xs) >= 2) == [0, 0]
    assert find_example(gs.just(3)) == 3
    with pytest.raises(hegel.Unsatisfiable):
        find_example(gs.integers(), lambda x: False)
    with pytest.raises(hegel.InvalidArgument):
        find_example(5)


def test_unconfirmed_failure_is_reported_with_a_caveat(output):
    calls = [0]

    def body(tc):
        tc.draw(gs.integers())
        calls[0] += 1
        assert calls[0] != 1

    error = failing(body)
    assert any(note.startswith("unconfirmed failure") for note in notes(error))
    assert not any("reproduce_failure" in note for note in notes(error))


def test_nondeterminism_strictness_error_aborts_the_run(output):
    calls = [0]

    def body(tc):
        tc.draw(gs.integers())
        calls[0] += 1
        assert calls[0] != 1

    failing(body, hegel.Flaky, nondeterminism_strictness=NondeterminismStrictness.error)


def test_nondeterminism_strictness_warn_prints_a_notice(output):
    calls = [0]

    def body(tc):
        tc.draw(gs.integers())
        calls[0] += 1
        assert calls[0] != 1

    failing(body, nondeterminism_strictness=NondeterminismStrictness.warn)
    assert any("nondeterminis" in line for line in output.lines)


def test_health_check_failure():
    def body(tc):
        tc.draw(gs.integers().filter(lambda x: x == 12345))

    error = failing(body, hegel.FailedHealthCheck, test_cases=100)
    assert "FilterTooMuch" in str(error)
    assert isinstance(error, hegel.RunError)


def test_health_check_can_be_suppressed():
    def body(tc):
        tc.draw(gs.integers(0, 10**6).filter(lambda x: x % 20 == 0))

    run_test(body, test_cases=5, suppress_health_check=hegel.HealthCheck.all())


def test_rejecting_every_case_is_unsatisfiable_even_with_health_checks_suppressed():
    def body(tc):
        tc.draw(gs.integers(0, 10**6).filter(lambda x: x < 0))

    error = failing(
        body, hegel.Unsatisfiable, test_cases=5, suppress_health_check=hegel.HealthCheck.all()
    )
    assert "No valid input was generated" in str(error)


def test_multiple_failures(output):
    def body(tc):
        x = tc.draw(gs.integers(0, 100))
        if x > 50:
            raise ValueError(x)
        assert x != 7

    error = failing(body, hegel.MultipleFailures, report_multiple_failures=True, test_cases=300)
    assert {type(e) for e in error.exceptions} == {ValueError, AssertionError}
    assert all(notes(e)[0].startswith("Falsifying example:") for e in error.exceptions)
    assert str(error).startswith("Property-based test failed with 2 distinct failures.")
    assert output.lines == []


def test_verbosity_verbose_prints_every_case(output):
    def body(tc):
        tc.draw(gs.integers())
        tc.note("hello")

    run_test(body, verbosity=Verbosity.verbose, test_cases=3)
    assert output.lines.count("hello") == 3
    assert "Test case passed" in output.lines


def test_verbose_output_for_failures_and_rejections(output):
    def body(tc):
        x = tc.draw(gs.integers(0, 10))
        tc.assume(x > 0)
        assert x < 5

    failing(body, verbosity=Verbosity.debug)
    assert "Test case rejected: assumption failed" in output.lines
    assert any(line.startswith("Test case failed: AssertionError at") for line in output.lines)
    assert output.text.count("Traceback") >= 2


def test_verbose_overrun_message(output):
    def body(tc):
        # a very large draw during shrinking makes the engine abandon cases
        while True:
            tc.draw_silent(gs.lists(gs.integers(), min_size=100))

    failing(body, hegel.RunError, verbosity=Verbosity.verbose, test_cases=3)


def test_quiet_verbosity_prints_nothing(output):
    def body(tc):
        x = tc.draw(gs.integers())
        assert x < 10

    error = failing(body, verbosity=Verbosity.quiet)
    assert output.lines == []
    assert notes(error) == []


def test_quiet_multiple_failures(output):
    def body(tc):
        x = tc.draw(gs.integers(0, 100))
        if x > 50:
            raise ValueError(x)
        assert x != 7

    failing(
        body,
        hegel.MultipleFailures,
        report_multiple_failures=True,
        test_cases=300,
        verbosity=Verbosity.quiet,
    )
    assert output.lines == []


def test_output_defaults_to_stderr(capsys):
    def body(tc):
        tc.note("to stderr")

    hegel.run(body, test_cases=1, verbosity=Verbosity.verbose)
    assert "to stderr" in capsys.readouterr().err


def test_a_failure_prints_nothing_beyond_its_exception(capsys):
    def body(tc):
        tc.note("only in the notes")
        raise ValueError

    with pytest.raises(ValueError) as info:
        hegel.run(body, database="")
    assert capsys.readouterr().err == ""
    assert "only in the notes" in notes(info.value)[0]


def test_a_failure_inside_hegel_keeps_its_whole_traceback():
    error = ValueError()
    error.__traceback__ = None
    hegel.runner._trim_traceback(error)
    assert error.__traceback__ is None

    def body(tc):
        tc.draw(gs.integers().map(lambda x: 1 / 0))

    exception = failing(body, ZeroDivisionError)
    assert exception.__traceback__ is not None


def test_a_reproducer_can_be_reworded():
    def body(tc):
        tc.draw(gs.integers())
        raise ValueError

    error = failing(body, ValueError, print_blob=True, reproducer=lambda blob: f"replay {blob}")
    assert notes(error)[-1].startswith("replay ")


def test_output_override_is_per_thread():
    lines = []

    def sink(line):
        lines.append(line)

    with with_output_override(sink):
        assert RunOutput.resolve().sink is sink
        result = []
        thread = threading.Thread(target=lambda: result.append(RunOutput.resolve().sink))
        thread.start()
        thread.join()
    assert result == [None]
    assert RunOutput.resolve().sink is None
    RunOutput(None).block("a\nb\n")


def test_keyboard_interrupt_propagates():
    def body(tc):
        raise KeyboardInterrupt

    with pytest.raises(KeyboardInterrupt):
        hegel.run(body)


def test_invalid_argument_in_test_propagates_with_output(output):
    def body(tc):
        x = tc.draw(gs.integers())  # noqa: F841
        raise hegel.InvalidArgument("bad")

    failing(body, hegel.InvalidArgument)
    assert output.lines == []


def test_internal_error_propagates():
    def body(tc):
        raise hegel.InternalError("bug")

    failing(body, hegel.InternalError)


def test_base_exception_subclass_is_a_failure():
    class Odd(BaseException):
        pass

    def body(tc):
        raise Odd

    failing(body, Odd)


def test_user_location_skips_hegel_frames():
    def body(tc):
        tc.draw(gs.integers().map(lambda x: 1 / 0))

    error = failing(body, ZeroDivisionError)
    assert __file__ in _user_location(error)
    assert _user_location(ValueError()) == "<unknown>"


def test_origin_includes_the_chain_and_group_members():
    def wrapped(cause):
        try:
            raise cause
        except LookupError as error:
            raise ValueError("wrapped") from error

    def caught(fn, *args):
        try:
            fn(*args)
        except BaseException as error:
            return error
        raise AssertionError("did not raise")  # nocov

    here = re.escape(__file__)
    origin = _origin(caught(wrapped, KeyError()))
    assert re.fullmatch(rf"ValueError at {here}:\d+ \(from KeyError at {here}:\d+\)", origin)
    assert origin != _origin(caught(wrapped, IndexError()))
    # The exception being handled counts too, even when its display is
    # suppressed (``raise ... from None``).
    covered = caught(wrapped, KeyError())
    covered.__cause__ = None
    assert "(from KeyError at" in _origin(covered)
    # A group is distinguished by its members, and a chain never loops.
    inner, outer = KeyError(), ValueError()
    inner.__context__, outer.__context__ = outer, inner
    group = caught(lambda: (_ for _ in ()).throw(ExceptionGroup("g", [inner, outer])))
    assert _origin(group) == (
        f"ExceptionGroup at {_user_location(group)} "
        "[KeyError at <unknown> (from ValueError at <unknown>), "
        "ValueError at <unknown> (from KeyError at <unknown>)]"
    )


def test_failures_with_different_causes_are_distinct_bugs():
    def body(tc):
        x = tc.draw(gs.integers(0, 1))
        try:
            if x:
                raise KeyError(x)
            raise IndexError(x)
        except LookupError as error:
            raise ValueError("wrapped") from error

    error = failing(body, hegel.MultipleFailures, report_multiple_failures=True, test_cases=50)
    assert {type(e.__cause__) for e in error.exceptions} == {KeyError, IndexError}


def test_a_group_of_control_exceptions_is_handled_as_one(output):
    # Rejected, as the assumption is preferred to the StopTest; nested
    # groups are unwrapped too.
    def rejecting(tc):
        x = tc.draw(gs.integers(0, 1))
        if x:
            raise BaseExceptionGroup("tasks", [hegel.StopTest(), hegel.AssumeFailed()])
        raise BaseExceptionGroup("tasks", [BaseExceptionGroup("inner", [hegel.AssumeFailed()])])

    failing(rejecting, hegel.FailedHealthCheck, verbosity=Verbosity.verbose, test_cases=10)
    assert "Test case stopped: the engine abandoned it" not in output.lines
    assert "Test case rejected: assumption failed" in output.lines

    def stopping(tc):
        tc.draw(gs.integers())
        raise BaseExceptionGroup("tasks", [hegel.StopTest()])

    failing(stopping, hegel.RunError, verbosity=Verbosity.verbose, test_cases=10)
    assert "Test case stopped: the engine abandoned it" in output.lines

    def done(tc):
        tc.draw(gs.integers())
        raise BaseExceptionGroup("tasks", [LoopDone()])

    run_test(done, test_cases=3)

    def invalid(tc):
        tc.draw(gs.integers())
        raise ExceptionGroup("tasks", [hegel.InvalidArgument("bad")])

    error = failing(invalid, hegel.InvalidArgument)
    assert str(error) == "bad" and error.__suppress_context__


def test_a_group_with_a_user_error_in_it_is_a_failure(output):
    def body(tc):
        tc.draw(gs.integers())
        raise BaseExceptionGroup("tasks", [hegel.AssumeFailed(), ValueError("boom")])

    error = failing(body, BaseExceptionGroup, verbosity=Verbosity.verbose)
    assert [type(e) for e in error.exceptions] == [hegel.AssumeFailed, ValueError]
    assert any(
        line.startswith("Test case failed: BaseExceptionGroup at") and "ValueError at" in line
        for line in output.lines
    )


def test_location_of():
    location = _location_of(test_location_of)
    assert location is not None
    assert location.file.endswith("test_runner.py")
    assert location.function == "test_location_of"
    assert location.class_name == ""

    class Holder:
        def method(self):
            pass

    assert _location_of(Holder.method).class_name == ""  # <locals>
    assert _location_of(len) is None


class TestInClass:
    @hegel.test(test_cases=2)
    def test_method(self, tc):
        assert tc.draw(gs.integers()) is not None

    def test_location_has_class_name(self):
        assert _location_of(TestInClass.test_method.hegel_inner_test).class_name == "TestInClass"
        self.test_method()


def test_run_with_settings_object_and_overrides():
    calls = []

    def body(tc):
        calls.append(tc.draw(gs.integers()))

    hegel.run(body, hegel.Settings(test_cases=4))
    hegel.run(body, hegel.Settings(test_cases=4), test_cases=2)
    assert len(calls) == 6


def test_derandomized_runs_are_repeatable():
    def collect():
        seen = []
        hegel.run(lambda tc: seen.append(tc.draw(gs.integers())), derandomize=True, test_cases=10)
        return seen

    assert collect() == collect()


def test_seeded_runs_are_repeatable():
    def collect(seed):
        seen = []
        hegel.run(lambda tc: seen.append(tc.draw(gs.integers())), seed=seed, test_cases=10)
        return seen

    assert collect(1) == collect(1)


def test_database_persists_failures(tmp_path):
    db = str(tmp_path / "db")
    seen = []

    def body(tc):
        xs = tc.draw(gs.lists(gs.integers()))
        seen.append(xs)
        assert len(xs) < 3

    with pytest.raises(AssertionError):
        hegel.run(body, database=db, database_key="k")
    seen.clear()
    with pytest.raises(AssertionError):
        hegel.run(body, database=db, database_key="k", phases=[Phase.reuse])
    assert seen and all(len(xs) >= 3 for xs in seen[:1])


def test_sys_exit_propagates():
    def body(tc):
        sys.exit(3)

    with pytest.raises(SystemExit):
        hegel.run(body)


class _FakeResult:
    def __init__(self, status, error=None, failures=(), kind=0):
        self._status = status
        self._error = error
        self._failures = list(failures)
        self._kind = kind

    def status(self):
        return self._status

    def error(self):
        return self._error

    def error_kind(self):
        return self._kind, 0

    def failure_count(self):
        return len(self._failures)

    def failure(self, index):
        return self._failures[index]


class _FakeRun:
    def __init__(self, result):
        self._result = result

    def next_test_case(self):
        return None

    def drain_output(self):
        return ["engine line"]

    def result(self):
        return self._result


@pytest.mark.parametrize(
    "result, exception, match",
    [
        (_FakeResult(hegel._native.RUN_STATUS_ERROR, "Something odd"), hegel.RunError, "odd"),
        (_FakeResult(hegel._native.RUN_STATUS_ERROR), hegel.RunError, "unknown error"),
        (
            _FakeResult(
                hegel._native.RUN_STATUS_ERROR, "flaky", kind=hegel._native.RUN_ERROR_FLAKY
            ),
            hegel.Flaky,
            "flaky",
        ),
        (
            _FakeResult(
                hegel._native.RUN_STATUS_ERROR,
                "nondeterministic",
                kind=hegel._native.RUN_ERROR_NONDETERMINISTIC,
            ),
            hegel.Flaky,
            "nondeterministic",
        ),
        (
            _FakeResult(hegel._native.RUN_STATUS_FAILED, failures=[("origin", None, None)]),
            hegel.InternalError,
            "never seen by the test",
        ),
    ],
)
def test_unexpected_engine_results(monkeypatch, output, result, exception, match):
    monkeypatch.setattr(hegel._native.Run, "start", staticmethod(lambda settings: _FakeRun(result)))
    with pytest.raises(exception, match=match):
        hegel.run(lambda tc: None)
    assert "engine line" in output.lines


def test_explicit_test_case_below_test_decorator():
    seen = []

    @hegel.test(test_cases=1)
    @hegel.explicit_test_case(1)
    @hegel.explicit_test_case(2)
    def test_it(tc):
        seen.append(tc.draw(gs.integers(5, 9)))

    assert [c.args for c in test_it.hegel_explicit_cases] == [(1,), (2,)]
    test_it()
    assert seen[:2] == [1, 2]


def test_skip_exceptions_end_the_run_without_shrinking():
    import unittest

    calls = []

    def body(tc):
        calls.append(tc.draw(gs.integers(0, 100)))
        raise unittest.SkipTest("not today")

    with pytest.raises(unittest.SkipTest):
        hegel.run(body, test_cases=50, database="")
    assert len(calls) == 1

    def pytest_skip(tc):
        tc.draw(gs.integers())
        pytest.skip("nor tomorrow")

    with pytest.raises(pytest.skip.Exception):
        hegel.run(pytest_skip, test_cases=50, database="")


def test_skip_exceptions_without_pytest(monkeypatch):
    import unittest

    from hegel.runner import _skip_exceptions

    assert pytest.skip.Exception in _skip_exceptions()
    monkeypatch.delitem(sys.modules, "_pytest.outcomes")
    assert _skip_exceptions() == (unittest.SkipTest,)


def test_async_tests_are_rejected():
    with pytest.raises(hegel.InvalidArgument, match="is async"):

        @hegel.test
        async def test_coroutine(tc):
            raise AssertionError

    with pytest.raises(hegel.InvalidArgument, match="is async"):

        @hegel.test
        async def test_async_generator(tc):
            yield

    async def body(tc):
        raise AssertionError

    with pytest.raises(hegel.InvalidArgument, match="returned an awaitable"):
        hegel.run(body, database="")
    with pytest.raises(hegel.InvalidArgument, match="returned an awaitable"):
        hegel.run(body, explicit_cases=[hegel.ExplicitCase((), {})], database="")

    class Awaitable:
        def __await__(self):
            yield

    with pytest.raises(hegel.InvalidArgument, match="returned an awaitable"):
        hegel.run(lambda tc: Awaitable(), database="")


def test_arguments_that_do_not_fit_fail_before_the_run():
    calls = []

    @hegel.test
    def test_it(tc, x):
        calls.append(x)

    with pytest.raises(TypeError, match="missing a required argument: 'x'"):
        test_it()
    assert calls == []


def test_calling_a_hegel_test_inside_another_is_an_error():
    @hegel.test(test_cases=1)
    def inner(tc):
        pass

    def outer(tc):
        inner()

    with pytest.raises(hegel.InvalidArgument, match="called from inside another Hegel test"):
        hegel.run(outer, database="")
    inner()  # fine once the outer run is over


def test_using_a_test_case_after_its_test_is_an_error():
    kept = []

    def body(tc):
        kept.append(tc)

    run_test(body, test_cases=1, database="")
    with pytest.raises(hegel.InvalidArgument, match="this test case has already finished"):
        kept[0].draw(gs.integers())


def test_drawing_from_something_other_than_a_generator_is_an_error():
    def from_int(tc):
        tc.draw(5)

    def from_function(tc):
        tc.draw(gs.integers)

    with pytest.raises(hegel.InvalidArgument, match="takes a generator, but got 5 of type int$"):
        hegel.run(from_int, database="")
    with pytest.raises(hegel.InvalidArgument, match="Did you forget to call it"):
        hegel.run(from_function, database="")


def test_settings_argument_must_be_a_settings_object():
    with pytest.raises(hegel.InvalidArgument, match="settings=5 must be a Settings object or None"):
        hegel.run(lambda tc: None, settings=5)
    with pytest.raises(hegel.InvalidArgument, match="settings=5 must be a Settings object or None"):
        hegel.test(settings=5)


def test_test_decorator_rejects_a_class():
    with pytest.raises(hegel.InvalidArgument, match="not the class Suite"):

        @hegel.test
        class Suite:
            def test_method(self, tc):
                pass


def test_test_decorator_requires_a_positional_test_case():
    with pytest.raises(hegel.InvalidArgument, match="'tc' cannot be passed positionally"):

        @hegel.test
        def keyword_only(*, tc):
            pass

    with pytest.raises(hegel.InvalidArgument, match="'kwargs' cannot be passed positionally"):

        @hegel.test
        def only_kwargs(**kwargs):
            pass

    with pytest.raises(hegel.InvalidArgument, match="first argument after self"):

        @hegel.test
        def method_without_test_case(self):
            pass

    @hegel.test(test_cases=2)
    def variadic(*args):
        assert isinstance(args[0], hegel.TestCase)

    variadic()


def test_explicit_cases_are_reported_in_verbose_mode(output):
    @hegel.explicit_test_case(x=1)
    @hegel.explicit_test_case(x=2)
    @hegel.test(verbosity=Verbosity.verbose, test_cases=1)
    def test_it(tc):
        x = tc.draw(gs.integers())
        tc.note(f"noted {x}")
        assert x != 2

    with pytest.raises(AssertionError):
        test_it()
    first = output.lines.index("Running explicit test case: explicit_test_case(x=1)")
    assert output.lines[first : first + 4] == [
        "Running explicit test case: explicit_test_case(x=1)",
        "x = 1",
        "noted 1",
        "Test case passed",
    ]
    second = output.lines.index("Running explicit test case: explicit_test_case(x=2)")
    assert output.lines[second + 1 : second + 3] == ["x = 2", "noted 2"]
    assert "Traceback (most recent call last):" in output.lines[second:]
    assert any(line.startswith("Test case failed: AssertionError at") for line in output.lines)


def test_explicit_case_misuse_is_reported_in_verbose_mode(output):
    @hegel.explicit_test_case(x=1)
    @hegel.test(verbosity=Verbosity.verbose, test_cases=1)
    def test_it(tc):
        x = tc.draw(gs.integers())  # noqa: F841
        tc.draw(gs.integers())

    with pytest.raises(hegel.InvalidArgument, match="has no value for draw"):
        test_it()
    assert output.lines[-1] == "x = 1"


def test_explicit_cases_print_nothing_unless_verbose(output):
    @hegel.explicit_test_case(x=1)
    @hegel.test(test_cases=1)
    def test_it(tc):
        x = tc.draw(gs.integers())  # noqa: F841

    test_it()
    assert output.lines == []


def test_first_explicit_failure_ends_the_run_by_default():
    seen = []

    @hegel.explicit_test_case(x=1)
    @hegel.explicit_test_case(x=2)
    @hegel.test(test_cases=1)
    def test_it(tc):
        x = tc.draw(gs.integers())
        seen.append(x)
        raise ValueError

    with pytest.raises(ValueError):
        test_it()
    assert seen == [1]


def test_explicit_failures_are_collected_with_generated_ones(output):
    seen = []

    @hegel.explicit_test_case(x=1)
    @hegel.explicit_test_case(x=2)
    @hegel.explicit_test_case(x=3)
    @hegel.test(report_multiple_failures=True, test_cases=200)
    def test_it(tc):
        x = tc.draw(gs.integers(-100, 100))
        seen.append(x)
        if x == 1:
            raise ValueError(x)
        if x == 2:
            raise KeyError(x)
        if x < 0:
            raise TypeError(x)

    with pytest.raises(hegel.MultipleFailures) as info:
        test_it()
    assert seen[:3] == [1, 2, 3] and len(seen) > 3
    assert [type(e) for e in info.value.exceptions] == [ValueError, KeyError, TypeError]
    assert notes(info.value.exceptions[0]) == ["Falsifying example:\nx = 1"]
    assert notes(info.value.exceptions[2]) == ["Falsifying example:\nx = -1"]
    assert str(info.value).startswith("Property-based test failed with 3 distinct failures.")
    assert output.lines == []


def test_a_generated_failure_with_an_explicit_failures_origin_is_the_same_bug():
    @hegel.explicit_test_case(x=7)
    @hegel.explicit_test_case(x=8)
    @hegel.test(report_multiple_failures=True)
    def test_it(tc):
        x = tc.draw(gs.integers())
        assert x < 0

    # The two explicit cases and the generated one all fail at the same
    # assertion: one bug, reported from the first explicit case.
    with pytest.raises(AssertionError) as info:
        test_it()
    assert notes(info.value) == ["Falsifying example:\nx = 7"]


def test_a_single_explicit_failure_is_raised_after_a_passing_generate_phase():
    @hegel.explicit_test_case(x=7)
    @hegel.test(report_multiple_failures=True)
    def test_it(tc):
        x = tc.draw(gs.integers())
        assert x != 7

    with pytest.raises(AssertionError) as info:
        test_it()
    assert info.value.__cause__ is None
    assert notes(info.value) == ["Falsifying example:\nx = 7"]


def test_a_run_error_after_explicit_failures_is_chained_onto_them():
    @hegel.explicit_test_case(x=7)
    @hegel.test(report_multiple_failures=True, test_cases=100)
    def test_it(tc):
        x = tc.draw(gs.integers().filter(lambda x: x == 12345))
        assert x != 7

    with pytest.raises(AssertionError) as info:
        test_it()
    assert isinstance(info.value.__cause__, hegel.FailedHealthCheck)


def test_errors_and_skips_in_explicit_cases_are_not_collected():
    import unittest

    @hegel.explicit_test_case(x=7)
    @hegel.explicit_test_case(x=8)
    @hegel.test(report_multiple_failures=True)
    def skips(tc):
        x = tc.draw(gs.integers())  # noqa: F841
        raise unittest.SkipTest("not today")

    with pytest.raises(unittest.SkipTest) as info:
        skips()
    assert notes(info.value) == []

    @hegel.explicit_test_case(x=7)
    @hegel.explicit_test_case(x=8)
    @hegel.test(report_multiple_failures=True)
    def interrupts(tc):
        x = tc.draw(gs.integers())  # noqa: F841
        raise KeyboardInterrupt

    with pytest.raises(KeyboardInterrupt):
        interrupts()

    def generate_phase_misuse(tc):
        x = tc.draw(gs.integers())
        if x != 7:
            raise hegel.InvalidArgument("bad")
        raise ValueError

    with pytest.raises(hegel.InvalidArgument, match="bad"):
        hegel.run(
            generate_phase_misuse,
            report_multiple_failures=True,
            explicit_cases=[hegel.ExplicitCase((7,), {})],
            database="",
        )

    def generate_phase_skip(tc):
        x = tc.draw(gs.integers())
        if x != 7:
            pytest.skip("not today")
        raise ValueError

    with pytest.raises(pytest.skip.Exception):
        hegel.run(
            generate_phase_skip,
            report_multiple_failures=True,
            explicit_cases=[hegel.ExplicitCase((7,), {})],
            database="",
        )
