"""Run configuration: settings, enums, and profiles."""

from __future__ import annotations

import dataclasses
import enum
import os
from collections.abc import Iterable
from dataclasses import dataclass
from typing import Any, TypeVar

from hegel import _native
from hegel.errors import InvalidArgument

E = TypeVar("E", bound=enum.Enum)

__all__ = [
    "Backend",
    "HealthCheck",
    "NondeterminismStrictness",
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


class NondeterminismStrictness(enum.Enum):
    """How a run reacts to a test whose behaviour changes when the same
    choices are replayed: silently switch to confirming failures by
    repeated replay (``quiet``, the default), do so with a one-line
    notice (``warn``), or abort the run with :class:`hegel.Flaky`
    (``error``)."""

    quiet = _native.NONDETERMINISM_QUIET
    warn = _native.NONDETERMINISM_WARN
    error = _native.NONDETERMINISM_ERROR


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


def _coerce_member(cls: type[E], field: str, value: Any) -> E:
    """``value`` as a member of ``cls``: a member, or a member's name."""
    if isinstance(value, cls):
        return value
    if isinstance(value, str) and value in cls.__members__:
        return cls[value]
    choices = ", ".join(f"{cls.__name__}.{name}" for name in cls.__members__)
    raise InvalidArgument(f"{field}={value!r} must be one of {choices}")


def _coerce_members(cls: type[E], field: str, value: Any) -> frozenset[E]:
    if isinstance(value, (cls, str)):
        return frozenset([_coerce_member(cls, field, value)])
    try:
        values = list(value)
    except TypeError:
        raise InvalidArgument(
            f"{field}={value!r} must be a collection of {cls.__name__} values"
        ) from None
    return frozenset(_coerce_member(cls, field, v) for v in values)


def _check_bool(field: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise InvalidArgument(f"{field}={value!r} must be True or False")
    return value


def _check_int(field: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise InvalidArgument(f"{field}={value!r} must be an integer")
    return value


def _check_profile_name(name: Any) -> None:
    if not isinstance(name, str):
        raise InvalidArgument(f"name={name!r} must be a string naming a profile")


@dataclass(frozen=True)
class Settings:
    """Configuration for a run.

    ``Settings()`` starts from the default profile (resolved by the engine:
    ``hegel.toml``, ``HEGEL_DEFAULT_PROFILE``, the ``default`` set with
    :meth:`set_default_profile`, or the environment: ``workload`` under
    Antithesis, ``ci`` on a CI server, ``development`` otherwise), with any
    keyword arguments overriding individual fields.

    ``database`` is ``None`` for the engine's default directory
    (``.hegel/examples``), a directory path, or ``""`` (or ``False``) to
    disable the database. Note that ``None`` does not disable it, unlike in
    the Rust library and Hypothesis.

    Enum-valued fields take members (``HealthCheck.filter_too_much``) or
    their names (``"filter_too_much"``).
    """

    # Every field is filled in by __init__, from the default profile or the
    # parent settings: these declarations give the fields and types only.
    test_cases: int
    verbosity: Verbosity
    seed: int | None
    derandomize: bool
    database: str | None
    suppress_health_check: frozenset[HealthCheck]
    phases: frozenset[Phase]
    report_multiple_failures: bool
    show_statistics: bool
    print_blob: bool
    backend: Backend
    nondeterminism_strictness: NondeterminismStrictness

    def __init__(self, parent: Settings | None = None, **overrides: Any) -> None:
        if parent is not None and not isinstance(parent, Settings):
            raise InvalidArgument(f"parent={parent!r} must be a Settings object or None")
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
        _check_profile_name(name)
        return cls(_from_native(_native.Settings.for_profile(name)), **overrides)

    @staticmethod
    def register_profile(name: str, settings: Settings | None = None, **overrides: Any) -> None:
        """Register (or replace) a named profile, from ``settings`` (the
        default profile when omitted) with ``overrides`` applied."""
        _check_profile_name(name)
        if settings is not None and not isinstance(settings, Settings):
            raise InvalidArgument(f"settings={settings!r} must be a Settings object or None")
        base = Settings() if settings is None else settings
        _native.register_profile(name, Settings(base, **overrides)._build())

    @staticmethod
    def set_default_profile(name: str | None) -> None:
        """Make the named (already registered) profile the one
        ``Settings()`` resolves, for the whole process. ``None`` restores
        the engine's own choice (see :class:`Settings`)."""
        if name is not None:
            _check_profile_name(name)
            _native.Settings.for_profile(name)  # raises for an unknown profile
        _native.set_default_profile(name)

    def replace(self, **overrides: Any) -> Settings:
        """A copy of these settings with ``overrides`` applied."""
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
        native.set_nondeterminism_strictness(self.nondeterminism_strictness.value)
        return native


def _assign(settings: Settings, values: dict[str, Any]) -> None:
    test_cases = _check_int("test_cases", values["test_cases"])
    if test_cases < 0:
        raise InvalidArgument(f"test_cases={test_cases} must be non-negative")
    if test_cases >= 1 << 64:
        raise InvalidArgument(f"test_cases={test_cases} must be less than 2**64")
    seed = values["seed"]
    if seed is not None:
        seed = _check_int("seed", seed) & ((1 << 64) - 1)
    database = values["database"]
    if database is False:
        database = ""
    elif isinstance(database, (str, os.PathLike)):
        database = os.fspath(database)
    elif database is not None:
        raise InvalidArgument(
            f"database={database!r} must be a directory path, None for the default, "
            'or "" to disable the database'
        )
    checked = {
        "test_cases": test_cases,
        "verbosity": _coerce_member(Verbosity, "verbosity", values["verbosity"]),
        "seed": seed,
        "derandomize": _check_bool("derandomize", values["derandomize"]),
        "database": database,
        "suppress_health_check": _coerce_members(
            HealthCheck, "suppress_health_check", values["suppress_health_check"]
        ),
        "phases": _coerce_members(Phase, "phases", values["phases"]),
        "report_multiple_failures": _check_bool(
            "report_multiple_failures", values["report_multiple_failures"]
        ),
        "show_statistics": _check_bool("show_statistics", values["show_statistics"]),
        "print_blob": _check_bool("print_blob", values["print_blob"]),
        "backend": _coerce_member(Backend, "backend", values["backend"]),
        "nondeterminism_strictness": _coerce_member(
            NondeterminismStrictness,
            "nondeterminism_strictness",
            values["nondeterminism_strictness"],
        ),
    }
    for name, value in checked.items():
        object.__setattr__(settings, name, value)


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
            "nondeterminism_strictness": NondeterminismStrictness(
                native.get_nondeterminism_strictness()
            ),
        },
    )
    return settings


_FIELDS = tuple(field.name for field in dataclasses.fields(Settings))
