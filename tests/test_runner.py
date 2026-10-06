from __future__ import annotations

import inspect
import sys
import threading

import pytest

import hegel
from hegel import generators as gs
from hegel.runner import (
    RunOutput,
    _location_of,
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
    assert notes(info.value)[1].startswith("To reproduce this failure")
    assert "xs = [3]" in output.text
    assert "AssertionError" in output.text
    assert '@hegel.reproduce_failure("' in output.text


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
    assert "Explicit test case explicit_test_case(x=7) failed:" in output.lines
    assert "x = 7" in output.lines


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
        tc.generate_boolean()

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
    with pytest.raises(hegel.InvalidArgument):
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
    assert any(line.startswith("note: unconfirmed failure") for line in output.lines)
    assert "reproduce_failure" not in "\n".join(output.lines)


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
        tc.draw(gs.integers(0, 10**6).filter(lambda x: x == 12345))

    run_test(body, test_cases=5, suppress_health_check=hegel.HealthCheck.all())


def test_multiple_failures(output):
    def body(tc):
        x = tc.draw(gs.integers(0, 100))
        if x > 50:
            raise ValueError(x)
        assert x != 7

    error = failing(body, hegel.MultipleFailures, report_multiple_failures=True, test_cases=300)
    assert {type(e) for e in error.exceptions} == {ValueError, AssertionError}
    assert all(notes(e)[0].startswith("Falsifying example:") for e in error.exceptions)
    assert "Property-based test failed with 2 distinct failures." in output.lines


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
        raise ValueError

    with pytest.raises(ValueError):
        hegel.run(body)
    assert "to stderr" in capsys.readouterr().err


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
    def __init__(self, status, error=None, failures=()):
        self._status = status
        self._error = error
        self._failures = list(failures)

    def status(self):
        return self._status

    def error(self):
        return self._error

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
