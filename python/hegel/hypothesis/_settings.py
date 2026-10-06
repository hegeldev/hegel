"""The ``settings`` object and its profiles."""

from __future__ import annotations

import contextlib
import datetime
import inspect
import os
import threading
from collections.abc import Collection, Iterator
from enum import Enum, EnumMeta, IntEnum, unique
from typing import Any

from hegel import settings as hegel_settings
from hegel.hypothesis.database import DirectoryBasedExampleDatabase, ExampleDatabase
from hegel.hypothesis.errors import InvalidArgument
from hegel.hypothesis.utils.conventions import not_set
from hegel.hypothesis.utils.deprecation import note_deprecation

__all__ = ["HealthCheck", "Phase", "Verbosity", "settings"]


@unique
class Verbosity(IntEnum):
    quiet = 0
    normal = 1
    verbose = 2
    debug = 3

    def __repr__(self) -> str:
        return f"Verbosity.{self.name}"


@unique
class Phase(IntEnum):
    explicit = 0
    reuse = 1
    generate = 2
    target = 3
    shrink = 4
    explain = 5

    def __repr__(self) -> str:
        return f"Phase.{self.name}"


class HealthCheckMeta(EnumMeta):
    def __iter__(self) -> Iterator[HealthCheck]:
        deprecated = (HealthCheck.return_value, HealthCheck.not_a_test_method)
        return iter(x for x in super().__iter__() if x not in deprecated)


@unique
class HealthCheck(Enum, metaclass=HealthCheckMeta):
    data_too_large = 1
    filter_too_much = 2
    too_slow = 3
    return_value = 5
    large_base_example = 7
    not_a_test_method = 8
    function_scoped_fixture = 9
    differing_executors = 10
    nested_given = 11

    def __repr__(self) -> str:
        return f"HealthCheck.{self.name}"

    @classmethod
    def all(cls) -> list[HealthCheck]:
        note_deprecation(
            "`HealthCheck.all()` is deprecated; use `list(HealthCheck)` instead.",
            since="2023-04-16",
            has_codemod=True,
            stacklevel=1,
        )
        return list(cls)


_HEGEL_PHASES = {
    Phase.explicit: hegel_settings.Phase.explicit,
    Phase.reuse: hegel_settings.Phase.reuse,
    Phase.generate: hegel_settings.Phase.generate,
    Phase.target: hegel_settings.Phase.target,
    Phase.shrink: hegel_settings.Phase.shrink,
}
_HEGEL_HEALTH_CHECKS = {
    HealthCheck.data_too_large: hegel_settings.HealthCheck.test_cases_too_large,
    HealthCheck.filter_too_much: hegel_settings.HealthCheck.filter_too_much,
    HealthCheck.too_slow: hegel_settings.HealthCheck.too_slow,
    HealthCheck.large_base_example: hegel_settings.HealthCheck.large_initial_test_case,
}

all_settings = [
    "max_examples",
    "derandomize",
    "database",
    "verbosity",
    "phases",
    "stateful_step_count",
    "report_multiple_bugs",
    "suppress_health_check",
    "deadline",
    "print_blob",
    "backend",
]


def _check_type(typ: type, value: Any, name: str) -> None:
    if not isinstance(value, typ):
        raise InvalidArgument(
            f"Expected {typ.__name__} but got {name}={value!r} (type={type(value).__name__})"
        )


def _validate_max_examples(value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise InvalidArgument(f"Expected int but got max_examples={value!r}")
    if value < 1:
        raise InvalidArgument(
            f"max_examples={value!r} must be at least one. If you want to disable "
            "generation entirely, use phases=[Phase.explicit] instead."
        )
    return value


def _validate_bool(name: str, value: Any) -> bool:
    if value not in (True, False) or not isinstance(value, bool):
        raise InvalidArgument(f"Invalid {name}={value!r}, must be one of [True, False]")
    return value


def _validate_enum(enum: type[Enum], value: Any, name: str) -> Any:
    if isinstance(value, enum):
        return value
    try:
        if isinstance(value, str):
            return enum[value]
        return enum(value)
    except (ValueError, KeyError):
        raise InvalidArgument(
            f"Invalid {name}={value!r}, must be one of {[v for v in enum]!r}"
        ) from None


def _validate_phases(phases: Any) -> tuple[Phase, ...]:
    if isinstance(phases, (str, bytes)) or not isinstance(phases, Collection):
        raise InvalidArgument(f"phases={phases!r} must be a collection of Phase values")
    phases = [_validate_enum(Phase, p, "phases") for p in phases]
    return tuple(p for p in Phase if p in phases)


def _validate_health_checks(value: Any) -> tuple[HealthCheck, ...]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Collection):
        raise InvalidArgument(
            f"suppress_health_check={value!r} must be a collection of HealthCheck values"
        )
    checks = tuple(_validate_enum(HealthCheck, h, "suppress_health_check") for h in value)
    for check in checks:
        if check in (HealthCheck.return_value, HealthCheck.not_a_test_method):
            note_deprecation(
                f"The {check.name} health check is deprecated, because this is always an error.",
                since="2023-03-15",
                has_codemod=False,
                stacklevel=2,
            )
    return checks


def _validate_deadline(value: Any) -> datetime.timedelta | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise InvalidArgument(f"deadline={value!r} (type bool) must be a number or None")
    if isinstance(value, datetime.timedelta):
        delta = value
    elif isinstance(value, (int, float)):
        if value != value or value in (float("inf"), float("-inf")):
            raise InvalidArgument(f"deadline={value!r} must be finite")
        try:
            delta = datetime.timedelta(milliseconds=value)
        except OverflowError:
            raise InvalidArgument(f"deadline={value!r} is too large to be a timedelta") from None
    else:
        raise InvalidArgument(f"deadline={value!r} (type {type(value).__name__}) is not valid")
    if delta <= datetime.timedelta(0):
        raise InvalidArgument(f"deadline={value!r} must be positive")
    return delta


def _validate_database(value: Any) -> ExampleDatabase | None:
    if value is None or isinstance(value, ExampleDatabase):
        return value
    raise InvalidArgument(
        f"Expected None or ExampleDatabase but got database={value!r} "
        f"(type={type(value).__name__}). Pass DirectoryBasedExampleDatabase(path) "
        "to use a directory."
    )


_local = threading.local()

_CI_VARS: dict[str, str | None] = {
    "CI": None,  # various, including GitHub Actions, Travis CI, and AppVeyor
    "__TOX_ENVIRONMENT_VARIABLE_ORIGINAL_CI": None,  # tox under CI
    "TF_BUILD": "True",  # Azure Pipelines
}


def is_in_ci() -> bool:
    return any(
        key in os.environ and (value is None or os.environ[key] == value)
        for key, value in _CI_VARS.items()
    )


class _DefaultVariable:
    """The settings object currently loaded as the default (per thread),
    or None when the current profile has to be looked up again."""

    def __init__(self) -> None:
        self._data = threading.local()

    @property
    def value(self) -> settings | None:
        return getattr(self._data, "value", None)

    @value.setter
    def value(self, value: settings | None) -> None:
        self._data.value = value


default_variable = _DefaultVariable()


class settingsMeta(type):
    @property
    def default(cls) -> settings:
        override = getattr(_local, "override", None)
        if override is not None:
            return override
        current = default_variable.value
        if current is None:
            current = settings._profiles.get(settings._current_profile)
            default_variable.value = current
        return current  # type: ignore[return-value]

    def __setattr__(cls, name: str, value: object) -> None:
        if name == "default":
            raise AttributeError(
                "Cannot assign to the property settings.default - "
                "consider using settings.load_profile instead."
            )
        if not name.startswith("_"):
            raise AttributeError(
                f"Cannot assign hypothesis.settings.{name}={value!r} - the settings "
                "class is immutable.  You can change the global default "
                "settings with settings.load_profile, or use @settings(...) "
                "to decorate your test instead."
            )
        super().__setattr__(name, value)

    def __getattr__(cls, name: str) -> Any:
        # ``settings.max_examples`` etc. read the current default settings.
        if name in all_settings:
            return getattr(cls.default, name)
        raise AttributeError(name)

    def __repr__(cls) -> str:
        return "hypothesis.settings"


class settings(metaclass=settingsMeta):
    """Settings for Hypothesis-style tests, mapped onto Hegel's settings.

    ``deadline`` is enforced by the Python layer (per test case, with the
    final replay allowed 1.25x). ``Phase.explain`` and the health checks
    without a Hegel equivalent are accepted and ignored.
    """

    _profiles: dict[str, settings] = {}
    _current_profile: str = "default"
    _lock = threading.Lock()

    __slots__ = (
        "_fallback",
        "_in_definition",
        "backend",
        "database",
        "deadline",
        "derandomize",
        "max_examples",
        "phases",
        "print_blob",
        "report_multiple_bugs",
        "stateful_step_count",
        "suppress_health_check",
        "verbosity",
    )

    def __init__(
        self,
        parent: settings | None = None,
        *,
        max_examples: int = not_set,
        derandomize: bool = not_set,
        database: ExampleDatabase | None = not_set,
        verbosity: Verbosity = not_set,
        phases: Collection[Phase] = not_set,
        stateful_step_count: int = not_set,
        report_multiple_bugs: bool = not_set,
        suppress_health_check: Collection[HealthCheck] = not_set,
        deadline: int | float | datetime.timedelta | None = not_set,
        print_blob: bool = not_set,
        backend: str = not_set,
    ) -> None:
        object.__setattr__(self, "_in_definition", True)
        if parent is not None:
            _check_type(settings, parent, "parent")
        if derandomize not in (not_set, False) and database not in (not_set, None):
            raise InvalidArgument(
                "derandomize=True implies database=None, so passing "
                f"database={database!r} too is invalid."
            )
        base = parent if parent is not None else settings.default
        object.__setattr__(self, "_fallback", base)

        def pick(name: str, value: Any, validate: Any) -> Any:
            if value is not_set:
                return getattr(base, name) if base is not None else _DEFAULTS[name]
            return validate(value)

        self.max_examples = pick("max_examples", max_examples, _validate_max_examples)
        self.derandomize = pick(
            "derandomize", derandomize, lambda v: _validate_bool("derandomize", v)
        )
        self.database = pick("database", database, _validate_database)
        self.verbosity = pick(
            "verbosity", verbosity, lambda v: _validate_enum(Verbosity, v, "verbosity")
        )
        self.phases = pick("phases", phases, _validate_phases)
        self.stateful_step_count = pick(
            "stateful_step_count", stateful_step_count, _validate_step_count
        )
        self.report_multiple_bugs = pick(
            "report_multiple_bugs",
            report_multiple_bugs,
            lambda v: _validate_bool("report_multiple_bugs", v),
        )
        self.suppress_health_check = pick(
            "suppress_health_check", suppress_health_check, _validate_health_checks
        )
        self.deadline = pick("deadline", deadline, _validate_deadline)
        self.print_blob = pick("print_blob", print_blob, lambda v: _validate_bool("print_blob", v))
        self.backend = pick("backend", backend, _validate_backend)
        object.__setattr__(self, "_in_definition", False)

    def __setattr__(self, name: str, value: object) -> None:
        if not name.startswith("_") and not self._in_definition:
            raise AttributeError("settings objects are immutable")
        object.__setattr__(self, name, value)

    def __call__(self, test: Any) -> Any:
        """Attach these settings to a test function or state machine class."""
        if not callable(test):
            raise InvalidArgument(
                "settings objects can be called as a decorator with @given, "
                f"but decorated test={test!r} is not callable."
            )
        if inspect.isclass(test):
            from hegel.hypothesis.stateful import RuleBasedStateMachine

            if issubclass(test, RuleBasedStateMachine):
                attr_name = "_hypothesis_internal_settings_applied"
                if getattr(test, attr_name, False):
                    raise InvalidArgument(
                        "Applying the @settings decorator twice would "
                        "overwrite the first version; merge their arguments "
                        "instead."
                    )
                setattr(test, attr_name, True)
                test.TestCase.settings = self
                return test
            raise InvalidArgument(
                "@settings(...) can only be used as a decorator on "
                "functions, or on subclasses of RuleBasedStateMachine."
            )
        if hasattr(test, "_hypothesis_internal_settings_applied"):
            raise InvalidArgument(
                f"{getattr(test, '__name__', test)} has already been decorated with a "
                "settings object.\n"
                f"    Previous:  {test._hypothesis_internal_use_settings!r}\n"
                f"    This:  {self!r}"
            )
        test._hypothesis_internal_use_settings = self
        test._hypothesis_internal_settings_applied = True
        return test

    def __repr__(self) -> str:
        bits = sorted(
            f"{name}={getattr(self, name)!r}" for name in all_settings if name != "backend"
        )
        return "settings({})".format(", ".join(bits))

    def show_changed(self) -> str:
        bits = []
        default = settings._profiles["default"]
        for name in all_settings:
            value = getattr(self, name)
            if value != getattr(default, name):
                bits.append(f"{name}={value!r}")
        return ", ".join(sorted(bits, key=len))

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, settings):
            return NotImplemented
        return all(getattr(self, n) == getattr(other, n) for n in all_settings)

    def __hash__(self) -> int:
        return hash(tuple(repr(getattr(self, n)) for n in all_settings))

    # -- profiles ----------------------------------------------------------

    @staticmethod
    def register_profile(name: str, parent: settings | None = None, /, **kwargs: Any) -> None:
        """Register a named collection of settings, built on ``parent`` (or
        the default profile) with ``kwargs`` overriding."""
        _check_type(str, name, "name")
        override = getattr(_local, "override", None)
        if override is not None and override != settings._profiles.get(settings._current_profile):
            note_deprecation(
                "Cannot register a settings profile when the current settings differ "
                "from the current profile (usually due to an @settings decorator). "
                "Register profiles at module level instead.",
                since="2025-11-15",
                has_codemod=False,
                stacklevel=1,
            )
        base = parent if parent is not None else settings._profiles.get("default")
        with settings._lock:
            # Pass the parent straight through when nothing overrides it, so
            # the registered profile falls back to it directly.
            settings._profiles[name] = (
                parent if parent is not None and not kwargs else settings(base, **kwargs)
            )
        if settings._current_profile == name:
            settings.load_profile(name)

    @staticmethod
    def get_current_profile_name() -> str:
        return settings._current_profile

    @staticmethod
    def get_profile(name: str) -> settings:
        _check_type(str, name, "name")
        try:
            return settings._profiles[name]
        except KeyError:
            raise InvalidArgument(f"Profile {name!r} is not registered") from None

    @staticmethod
    def load_profile(name: str) -> None:
        """Make the named profile the default for tests without their own."""
        _check_type(str, name, "name")
        with settings._lock:
            profile = settings.get_profile(name)
            settings._current_profile = name
            default_variable.value = profile

    # -- mapping to Hegel ---------------------------------------------------

    def to_hegel(self) -> hegel_settings.Settings:
        """The Hegel settings this object corresponds to."""
        if self.database is None:
            database: str | None = ""
        else:
            database = self.database._hegel_path or None
        phases = [_HEGEL_PHASES[p] for p in self.phases if p in _HEGEL_PHASES]
        suppressed = [
            _HEGEL_HEALTH_CHECKS[h] for h in self.suppress_health_check if h in _HEGEL_HEALTH_CHECKS
        ]
        backend = (
            hegel_settings.Backend.urandom
            if self.backend == "urandom"
            else hegel_settings.Backend.default
        )
        return hegel_settings.Settings(
            test_cases=self.max_examples,
            derandomize=self.derandomize,
            database=database,
            verbosity=hegel_settings.Verbosity[self.verbosity.name],
            phases=phases,
            suppress_health_check=suppressed,
            report_multiple_failures=self.report_multiple_bugs,
            print_blob=self.print_blob,
            backend=backend,
        )


@contextlib.contextmanager
def local_settings(s: settings) -> Iterator[settings]:
    """Make ``s`` the default settings on this thread for the block (as a
    running test's settings are)."""
    previous = getattr(_local, "override", None)
    _local.override = s
    try:
        yield s
    finally:
        _local.override = previous


def _validate_step_count(value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise InvalidArgument(f"stateful_step_count={value!r} must be a positive integer")
    return value


def _validate_backend(value: Any) -> str:
    if not isinstance(value, str):
        raise InvalidArgument(f"backend={value!r} must be a string")
    if value not in ("hypothesis", "hegel", "urandom"):
        raise InvalidArgument(
            f"backend={value!r} is not available - choose one of ['hegel', 'hypothesis', 'urandom']"
        )
    return value


_DEFAULTS: dict[str, Any] = {
    "max_examples": 100,
    "derandomize": False,
    "database": DirectoryBasedExampleDatabase(os.path.join(os.getcwd(), ".hegel", "examples")),
    "verbosity": Verbosity.normal,
    "phases": tuple(Phase),
    "stateful_step_count": 50,
    "report_multiple_bugs": True,
    "suppress_health_check": (),
    "deadline": datetime.timedelta(milliseconds=200),
    "print_blob": False,
    "backend": "hypothesis",
}


class _DefaultDatabase(DirectoryBasedExampleDatabase):
    """Hegel's own default database location, decided by the engine."""

    def __init__(self) -> None:
        self.path = None  # type: ignore[assignment]
        self._hegel_path = ""

    def __repr__(self) -> str:
        return "DirectoryBasedExampleDatabase(<hegel default>)"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, _DefaultDatabase)

    def __hash__(self) -> int:
        return 0


_DEFAULTS["database"] = _DefaultDatabase()

settings._profiles["default"] = settings()
settings.register_profile(
    "ci",
    derandomize=True,
    database=None,
    deadline=None,
    print_blob=True,
    suppress_health_check=[HealthCheck.too_slow],
)
if is_in_ci():  # nocov
    settings.load_profile("ci")
