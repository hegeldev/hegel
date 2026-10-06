from __future__ import annotations

import pytest

import hegel
from hegel import errors
from hegel.explicit import NO_HANDLE, ExplicitCase, ExplicitTestCase
from hegel.test_case import TestCase


def test_explicit_case_repr():
    assert repr(ExplicitCase((1, "a"), {"k": 2})) == "explicit_test_case(1, 'a', k=2)"
    assert repr(ExplicitCase((), {})) == "explicit_test_case()"


def test_no_handle_rejects_every_operation():
    with pytest.raises(hegel.InvalidArgument, match="anything is not supported"):
        NO_HANDLE.anything()


def test_explicit_test_case_consumption():
    explicit = ExplicitTestCase(ExplicitCase((1, 2), {"x": 3}))
    tc = TestCase(NO_HANDLE, True, None, explicit)
    assert tc.draw(hegel.generators.integers(), name="x") == 3
    assert tc.draw(hegel.generators.integers(), name="x") == 1  # x already used: positional
    assert tc.draw(hegel.generators.integers()) == 2
    assert explicit.lines == ["x = 3", "x_2 = 1", "Draw 1: 2"]
    tc.note("a\nb")
    assert explicit.lines[-2:] == ["a", "b"]
    explicit.check_consumed()
    with pytest.raises(hegel.InvalidArgument, match="unnamed draw"):
        tc.draw(hegel.generators.integers())
    with pytest.raises(hegel.InvalidArgument, match="draw 'y'"):
        tc.draw(hegel.generators.integers(), name="y")


def test_explicit_test_case_unused_values():
    explicit = ExplicitTestCase(ExplicitCase((1,), {"x": 3}))
    with pytest.raises(hegel.InvalidArgument) as info:
        explicit.check_consumed()
    message = str(info.value)
    assert "named values not drawn: 'x'" in message
    assert "1 positional value(s) not drawn" in message


def test_explicit_target_and_events_are_ignored():
    explicit = ExplicitTestCase(ExplicitCase((), {}))
    tc = TestCase(NO_HANDLE, True, None, explicit)
    tc.target(1.0)
    tc.event("e")
    tc.event_value("v", 2.0)
    with pytest.raises(hegel.InvalidArgument):
        tc.reject()


def test_error_hierarchy():
    assert issubclass(errors.FailedHealthCheck, errors.RunError)
    assert issubclass(errors.Flaky, errors.RunError)
    assert issubclass(errors.Unsatisfiable, errors.RunError)
    assert issubclass(errors.RunError, errors.HegelError)
    assert issubclass(errors.DidNotReproduce, errors.HegelError)
    assert issubclass(errors.InvalidArgument, errors.HegelError)
    assert issubclass(errors.BackendError, errors.HegelError)
    assert issubclass(errors.InternalError, errors.HegelError)
    assert issubclass(errors.HegelError, Exception)
    for control in (errors.StopTest, errors.AssumeFailed, errors.RetrySignal, errors.LoopDone):
        assert issubclass(control, BaseException) and not issubclass(control, Exception)
    group = errors.MultipleFailures("two", [ValueError(), KeyError()])
    assert isinstance(group, ExceptionGroup)
    assert len(group.exceptions) == 2
    assert "Flaky test detected" in errors.FLAKY_DIAGNOSTIC
    assert set(errors.__all__) <= set(dir(errors))
    assert set(hegel.__all__) <= set(dir(hegel))
    assert set(hegel.generators.__all__) <= set(dir(hegel.generators))


def test_explicit_draws_without_emit_are_not_recorded():
    explicit = ExplicitTestCase(ExplicitCase((1,), {}))
    tc = TestCase(NO_HANDLE, False, None, explicit)
    assert tc.draw(hegel.generators.integers(), name="x") == 1
    assert explicit.lines == []


def test_explicit_only_named_values_unused():
    explicit = ExplicitTestCase(ExplicitCase((), {"x": 3}))
    with pytest.raises(hegel.InvalidArgument) as info:
        explicit.check_consumed()
    assert "positional" not in str(info.value)
