"""Run configuration: settings, enums, and profiles."""

from __future__ import annotations

import dataclasses
import enum
from collections.abc import Iterable
from dataclasses import dataclass
from typing import Any

from hegel import _native

__all__ = [
    "Backend",
    "HealthCheck",
    "Phase",
    "Settings",
    "TestLocation",
    "Verbosity",
]


class Verbosity(enum.Enum):
    """How much output Hegel produces during a run."""

    quiet = _native.VERBOSITY_QUIET
    normal = _native.VERBOSITY_NORMAL
    verbose = _native.VERBOSITY_VERBOSE
    debug = _native.VERBOSITY_DEBUG


class Phase(enum.Enum):
    """The phases of a run. By default all of them run."""

    explicit = _native.PHASE_EXPLICIT
    reuse = _native.PHASE_REUSE
    generate = _native.PHASE_GENERATE
    target = _native.PHASE_TARGET
    shrink = _native.PHASE_SHRINK

    @classmethod
    def all(cls) -> frozenset[Phase]:
        return frozenset(cls)


class HealthCheck(enum.Enum):
    """Health checks that can be suppressed."""

    filter_too_much = _native.HC_FILTER_TOO_MUCH
    too_slow = _native.HC_TOO_SLOW
    test_cases_too_large = _native.HC_TEST_CASES_TOO_LARGE
    large_initial_test_case = _native.HC_LARGE_INITIAL_TEST_CASE

    @classmethod
    def all(cls) -> frozenset[HealthCheck]:
        return frozenset(cls)


class Backend(enum.Enum):
    """Where the engine's randomness comes from."""

    default = _native.BACKEND_DEFAULT
    urandom = _native.BACKEND_URANDOM


@dataclass(frozen=True)
class TestLocation:
    """Where a test is defined, recorded in the database and statistics."""

    __test__ = False  # not a pytest test class

    file: str
    begin_line: int
    class_name: str = ""
    function: str = ""


def _flags(members: Iterable[enum.Enum]) -> int:
    mask = 0
    for member in members:
        mask |= int(member.value)
    return mask


def _unflags(cls: type[enum.Enum], mask: int) -> frozenset[Any]:
    return frozenset(member for member in cls if mask & int(member.value))


_FIELDS = (
    "test_cases",
    "verbosity",
    "seed",
    "derandomize",
    "database",
    "suppress_health_check",
    "phases",
    "report_multiple_failures",
    "show_statistics",
    "print_blob",
    "backend",
)


def _coerce_members(cls: type[enum.Enum], value: Any) -> frozenset[Any]:
    if isinstance(value, cls):
        return frozenset([value])
    return frozenset(cls(v) if not isinstance(v, cls) else v for v in value)


@dataclass(frozen=True)
class Settings:
    """Configuration for a run.

    ``Settings()`` starts from the default profile (resolved by the engine:
    ``hegel.toml``, ``HEGEL_DEFAULT_PROFILE``, the ``default`` set with
    :meth:`set_default_profile`, or the environment: ``workload`` under
    Antithesis, ``ci`` on a CI server, ``development`` otherwise), with any
    keyword arguments overriding individual fields.

    ``database`` is ``None`` for the engine default directory, ``""`` to
    disable the database, or a directory path.
    """

    test_cases: int = 100
    verbosity: Verbosity = Verbosity.normal
    seed: int | None = None
    derandomize: bool = False
    database: str | None = None
    suppress_health_check: frozenset[HealthCheck] = frozenset()
    phases: frozenset[Phase] = Phase.all()
    report_multiple_failures: bool = False
    show_statistics: bool = False
    print_blob: bool = False
    backend: Backend = Backend.default

    def __init__(self, parent: Settings | None = None, **overrides: Any) -> None:
        base = _from_native(_native.Settings.new()) if parent is None else parent
        values = {name: getattr(base, name) for name in _FIELDS}
        for name, value in overrides.items():
            if name not in _FIELDS:
                raise TypeError(f"Settings() got an unexpected keyword argument {name!r}")
            values[name] = value
        _assign(self, values)

    @classmethod
    def from_profile(cls, name: str, **overrides: Any) -> Settings:
        """Settings initialised from the named profile."""
        return cls(_from_native(_native.Settings.for_profile(name)), **overrides)

    @staticmethod
    def register_profile(name: str, settings: Settings | None = None, **overrides: Any) -> None:
        """Register (or replace) a named profile, from ``settings`` (the
        default profile when omitted) with ``overrides`` applied."""
        base = Settings() if settings is None else settings
        _native.register_profile(name, Settings(base, **overrides)._build())

    @staticmethod
    def set_default_profile(name: str) -> None:
        """Make the named profile the one ``Settings()`` resolves."""
        _native.set_default_profile(name)

    @staticmethod
    def load_profile(name: str) -> None:
        """Alias for :meth:`set_default_profile`."""
        _native.set_default_profile(name)

    def replace(self, **overrides: Any) -> Settings:
        return Settings(self, **overrides)

    def _build(
        self,
        database_key: str | None = None,
        test_location: TestLocation | None = None,
    ) -> _native.Settings:
        """A native settings handle: every field set explicitly on top of
        the immutable ``base`` profile."""
        native = _native.Settings.for_profile("base")
        native.set_test_cases(self.test_cases)
        native.set_verbosity(self.verbosity.value)
        native.set_seed(self.seed)
        native.set_derandomize(self.derandomize)
        native.set_report_multiple_failures(self.report_multiple_failures)
        native.set_show_statistics(self.show_statistics)
        native.set_database(self.database)
        native.set_print_blob(self.print_blob)
        if database_key is not None:
            native.set_database_key(database_key)
        if test_location is not None:
            native.set_test_location(
                test_location.file,
                test_location.begin_line,
                test_location.class_name,
                test_location.function,
            )
        native.set_phases(_flags(self.phases))
        native.set_suppress_health_check(_flags(self.suppress_health_check))
        native.set_backend(self.backend.value)
        return native


def _assign(settings: Settings, values: dict[str, Any]) -> None:
    test_cases = int(values["test_cases"])
    if test_cases < 0:
        raise _native.InvalidArgument("test_cases must be non-negative")
    seed = values["seed"]
    if seed is not None:
        seed = int(seed) & ((1 << 64) - 1)
    database = values["database"]
    if database is False:
        database = ""
    elif database is not None:
        database = str(database)
    object.__setattr__(settings, "test_cases", test_cases)
    object.__setattr__(settings, "verbosity", Verbosity(values["verbosity"]))
    object.__setattr__(settings, "seed", seed)
    object.__setattr__(settings, "derandomize", bool(values["derandomize"]))
    object.__setattr__(settings, "database", database)
    object.__setattr__(
        settings,
        "suppress_health_check",
        _coerce_members(HealthCheck, values["suppress_health_check"]),
    )
    object.__setattr__(settings, "phases", _coerce_members(Phase, values["phases"]))
    object.__setattr__(
        settings, "report_multiple_failures", bool(values["report_multiple_failures"])
    )
    object.__setattr__(settings, "show_statistics", bool(values["show_statistics"]))
    object.__setattr__(settings, "print_blob", bool(values["print_blob"]))
    object.__setattr__(settings, "backend", Backend(values["backend"]))


def _from_native(native: _native.Settings) -> Settings:
    settings = object.__new__(Settings)
    _assign(
        settings,
        {
            "test_cases": native.get_test_cases(),
            "verbosity": Verbosity(native.get_verbosity()),
            "seed": native.get_seed(),
            "derandomize": native.get_derandomize(),
            "database": native.get_database(),
            "suppress_health_check": _unflags(HealthCheck, native.get_suppress_health_check()),
            "phases": _unflags(Phase, native.get_phases()),
            "report_multiple_failures": native.get_report_multiple_failures(),
            "show_statistics": native.get_show_statistics(),
            "print_blob": native.get_print_blob(),
            "backend": Backend(native.get_backend()),
        },
    )
    return settings


def settings_fields() -> tuple[str, ...]:
    return tuple(f.name for f in dataclasses.fields(Settings))
