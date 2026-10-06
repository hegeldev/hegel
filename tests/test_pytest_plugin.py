from __future__ import annotations

import sys

import pytest

TEST_FILE = """
import hegel
from hegel import generators as gs

@hegel.test(database="")
def test_passes(tc):
    assert tc.draw(gs.integers(0, 10)) <= 10

@hegel.test(database="")
def test_fails(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    assert len(xs) < 2

def test_plain():
    pass

def test_profile_seed_verbosity():
    settings = hegel.Settings()
    print("SEED", settings.seed, "VERBOSITY", settings.verbosity.name, "CASES", settings.test_cases)
"""


def test_plugin_marks_hegel_tests_and_reports_failures(pytester: pytest.Pytester):
    pytester.makepyfile(TEST_FILE)
    result = pytester.runpytest("-v", "-m", "hegel")
    result.assert_outcomes(passed=1, failed=1)
    result.stdout.fnmatch_lines(["*Falsifying example:*", "*xs = [[]0, 0]*"])
    result = pytester.runpytest("-m", "not hegel")
    result.assert_outcomes(passed=2)


def test_plugin_options(pytester: pytest.Pytester):
    pytester.makepyfile(TEST_FILE)
    pytester.makefile(
        ".py",
        conftest="""
import hegel
hegel.Settings.register_profile("mine", test_cases=7)
""",
    )
    result = pytester.runpytest(
        "-s",
        "-k",
        "profile_seed",
        "--hegel-profile=mine",
        "--hegel-seed=42",
        "--hegel-verbosity=quiet",
    )
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*SEED 42 VERBOSITY quiet CASES 7*"])
    result = pytester.runpytest("-s", "-k", "profile_seed", "--hegel-seed=1")
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*SEED 1 VERBOSITY*"])
    result = pytester.runpytest("-s", "-k", "profile_seed", "--hegel-verbosity=debug")
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*VERBOSITY debug*"])


HYPOTHESIS_FILE = """
from hegel.hypothesis import given, settings, strategies as st

@given(st.integers())
def test_hypothesis_style(x):
    print("SEED", getattr(test_hypothesis_style, "_hypothesis_internal_use_seed", None))
    print("PROFILE", settings.default.max_examples, settings.default.verbosity.name)
"""


@pytest.fixture(autouse=True)
def _fresh_compat_modules(monkeypatch: pytest.MonkeyPatch) -> None:
    """Each in-process pytest run below imports hegel.hypothesis afresh, so
    the profile one run loads cannot leak into the next."""
    for name in [
        m for m in sys.modules if m == "hegel.hypothesis" or m.startswith("hegel.hypothesis.")
    ]:
        monkeypatch.delitem(sys.modules, name)


def test_plugin_hypothesis_options(pytester: pytest.Pytester):
    pytester.makepyfile(HYPOTHESIS_FILE)
    pytester.makefile(
        ".py",
        conftest="""
from hegel.hypothesis import settings
settings.register_profile("mine", max_examples=3)
""",
    )
    result = pytester.runpytest("-v", "-m", "hypothesis")
    result.assert_outcomes(passed=1)
    result = pytester.runpytest(
        "-s",
        "--hypothesis-profile=mine",
        "--hypothesis-seed=5",
        "--hypothesis-verbosity=quiet",
    )
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*SEED 5*", "*PROFILE 3 quiet*"])
    result = pytester.runpytest("-s", "--hypothesis-verbosity=normal")
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*SEED None*", "*PROFILE 100 normal*"])
    result = pytester.runpytest("-s", "--hypothesis-profile=mine")
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*SEED None*", "*PROFILE 3 normal*"])


def test_plugin_seed_option_keeps_an_explicit_seed(pytester: pytest.Pytester):
    pytester.makepyfile(
        """
from hegel.hypothesis import given, seed, strategies as st

@seed(7)
@given(st.integers())
def test_seeded(x):
    print("SEED", test_seeded._hypothesis_internal_use_seed)
"""
    )
    result = pytester.runpytest("-s", "--hypothesis-seed=5")
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*SEED 7*"])


def test_plugin_rejects_pointless_hypothesis_decorators(pytester: pytest.Pytester):
    pytester.makepyfile(
        """
from hegel.hypothesis import example, settings

@example(1)
def test_example_without_given():
    pass

@settings(max_examples=5)
def test_settings_without_given():
    pass

def test_plain():
    pass
"""
    )
    result = pytester.runpytest("-v")
    result.assert_outcomes(passed=1, failed=2)
    result.stdout.fnmatch_lines(
        ["*Using `@example` on a test without `@given` is completely pointless*"]
    )


def test_plugin_suppresses_differing_executors_for_parametrized_methods(
    pytester: pytest.Pytester,
):
    pytester.makepyfile(
        """
import pytest
from hegel.hypothesis import given, settings, HealthCheck, strategies as st

class TestParametrized:
    @pytest.mark.parametrize("n", [1, 2])
    @given(st.integers())
    def test_method(self, n, x):
        pass

    @pytest.mark.parametrize("n", [1, 2])
    @settings(suppress_health_check=[HealthCheck.differing_executors])
    @given(st.integers())
    def test_already_suppressed(self, n, x):
        pass
"""
    )
    result = pytester.runpytest("-v")
    result.assert_outcomes(passed=4)
