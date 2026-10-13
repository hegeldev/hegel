from __future__ import annotations

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
    print("DERANDOMIZE", settings.derandomize, "DATABASE", repr(settings.database))
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
    result = pytester.runpytest(
        "-s", "-k", "profile_seed", "--hegel-derandomize", "--hegel-database=elsewhere"
    )
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*DERANDOMIZE True DATABASE 'elsewhere'*"])
    result = pytester.runpytest("-s", "-k", "profile_seed", "--hegel-database=")
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*DATABASE ''*"])
