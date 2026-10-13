# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_can_repeatedly_push_the_same_thing,
# test_settings_in_strategies_are_from_test_scope, test_settings_apply_for_explicit_examples
# (local_settings / settings.default scoping); test_will_reload_profile_when_default_is_absent,
# test_cannot_assign_default, test_setattr_on_settings_singleton_is_error,
# test_register_profile_avoids_intermediate_profiles (settings.default descriptor and
# profile internals); test_settings_alone, test_settings_applied_twice_is_error,
# test_outer_ok, test_inner_ok (the @settings decorator);
# test_puts_the_database_in_the_home_dir_by_default, test_database_is_reference_preserved
# (ExampleDatabase objects); test_deadline_given_none, test_deadline_given_valid_int,
# test_deadline_given_valid_float, test_deadline_given_valid_timedelta (deadline);
# test_settings_decorator_applies_to_rule_based_state_machine_class,
# test_two_settings_decorators_applied_to_state_machine_class_raises_error,
# test_settings_decorator_applied_to_non_state_machine_class_raises_error,
# test_assigning_to_settings_attribute_on_state_machine_raises_error (@settings on a
# state machine class, stateful_step_count); test_derandomise_with_explicit_database_is_invalid
# (Hegel allows derandomize with a database); test_show_changed (show_changed);
# test_note_deprecation_checks_date, test_note_deprecation_checks_has_codemod,
# test_deprecated_settings_warn_on_set_settings, test_deprecated_settings_not_in_settings_all_list,
# test_cannot_register_profile_from_inside_test, test_can_set_verbosity_to_integers,
# test_can_set_phase_to_integers, test_can_set_suppressions_to_integers (deprecations and
# deprecated HealthCheck members); test_verbosity_is_comparable (Hegel's Verbosity is not
# ordered).

import os
import subprocess
import sys
from contextlib import contextmanager
from unittest import TestCase

import pytest

import hegel
from hegel import HealthCheck, InvalidArgument, Phase, Settings, Verbosity
from hegel import generators as gs
from tests.hypothesis_port.common.utils import counts_calls, skipif_emscripten

# The profile the suite's conftest makes the default before every test.
SUITE_PROFILE = "hypothesis-port"

# Environment variables the engine reads when choosing the environment's
# profile (CI detection, Antithesis, and explicit overrides).
_CI_VARS = (
    "CI",
    "TF_BUILD",
    "BUILDKITE",
    "CIRCLECI",
    "CIRRUS_CI",
    "CODEBUILD_BUILD_ID",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "HEROKU_TEST_RUN_ID",
    "TEAMCITY_VERSION",
    "bamboo.buildKey",
)
_PROFILE_VARS = (
    "HEGEL_DEFAULT_PROFILE",
    "HEGEL_CONFIG",
    "HEGEL_DERANDOMIZE",
    "ANTITHESIS_OUTPUT_DIR",
)


@contextmanager
def restore_profile():
    # avoid polluting global state by resetting the default profile back to
    # the suite's in tests which use set_default_profile
    try:
        yield
    finally:
        Settings.set_default_profile(SUITE_PROFILE)


def test_cannot_set_non_settings():
    s = Settings()
    with pytest.raises(AttributeError):
        s.databas_file = "some_file"


def test_settings_uses_defaults():
    s = Settings()
    assert s.test_cases == Settings.from_profile(SUITE_PROFILE).test_cases


def test_raises_attribute_error():
    with pytest.raises(AttributeError):
        Settings().kittens  # noqa: B018


def test_respects_none_database():
    assert Settings(database=None).database is None


def test_can_set_verbosity():
    Settings(verbosity=Verbosity.quiet)
    Settings(verbosity=Verbosity.normal)
    Settings(verbosity=Verbosity.verbose)
    Settings(verbosity=Verbosity.debug)


def test_can_not_set_verbosity_to_non_verbosity():
    with pytest.raises(InvalidArgument):
        Settings(verbosity="kittens")


@pytest.mark.parametrize("db", [None, ""])
def test_inherits_an_empty_database(db, tmp_path):
    parent = Settings(database=tmp_path)
    assert parent.database is not None
    s = Settings(parent, database=db)
    assert s.database == db
    t = Settings(s)
    assert t.database == db


@pytest.mark.parametrize("db", [None, ""])
def test_can_assign_database(db):
    x = Settings(database=db)
    assert x.database == db


def test_load_profile():
    with restore_profile():
        original_default = Settings.from_profile(SUITE_PROFILE).test_cases
        assert Settings().test_cases == original_default

        Settings.register_profile("test", Settings(test_cases=10))
        Settings.set_default_profile("test")

        assert Settings().test_cases == 10

        Settings.set_default_profile(SUITE_PROFILE)

        assert Settings().test_cases == original_default


def test_profile_names_must_be_strings():
    with pytest.raises(TypeError):
        Settings.register_profile(5)
    with pytest.raises(TypeError):
        Settings.from_profile(5)
    with pytest.raises(TypeError):
        Settings.set_default_profile(5)


def test_loading_profile_keeps_expected_behaviour():
    with restore_profile():
        Settings.register_profile("many", Settings(test_cases=10000))
        Settings.set_default_profile("many")
        assert Settings().test_cases == 10000
        assert Settings(test_cases=5).test_cases == 5
        assert Settings().test_cases == 10000


def test_load_non_existent_profile():
    with pytest.raises(InvalidArgument):
        Settings.from_profile("nonsense")
    with pytest.raises(InvalidArgument):
        Settings.set_default_profile("nonsense")


def test_cannot_set_settings():
    x = Settings()
    with pytest.raises(AttributeError):
        x.test_cases = "foo"
    with pytest.raises(AttributeError):
        x.database = "foo"
    assert x.test_cases != "foo"
    assert x.database != "foo"


def test_can_have_none_database():
    assert Settings(database=None).database is None


@pytest.mark.parametrize("db", [None, ""])
@pytest.mark.parametrize("bad_db", [3, object()])
def test_database_type_must_be_a_path(db, bad_db):
    with pytest.raises(InvalidArgument):
        Settings(Settings(database=db), database=bad_db)


def test_settings_as_decorator_must_be_on_callable():
    with pytest.raises(InvalidArgument):
        hegel.test(test_cases=1)(1)
    with pytest.raises(InvalidArgument):
        hegel.test(1)


class TestGivenExampleSettingsExplicitCalled(TestCase):
    """Real nasty edge case here.

    in #2160, if ``example`` is after ``given`` but before ``settings``,
    it will be completely ignored.

    If we set phases to only ``explicit``, the test case will never be called!

    We have to run an assertion outside of the test case itself.
    """

    @counts_calls
    def call_target(self):
        pass

    @hegel.test(phases=[Phase.explicit])
    @hegel.explicit_test_case(True)
    def test_example_explicit(self, tc):
        tc.draw(gs.booleans())
        self.call_target()

    def tearDown(self):
        # In #2160, this is 0.
        assert self.call_target.calls == 1


@pytest.mark.parametrize("value", ["always"])
def test_can_not_set_print_blob_to_non_print_settings(value):
    with pytest.raises(InvalidArgument):
        Settings(print_blob=value)


@pytest.mark.parametrize(
    "kwargs",
    [
        {"test_cases": -1},
        {"test_cases": 2.5},
        {"backend": "nonexistent_backend"},
        {"suppress_health_check": ["nonexistent_healthcheck"]},
        {"phases": ["nonexistent_phase"]},
        {"phases": 0},
        {"verbosity": -1},
        {"verbosity": "nonexistent_verbosity"},
        {"nondeterminism_strictness": "nonexistent_strictness"},
    ],
)
def test_invalid_settings_are_errors(kwargs):
    with pytest.raises(InvalidArgument):
        Settings(**kwargs)


@pytest.mark.xfail(
    strict=True,
    reason="Settings(parent) with something that is not a Settings raises "
    "AttributeError ('NotSettings' object has no attribute 'test_cases'), "
    "not InvalidArgument naming the parent",
)
def test_invalid_parent():
    class NotSettings:
        def __repr__(self):
            return "(not settings repr)"

    not_settings = NotSettings()

    with pytest.raises(InvalidArgument, match=r"\(not settings repr\)"):
        Settings(not_settings)


def test_default_settings_do_not_use_ci():
    development = Settings.from_profile("development")
    assert development.suppress_health_check == frozenset()
    assert development.derandomize is False


def _derandomize_in_subprocess(env):
    return subprocess.check_output(
        [sys.executable, "-c", "import hegel\nprint(hegel.Settings().derandomize)"],
        env=env,
        text=True,
        encoding="utf-8",
    ).strip()


@skipif_emscripten
def test_check_defaults_to_derandomize_when_running_on_ci():
    env = dict(os.environ)
    for key in _PROFILE_VARS:
        env.pop(key, None)
    env["CI"] = "true"

    assert _derandomize_in_subprocess(env) == "True"


@skipif_emscripten
def test_check_defaults_to_randomize_when_not_running_on_ci():
    env = dict(os.environ)
    for key in _CI_VARS + _PROFILE_VARS:
        env.pop(key, None)

    assert _derandomize_in_subprocess(env) == "False"


def test_reloads_the_loaded_profile_if_registered_again():
    with restore_profile():
        test_profile = "some_nonsense_profile_purely_for_this_test"
        test_value = 123456
        Settings.register_profile(test_profile, Settings(test_cases=test_value))
        Settings.set_default_profile(test_profile)
        assert Settings().test_cases == test_value
        test_value_2 = 42
        Settings.register_profile(test_profile, Settings(test_cases=test_value_2))
        assert Settings().test_cases == test_value_2


CI_TESTING_SCRIPT = """
import hegel

if __name__ == '__main__':
    hegel.Settings.register_profile("ci", hegel.Settings(test_cases=42))
    assert hegel.Settings().test_cases == 42, hegel.Settings().test_cases
"""


@skipif_emscripten
def test_will_automatically_pick_up_changes_to_ci_profile_in_ci():
    env = dict(os.environ)
    for key in _PROFILE_VARS:
        env.pop(key, None)
    env["CI"] = "true"
    subprocess.check_call(
        [sys.executable, "-c", CI_TESTING_SCRIPT],
        env=env,
        text=True,
        encoding="utf-8",
    )


def test_can_set_verbosity_to_strings():
    assert Settings(verbosity="quiet").verbosity is Verbosity.quiet
    assert Settings(verbosity="normal").verbosity is Verbosity.normal
    assert Settings(verbosity="verbose").verbosity is Verbosity.verbose
    assert Settings(verbosity="debug").verbosity is Verbosity.debug


def test_can_set_phase_to_strings():
    assert Settings(phases=["reuse"]).phases == frozenset({Phase.reuse})
    assert Settings(phases=["reuse", "explicit"]).phases == frozenset({Phase.explicit, Phase.reuse})


def test_can_set_suppressions_to_strings():
    assert Settings(suppress_health_check=["filter_too_much"]).suppress_health_check == frozenset(
        {HealthCheck.filter_too_much}
    )
    assert Settings(
        suppress_health_check=["filter_too_much", "too_slow"]
    ).suppress_health_check == frozenset({HealthCheck.filter_too_much, HealthCheck.too_slow})


def test_invalid_integer_phase_raises():
    with pytest.raises(ValueError):
        Phase(99)


def test_invalid_integer_healthcheck_raises():
    with pytest.raises(ValueError):
        HealthCheck(99)
