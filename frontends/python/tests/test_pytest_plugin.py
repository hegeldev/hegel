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
    result.stdout.fnmatch_lines(["*xs = [[]0, 0]*"])
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


def test_plugin_rejects_a_non_integer_seed(pytester: pytest.Pytester):
    pytester.makepyfile(TEST_FILE)
    result = pytester.runpytest("--hegel-seed=abc")
    assert result.ret == pytest.ExitCode.USAGE_ERROR
    result.stderr.fnmatch_lines(["*--hegel-seed:*`seed` expects an integer*abc*"])


SETTINGS_FILE = """
import hegel

def test_settings():
    settings = hegel.Settings()
    print("CASES", settings.test_cases, "UNBOUNDED", settings.unbounded_choices)
    print("SUPPRESSED", ",".join(sorted(h.name for h in settings.suppress_health_check)))
    print("BACKEND", settings.backend.name, "DATABASE", repr(settings.database))
"""


def test_plugin_sets_any_setting_from_its_hegel_toml_key(pytester: pytest.Pytester):
    pytester.makepyfile(SETTINGS_FILE)
    result = pytester.runpytest(
        "-s",
        "--hegel-setting=test_cases=17",
        "--hegel-setting",
        " unbounded_choices = yes ",
        "--hegel-setting=suppress_health_check=too_slow,filter_too_much",
        "--hegel-setting=backend=urandom",
        "--hegel-setting=database=disabled",
    )
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(
        [
            "*CASES 17 UNBOUNDED True*",
            "*SUPPRESSED filter_too_much,too_slow*",
            "*BACKEND urandom DATABASE ''*",
        ]
    )


def test_plugin_options_apply_in_order(pytester: pytest.Pytester):
    pytester.makepyfile(TEST_FILE)
    result = pytester.runpytest(
        "-s", "-k", "profile_seed", "--hegel-setting=seed=5", "--hegel-seed=6"
    )
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*SEED 6 VERBOSITY*"])


@pytest.mark.parametrize(
    ("option", "message"),
    [
        ("--hegel-setting=test_cases", "*--hegel-setting='test_cases' must be KEY=VALUE*"),
        ("--hegel-setting=nope=1", "*--hegel-setting:*unknown setting `nope`*"),
        ("--hegel-setting=verbosity=loud", "*`verbosity` expects one of*loud*"),
        ("--hegel-verbosity=loud", "*--hegel-verbosity:*`verbosity` expects one of*"),
        ("--hegel-setting=database_key=k", "*database_key is set for each test*"),
    ],
)
def test_plugin_rejects_bad_settings(pytester: pytest.Pytester, option: str, message: str):
    pytester.makepyfile(SETTINGS_FILE)
    result = pytester.runpytest(option)
    assert result.ret == pytest.ExitCode.USAGE_ERROR
    result.stderr.fnmatch_lines([message])


def test_settings_made_in_a_conftest_follow_the_options(pytester: pytest.Pytester):
    pytester.makeconftest(
        """
import hegel
FAST = hegel.Settings(test_cases=3, database="")
"""
    )
    pytester.makepyfile(
        """
import hegel
from hegel import generators as gs
from conftest import FAST

SEEN = []

@hegel.test(settings=FAST)
def test_fast(tc):
    SEEN.append(tc.draw(gs.integers()))

def test_report():
    print("SEED", FAST.seed, "VERBOSITY", FAST.verbosity.name, "CASES", FAST.test_cases)
    print("SEEN", len(SEEN))
"""
    )
    result = pytester.runpytest("-s", "--hegel-seed=42", "--hegel-setting", "verbosity=verbose")
    result.assert_outcomes(passed=2)
    result.stdout.fnmatch_lines(["*SEED 42 VERBOSITY verbose CASES 3*", "*SEEN 3*"])
    result.stderr.fnmatch_lines(["Running test case", "draw_1 = *"])
