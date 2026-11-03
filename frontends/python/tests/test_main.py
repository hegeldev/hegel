from __future__ import annotations

import inspect
import sys
from typing import Any

import pytest

import hegel
import hegel.runner
from hegel import HealthCheck, Phase, Settings
from hegel import generators as gs
from tests.helpers import notes


@pytest.fixture
def captured(monkeypatch: pytest.MonkeyPatch) -> dict[str, Any]:
    calls: dict[str, Any] = {}

    def fake_run(test_fn: Any, settings: Settings, **kwargs: Any) -> None:
        calls["settings"] = settings
        calls.update(kwargs)

    monkeypatch.setattr(hegel.runner, "_run", fake_run)
    return calls


def workload(tc: hegel.TestCase) -> None:
    tc.draw(gs.integers())


def usage_error(main: Any, argv: list[str], capsys: pytest.CaptureFixture[str]) -> str:
    with pytest.raises(SystemExit) as info:
        main(argv)
    assert info.value.code == 2
    return capsys.readouterr().err


def test_runs_exactly_one_test_case():
    seen = []

    @hegel.main
    def main(tc):
        seen.append(tc.draw(gs.integers()))

    assert main([]) is None
    assert len(seen) == 1


def test_one_test_case_whatever_the_settings(captured, monkeypatch):
    monkeypatch.setenv("HEGEL_TEST_CASES", "500")
    hegel.main(settings=Settings(test_cases=50))(workload)([])
    settings = captured["settings"]
    assert settings.test_cases == 1
    assert {HealthCheck.too_slow, HealthCheck.test_cases_too_large} <= set(
        settings.suppress_health_check
    )
    assert captured["database_key"] == "tests.test_main.workload"
    assert captured["test_location"].function == "workload"
    assert captured["blobs"] == []
    assert captured["explicit_cases"] == []


def test_a_script_is_keyed_by_its_path(captured):
    def script(tc):
        pass

    script.__module__ = "__main__"
    hegel.main(script)([])
    assert captured["database_key"] == f"{__file__}:{script.__qualname__}"


def test_options_override_the_decorator(captured):
    main = hegel.main(seed=1, derandomize=True)(workload)
    main(["--seed", "5", "--phases=generate", "--suppress-health-check", "filter_too_much"])
    settings = captured["settings"]
    assert settings.seed == 5
    assert settings.derandomize is True
    assert settings.phases == frozenset({Phase.generate})
    assert settings.suppress_health_check == frozenset(
        {HealthCheck.filter_too_much, HealthCheck.too_slow, HealthCheck.test_cases_too_large}
    )


def test_reads_sys_argv_by_default(captured, monkeypatch):
    monkeypatch.setattr(sys, "argv", ["workload", "--seed", "3"])
    hegel.main(workload)()
    assert captured["settings"].seed == 3


def test_profile_applies_under_the_decorator(captured):
    Settings.register_profile("main-profile", seed=7, show_statistics=True)
    default = Settings()
    hegel.main(show_statistics=False)(workload)(["--profile", "main-profile"])
    assert captured["settings"].seed == 7
    assert captured["settings"].show_statistics is False
    assert Settings() == default

    hegel.main(settings=Settings(seed=3))(workload)(["--profile", "main-profile"])
    assert captured["settings"].seed == 3


def test_explicit_cases_and_reproduce_blobs_apply(captured):
    @hegel.explicit_test_case(n=1)
    @hegel.main
    @hegel.reproduce_failure("BLOB1")
    @hegel.explicit_test_case(n=2)
    def main(tc):
        tc.draw(gs.integers())

    main(["--failure-blob", "BLOB2"])
    assert [case.kwargs for case in captured["explicit_cases"]] == [{"n": 1}, {"n": 2}]
    assert captured["blobs"] == ["BLOB1", "BLOB2"]
    assert captured["remedy"] == "remove the decorator and do not pass --failure-blob"

    @hegel.main
    @hegel.reproduce_failure("BLOB1")
    @hegel.reproduce_failure("BLOB2")
    def two(tc):
        tc.draw(gs.integers())

    two([])
    assert captured["remedy"] == "remove the decorators"


def test_reports_its_own_signature():
    assert list(inspect.signature(hegel.main(workload)).parameters) == ["argv"]


def test_refuses_to_run_inside_a_test():
    main = hegel.main(workload)

    @hegel.test(test_cases=1)
    def outer(tc):
        main([])

    with pytest.raises(hegel.InvalidArgument, match="called from inside a Hegel test"):
        outer()


def test_failure_reports_how_to_rerun_and_replays(output):
    @hegel.main
    def main(tc):
        n = tc.draw(gs.integers(min_value=1))
        assert n <= 0

    with pytest.raises(AssertionError) as info:
        main(["--print-blob", "true"])
    assert notes(info.value)[0] == "n = 1"
    rerun = notes(info.value)[-1]
    assert rerun.startswith("rerun with: --failure-blob ")
    blob = rerun.removeprefix("rerun with: --failure-blob ")

    with pytest.raises(AssertionError):
        main(["--failure-blob", blob])

    @hegel.main
    def fixed(tc):
        tc.draw(gs.integers(min_value=1))

    with pytest.raises(
        hegel.DidNotReproduce, match="do not pass --failure-blob once the failure is fixed"
    ):
        fixed(["--failure-blob", blob])


def test_usage_errors(capsys):
    main = hegel.main(workload)
    assert "--test-cases cannot be set" in usage_error(main, ["--test-cases", "5"], capsys)
    assert '--seed: `seed` expects an integer or "none", got "nope"' in usage_error(
        main, ["--seed", "nope"], capsys
    )
    assert 'unknown settings profile "nope"' in usage_error(main, ["--profile", "nope"], capsys)
    assert "unrecognized arguments: --bogus" in usage_error(main, ["--bogus"], capsys)
    assert "unrecognized arguments: --se" in usage_error(main, ["--se", "1"], capsys)


def test_help_lists_the_options(capsys):
    with pytest.raises(SystemExit) as info:
        hegel.main(workload)(["--help"])
    assert info.value.code == 0
    out = capsys.readouterr().out
    assert "--profile NAME" in out
    assert "--failure-blob BLOB" in out
    assert "--nondeterminism-strictness" in out
    assert "--test-cases" not in out


def test_rejects_what_it_cannot_run():
    with pytest.raises(hegel.InvalidArgument, match="test_cases cannot be set"):
        hegel.main(test_cases=5)
    with pytest.raises(hegel.InvalidArgument, match="must be a Settings"):
        hegel.main(settings={"seed": 1})  # type: ignore[arg-type]
    with pytest.raises(hegel.InvalidArgument, match="either a function or keyword settings"):
        hegel.main(workload, seed=1)
    with pytest.raises(hegel.InvalidArgument, match="must decorate a function"):
        hegel.main(Settings)
    with pytest.raises(hegel.InvalidArgument, match="must decorate a function"):
        hegel.main(1)  # type: ignore[arg-type]

    async def coroutine(tc):
        pass

    with pytest.raises(hegel.InvalidArgument, match="is async"):
        hegel.main(coroutine)

    def no_arguments():
        pass

    def two_arguments(tc, other):
        pass

    for fn in (no_arguments, two_arguments):
        with pytest.raises(hegel.InvalidArgument, match="exactly one argument"):
            hegel.main(fn)  # type: ignore[arg-type]
