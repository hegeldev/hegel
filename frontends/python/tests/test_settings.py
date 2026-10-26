from __future__ import annotations

import re
from pathlib import Path

import pytest

import hegel
from hegel import _native
from hegel.settings import (
    _FIELDS,
    Backend,
    HealthCheck,
    NondeterminismStrictness,
    Phase,
    Settings,
    TestLocation,
    Verbosity,
)


def test_default_settings_come_from_default_profile():
    settings = Settings()
    assert settings.database == ""
    assert settings.test_cases == 30


def test_overrides_apply_and_validate():
    settings = Settings(test_cases=7, seed=5, verbosity=Verbosity.verbose)
    assert settings.test_cases == 7
    assert settings.seed == 5
    assert settings.verbosity is Verbosity.verbose
    with pytest.raises(TypeError):
        Settings(no_such_field=1)
    with pytest.raises(hegel.InvalidArgument):
        Settings(test_cases=-1)


def test_coercions():
    settings = Settings(
        seed=-1,
        database=False,
        phases=[Phase.generate, Phase.shrink],
        suppress_health_check=HealthCheck.too_slow,
        verbosity=Verbosity.verbose,
        backend=Backend.urandom,
    )
    assert settings.seed == (1 << 64) - 1
    assert settings.database == ""
    assert settings.phases == frozenset({Phase.generate, Phase.shrink})
    assert settings.suppress_health_check == frozenset({HealthCheck.too_slow})
    assert settings.verbosity is Verbosity.verbose
    assert settings.backend is Backend.urandom
    assert Settings(database="/tmp/x").database == "/tmp/x"
    assert Settings(database=None).database is None


def test_replace_and_parent():
    base = Settings(test_cases=3)
    child = base.replace(seed=9)
    assert child.test_cases == 3 and child.seed == 9
    assert Settings(base, seed=10).seed == 10
    assert base.seed is None


def test_profiles():
    Settings.register_profile("custom", test_cases=11, derandomize=True)
    assert Settings.from_profile("custom").test_cases == 11
    assert Settings.from_profile("custom", test_cases=12).test_cases == 12
    Settings.register_profile("custom2", Settings.from_profile("custom"), seed=1)
    custom2 = Settings.from_profile("custom2")
    assert custom2.test_cases == 11 and custom2.seed == 1 and custom2.derandomize
    Settings.set_default_profile("custom2")
    assert Settings().seed == 1
    Settings.set_default_profile("hegel-tests")
    assert Settings().seed is None
    with pytest.raises(hegel.InvalidArgument):
        Settings.from_profile("no-such-profile")


def test_setting_an_unknown_default_profile_fails_and_changes_nothing():
    with pytest.raises(hegel.InvalidArgument, match="unknown settings profile"):
        Settings.set_default_profile("no-such-profile")
    assert Settings().test_cases == Settings.from_profile("hegel-tests").test_cases


def test_clearing_the_default_profile_restores_the_engine_choice():
    Settings.register_profile("cleared", test_cases=3)
    Settings.set_default_profile("cleared")
    try:
        Settings.set_default_profile(None)
        assert Settings().test_cases != 3
    finally:
        Settings.set_default_profile("hegel-tests")


@pytest.mark.parametrize(
    "overrides, match",
    [
        ({"test_cases": "10"}, "test_cases='10' must be an integer"),
        ({"test_cases": True}, "test_cases=True must be an integer"),
        ({"test_cases": -1}, "test_cases=-1 must be non-negative"),
        ({"seed": 1.5}, "seed=1.5 must be an integer"),
        ({"derandomize": 1}, "derandomize=1 must be True or False"),
        ({"database": 3}, "database=3 must be a directory path"),
        ({"verbosity": "loud"}, "verbosity='loud' must be one of Verbosity.quiet"),
        ({"phases": 3}, "phases=3 must be a collection of Phase values"),
        ({"suppress_health_check": ["nope"]}, "suppress_health_check='nope' must be one of"),
    ],
)
def test_invalid_settings_values(overrides, match):
    with pytest.raises(hegel.InvalidArgument, match=match):
        Settings(**overrides)


def test_enum_fields_take_member_names_and_paths_are_accepted(tmp_path):
    settings = Settings(
        verbosity="quiet",
        suppress_health_check="filter_too_much",
        phases=["generate", Phase.shrink],
        database=tmp_path,
    )
    assert settings.verbosity is Verbosity.quiet
    assert settings.suppress_health_check == {HealthCheck.filter_too_much}
    assert settings.phases == {Phase.generate, Phase.shrink}
    assert settings.database == str(tmp_path)
    assert Settings(database=False).database == ""


def test_builtin_profiles():
    ci = Settings.from_profile("ci")
    assert ci.derandomize is True
    assert Settings.from_profile("workload").backend is Backend.urandom


def test_build_round_trips_every_field():
    settings = Settings(
        test_cases=5,
        verbosity=Verbosity.quiet,
        seed=123,
        derandomize=True,
        database="",
        suppress_health_check=HealthCheck.all(),
        phases=[Phase.generate],
        report_multiple_failures=True,
        stop_on_first_failure=False,
        show_statistics=True,
        unbounded_choices=True,
        print_blob=True,
        backend=Backend.urandom,
        nondeterminism_strictness=NondeterminismStrictness.error,
    )
    native = settings._build("key", TestLocation("f.py", 3, "Cls", "fn"))
    assert native.get_test_cases() == 5
    assert native.get_verbosity() == _native.VERBOSITY_QUIET
    assert native.get_seed() == 123
    assert native.get_derandomize()
    assert native.get_database() == ""
    assert native.get_suppress_health_check() == sum(h.value for h in HealthCheck)
    assert native.get_phases() == Phase.generate.value
    assert native.get_report_multiple_failures()
    assert not native.get_stop_on_first_failure()
    assert native.get_show_statistics()
    assert native.get_unbounded_choices()
    assert native.get_print_blob()
    assert native.get_backend() == _native.BACKEND_URANDOM
    assert native.get_nondeterminism_strictness() == _native.NONDETERMINISM_ERROR


def test_enum_helpers_and_fields():
    assert Phase.all() == frozenset(Phase)
    assert HealthCheck.all() == frozenset(HealthCheck)
    assert set(_FIELDS) == {
        "test_cases",
        "verbosity",
        "seed",
        "derandomize",
        "database",
        "suppress_health_check",
        "phases",
        "report_multiple_failures",
        "stop_on_first_failure",
        "show_statistics",
        "unbounded_choices",
        "print_blob",
        "backend",
        "nondeterminism_strictness",
    }
    assert TestLocation("f", 1).class_name == ""


def test_settings_are_frozen():
    settings = Settings()
    with pytest.raises(AttributeError):
        settings.test_cases = 3  # type: ignore[misc]


def test_parent_and_profile_arguments_are_validated():
    class NotSettings:
        def __repr__(self):
            return "(not settings)"

    with pytest.raises(hegel.InvalidArgument, match=r"parent=\(not settings\) must be a Settings"):
        Settings(NotSettings())
    with pytest.raises(
        hegel.InvalidArgument, match=r"settings=\(not settings\) must be a Settings"
    ):
        Settings.register_profile("bad", NotSettings())
    for call in (Settings.register_profile, Settings.from_profile, Settings.set_default_profile):
        with pytest.raises(hegel.InvalidArgument, match="name=5 must be a string naming a profile"):
            call(5)
    with pytest.raises(
        hegel.InvalidArgument, match=r"test_cases=18446744073709551616 must be less"
    ):
        Settings(test_cases=1 << 64)
    assert Settings(test_cases=(1 << 64) - 1)._build().get_test_cases() == (1 << 64) - 1


HEGEL_H = Path(__file__).parents[3] / "libhegel" / "include" / "hegel.h"


PER_TEST_SETTERS = {"database_key", "test_location", "from_string"}


def test_every_engine_setting_is_a_settings_field():
    header = HEGEL_H.read_text()
    getters = set(re.findall(r"hegel_settings_get_(\w+)\(", header))
    setters = set(re.findall(r"hegel_settings_set_(\w+)\(", header)) - PER_TEST_SETTERS
    fields = set(_FIELDS)
    assert fields
    assert getters == fields
    assert setters == fields


def test_engine_profile_settings_reach_the_run():
    native = _native.Settings.for_profile("base")
    native.set_unbounded_choices(True)
    native.set_show_statistics(True)
    native.set_nondeterminism_strictness(_native.NONDETERMINISM_ERROR)
    _native.register_profile("engine-made", native)
    settings = Settings.from_profile("engine-made")
    assert settings.unbounded_choices
    built = settings._build()
    assert built.get_unbounded_choices()
    assert built.get_show_statistics()
    assert built.get_nondeterminism_strictness() == _native.NONDETERMINISM_ERROR
    assert not Settings(settings, unbounded_choices=False)._build().get_unbounded_choices()


def test_unbounded_choices_is_validated():
    with pytest.raises(hegel.InvalidArgument, match="unbounded_choices=1 must be True or False"):
        Settings(unbounded_choices=1)


def test_settings_follow_a_default_profile_set_after_they_were_made():
    settings = Settings(test_cases=3)
    child = settings.replace(print_blob=True)
    Settings.register_profile("later", seed=5, verbosity=Verbosity.verbose, test_cases=50)
    try:
        Settings.set_default_profile("later")
        assert settings.seed == 5
        assert settings.verbosity is Verbosity.verbose
        assert settings.test_cases == 3
        assert child.seed == 5 and child.print_blob
        built = settings._build()
        assert built.get_seed() == 5 and built.get_test_cases() == 3
    finally:
        Settings.set_default_profile("hegel-tests")
    assert settings.seed is None


def test_settings_from_a_profile_do_not_follow_the_default():
    settings = Settings.from_profile("ci", test_cases=4)
    Settings.register_profile("other", seed=5)
    try:
        Settings.set_default_profile("other")
        assert settings.seed is None and settings.derandomize and settings.test_cases == 4
    finally:
        Settings.set_default_profile("hegel-tests")


def test_settings_equality_hash_and_repr():
    assert Settings(test_cases=3) == Settings(test_cases=3)
    assert hash(Settings(test_cases=3)) == hash(Settings(test_cases=3))
    assert Settings(test_cases=3) != Settings(test_cases=4)
    assert Settings(test_cases=3) != Settings.from_profile("ci", test_cases=3)
    assert Settings() != object()
    assert repr(Settings(test_cases=3, seed=1)) == "Settings(test_cases=3, seed=1)"
    assert repr(Settings()) == "Settings()"
    assert repr(Settings.from_profile("ci")) == "Settings.from_profile('ci')"
    assert (
        repr(Settings.from_profile("ci", test_cases=3))
        == "Settings.from_profile('ci', test_cases=3)"
    )


def test_settings_cannot_be_changed_or_deleted():
    settings = Settings(test_cases=3)
    with pytest.raises(AttributeError, match="immutable"):
        settings.test_cases = 4  # type: ignore[misc]
    with pytest.raises(AttributeError, match="immutable"):
        del settings.test_cases
    with pytest.raises(AttributeError, match="no attribute 'kittens'"):
        settings.kittens  # noqa: B018
