"""Run configuration: settings, enums, and profiles."""

from __future__ import annotations

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
    """How much output Hegel produces during a run.

    - ``quiet``: nothing; a failing test still raises, without the drawn
      values attached.
    - ``normal``: the default. A failing test's drawn values and notes,
      attached to the exception, and any warnings.
    - ``verbose``: also every test case's drawn values and notes as it
      runs, and the start and end of each phase.
    - ``debug``: also the engine's own diagnostics.

    Settings accept a member or its name: ``verbosity="verbose"``.
    """

    quiet = _native.VERBOSITY_QUIET
    normal = _native.VERBOSITY_NORMAL
    verbose = _native.VERBOSITY_VERBOSE
    debug = _native.VERBOSITY_DEBUG


class Phase(enum.Enum):
    """The phases of a run, in the order they run. By default all of them
    do; ``phases`` selects a subset.

    - ``explicit``: run the explicit test cases added with
      :func:`~hegel.explicit_test_case`.
    - ``reuse``: replay failures saved in the example database.
    - ``generate``: generate new test cases.
    - ``target``: steer generation toward high :meth:`~hegel.TestCase.target`
      scores.
    - ``shrink``: shrink a failing test case to a minimal one. Without it,
      the first failure found is reported as it is.

    ``Phase.all()`` is the set of every phase.
    """

    explicit = _native.PHASE_EXPLICIT
    reuse = _native.PHASE_REUSE
    generate = _native.PHASE_GENERATE
    target = _native.PHASE_TARGET
    shrink = _native.PHASE_SHRINK

    @classmethod
    def all(cls) -> frozenset[Phase]:
        return frozenset(cls)


class HealthCheck(enum.Enum):
    """Health checks: proactive errors raised when a test is likely to test
    poorly or too slowly. A failing check raises
    :class:`~hegel.FailedHealthCheck`; ``suppress_health_check`` turns
    checks off for a test whose behaviour is intentional.

    - ``filter_too_much``: too many test cases are rejected, by
      :meth:`~hegel.TestCase.assume` or
      :meth:`~hegel.generators.Generator.filter`.
    - ``too_slow``: the first test cases take too long to run: more than
      30 seconds in all before ten valid ones, counting the test body and
      its generators.
    - ``test_cases_too_large``: test cases routinely reach the limit on the
      number of choices one may make. Suppressing it also removes that
      limit, as the ``unbounded_choices`` setting does.
    - ``large_initial_test_case``: the smallest possible test case is
      already very large.

    ``HealthCheck.all()`` is the set of every check.
    """

    filter_too_much = _native.HC_FILTER_TOO_MUCH
    too_slow = _native.HC_TOO_SLOW
    test_cases_too_large = _native.HC_TEST_CASES_TOO_LARGE
    large_initial_test_case = _native.HC_LARGE_INITIAL_TEST_CASE

    @classmethod
    def all(cls) -> frozenset[HealthCheck]:
        return frozenset(cls)


class Backend(enum.Enum):
    """Where the engine's randomness comes from.

    - ``default``: a seeded pseudo-random generator, so runs are
      reproducible from ``seed`` and shrinking and replay work as usual.
    - ``urandom``: fresh entropy from outside the process on every draw,
      for running under `Antithesis <https://antithesis.com/>`_, whose
      fuzzer then controls every choice. The ``workload`` profile selects
      it.
    """

    default = _native.BACKEND_DEFAULT
    urandom = _native.BACKEND_URANDOM


class NondeterminismStrictness(enum.Enum):
    """How a run reacts to a test whose behaviour changes when the same
    choices are replayed (a test depending on timing, threads, or state
    left over from an earlier test case).

    - ``quiet``: the default. Switch silently to confirming each failure
      by repeated replay; the report carries a caveat describing how
      reliably it reproduced.
    - ``warn``: the same, with a one-line notice when the switch happens.
    - ``error``: abort the run with :class:`~hegel.Flaky`, as Hypothesis
      does.
    """

    quiet = _native.NONDETERMINISM_QUIET
    warn = _native.NONDETERMINISM_WARN
    error = _native.NONDETERMINISM_ERROR


@dataclass(frozen=True)
class TestLocation:
    """Where a test is defined: the file, first line, class and function
    name. :func:`hegel.test` records it automatically; :func:`hegel.run`
    takes one to identify the test in the engine's reports."""

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


def _show(value: Any) -> str:
    if not isinstance(value, frozenset):
        return repr(value)
    if not value:
        return "frozenset()"
    members = sorted(value, key=lambda member: int(member.value))
    return f"frozenset({{{', '.join(repr(member) for member in members)}}})"


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


class Settings:
    """Configuration for a run. Immutable.

    A ``Settings`` value holds every field. ``Settings(**overrides)``
    resolves the default profile when it is created and keeps a copy of
    the result, with the given fields set over it: the ``default`` set with
    :meth:`set_default_profile`, ``HEGEL_DEFAULT_PROFILE``, the ``default``
    entry in ``hegel.toml``, or the environment: ``workload`` under
    Antithesis, ``ci`` on a CI server, ``development`` otherwise. The
    ``HEGEL_*`` environment variables (``HEGEL_TEST_CASES``, ``HEGEL_SEED``,
    ...) apply over the profile, and the given fields over both. A
    malformed ``hegel.toml`` or variable, or an unknown default profile,
    is an :class:`~hegel.InvalidArgument` raised here. The copy does not
    change afterwards: a ``Settings`` made before the default profile
    changes, at import time in a ``conftest.py`` say, keeps the values it
    was made with, so make it after :meth:`register_profile` and
    :meth:`set_default_profile`, or give :func:`hegel.test` its fields as
    keywords, which it resolves each time the test runs.
    ``Settings(parent, **overrides)`` copies ``parent``'s fields and sets
    ``overrides`` over them.

    Usually settings are given to :func:`hegel.test` as keywords
    (``@hegel.test(test_cases=500)``), which override the same fields.

    The fields:

    ``test_cases``
        How many test cases to run (default 100). Rejected test cases do
        not count.
    ``verbosity``
        How much Hegel prints: a :class:`Verbosity` (default ``normal``).
    ``seed``
        An integer seed for a reproducible run, or ``None`` (the default)
        for a fresh random seed each run.
    ``derandomize``
        Derive a fixed seed from the test, so every run of it is the same
        (default ``False``; the ``ci`` profile sets it). A ``seed`` takes
        precedence.
    ``database``
        Where failing examples are saved and replayed from: ``None`` for
        the engine's default directory (``.hegel/examples``), a directory
        path, or ``""`` (or ``False``) to disable the database. Note that
        ``None`` does not disable it, unlike in the Rust library and
        Hypothesis.
    ``suppress_health_check``
        The :class:`HealthCheck` members not to fail the run on (default
        none).
    ``phases``
        The :class:`Phase` members to run (default all).
    ``report_multiple_failures``
        Report every distinct failure found, as a
        :class:`~hegel.MultipleFailures`, rather than only the smallest
        (default ``False``).
    ``stop_on_first_failure``
        Stop generating once a failure is found (default ``True``). When
        ``False``, the run uses its whole ``test_cases`` budget before
        reporting; every failure found is saved to the database, and with
        ``report_multiple_failures`` off only the smallest is shrunk and
        reported.
    ``show_statistics``
        Print the end-of-run statistics of the events recorded with
        :meth:`~hegel.TestCase.event` and
        :meth:`~hegel.TestCase.event_value` (default ``False``).
    ``unbounded_choices``
        Let a test case make any number of choices (default ``False``).
        Otherwise a test case that makes more than 2**20 choices (draws,
        and the steps of collections and stateful runs) is stopped and
        discarded. A long-running test case, such as a concurrent state
        machine driven for hours, needs the limit removed; the cost is
        the memory to record every choice. Suppressing
        ``HealthCheck.test_cases_too_large`` removes the limit too.
    ``print_blob``
        On failure, say how to replay it with
        :func:`~hegel.reproduce_failure` (default ``True``).
    ``backend``
        The source of randomness: a :class:`Backend` (default
        ``default``).
    ``nondeterminism_strictness``
        How to react to a test that behaves differently on replay: a
        :class:`NondeterminismStrictness` (default ``quiet``).

    Enum-valued fields take members (``HealthCheck.filter_too_much``) or
    their names (``"filter_too_much"``); the set-valued fields take one
    or a collection of them. A value of the wrong type is an
    :class:`~hegel.InvalidArgument`, and an unknown field a
    :class:`TypeError`.

    ::

        thorough = hegel.Settings(test_cases=10_000, suppress_health_check=["too_slow"])

        @hegel.test(settings=thorough)
        def test_something(tc): ...
    """

    __slots__ = ("_values",)

    test_cases: int
    verbosity: Verbosity
    seed: int | None
    derandomize: bool
    database: str | None
    suppress_health_check: frozenset[HealthCheck]
    phases: frozenset[Phase]
    report_multiple_failures: bool
    stop_on_first_failure: bool
    show_statistics: bool
    unbounded_choices: bool
    print_blob: bool
    backend: Backend
    nondeterminism_strictness: NondeterminismStrictness

    _values: dict[str, Any]

    def __init__(self, parent: Settings | None = None, **overrides: Any) -> None:
        if parent is not None and not isinstance(parent, Settings):
            raise InvalidArgument(f"parent={parent!r} must be a Settings object or None")
        values = _read_native(_native.Settings.new()) if parent is None else dict(parent._values)
        for name, value in overrides.items():
            if name not in _FIELDS:
                raise TypeError(f"Settings() got an unexpected keyword argument {name!r}")
            values[name] = _VALIDATORS[name](name, value)
        object.__setattr__(self, "_values", values)

    def __getattr__(self, name: str) -> Any:
        if name not in _FIELDS:
            raise AttributeError(f"'Settings' object has no attribute {name!r}")
        return self._values[name]

    def __setattr__(self, name: str, value: Any) -> None:
        raise AttributeError(f"Settings are immutable: use replace() to change {name!r}")

    def __delattr__(self, name: str) -> None:
        raise AttributeError(f"Settings are immutable: cannot delete {name!r}")

    def __reduce__(self) -> tuple[Any, ...]:
        return (_restore_settings, (dict(self._values),))

    def _key(self) -> tuple[Any, ...]:
        return tuple(self._values[name] for name in _FIELDS)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Settings):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())

    def __repr__(self) -> str:
        fields = ", ".join(f"{name}={_show(self._values[name])}" for name in _FIELDS)
        return f"Settings({fields})"

    @classmethod
    def from_profile(cls, name: str, **overrides: Any) -> Settings:
        """Settings resolved, now, from the named profile rather than the
        default one, with ``overrides`` applied. The profile is one of the
        engine's (``development``, ``ci``, ``workload``, or ``base`` for
        the plain defaults), one defined in ``hegel.toml``, or one
        registered with :meth:`register_profile`; an unknown name is an
        error. Unlike ``Settings()``, this ignores which profile is the
        default."""
        _check_profile_name(name)
        return cls(_restore_settings(_read_native(_native.Settings.for_profile(name))), **overrides)

    @staticmethod
    def register_profile(name: str, settings: Settings | None = None, **overrides: Any) -> None:
        """Register (or replace) a named profile, for the whole process:
        ``settings`` (the default profile when omitted) with ``overrides``
        applied, resolved now. The profile can then be selected with
        :meth:`from_profile`, :meth:`set_default_profile`,
        ``HEGEL_DEFAULT_PROFILE`` or pytest's ``--hegel-profile``.

        ::

            hegel.Settings.register_profile("nightly", test_cases=10_000)
            hegel.Settings.set_default_profile("nightly")

        Registering a profile is not retroactive: a ``Settings`` value
        already made keeps its fields, so register profiles before making
        the settings that should start from them. The reserved names
        ``base`` and ``default`` cannot be registered.
        """
        _check_profile_name(name)
        if settings is not None and not isinstance(settings, Settings):
            raise InvalidArgument(f"settings={settings!r} must be a Settings object or None")
        base = Settings() if settings is None else settings
        _native.register_profile(name, Settings(base, **overrides)._build())

    @staticmethod
    def set_default_profile(name: str | None) -> None:
        """Make the named (already registered) profile the one
        ``Settings()`` resolves, for the whole process. ``None`` restores
        the engine's own choice (see :class:`Settings`). An unknown
        profile name is an error. Not retroactive: a ``Settings`` value
        already made keeps its fields."""
        if name is not None:
            _check_profile_name(name)
            _native.Settings.for_profile(name)
        _native.set_default_profile(name)

    def replace(self, **overrides: Any) -> Settings:
        """A copy of these settings with ``overrides`` applied."""
        return Settings(self, **overrides)

    def _build(
        self,
        database_key: str | None = None,
        test_location: TestLocation | None = None,
    ) -> _native.Settings:
        """A native settings handle with every field of these settings set
        on it."""
        native = _native.Settings.new()
        for name in _FIELDS:
            _SETTERS[name](native, self._values[name])
        if database_key is not None:
            native.set_database_key(database_key)
        if test_location is not None:
            native.set_test_location(
                test_location.file,
                test_location.begin_line,
                test_location.class_name,
                test_location.function,
            )
        return native


def _set_from_string(native: _native.Settings, key: str, value: str) -> None:
    """Set ``key`` on ``native`` as the engine reads a ``hegel.toml``
    profile: the key is the setting's name and the value is text in the
    engine's vocabulary for it (``"10"``, ``"verbose"``,
    ``"too_slow,filter_too_much"``). A command line passes its options
    through this, so the engine, not this package, knows each setting's
    spellings."""
    native.set_from_string(key, value)
    if key not in _FIELDS:
        raise InvalidArgument(
            f"{key} is set for each test, not for a whole run, so it cannot be set here"
        )


def _check_test_cases(field: str, value: Any) -> int:
    test_cases = _check_int(field, value)
    if test_cases < 0:
        raise InvalidArgument(f"test_cases={test_cases} must be non-negative")
    if test_cases >= 1 << 64:
        raise InvalidArgument(f"test_cases={test_cases} must be less than 2**64")
    return test_cases


def _check_seed(field: str, value: Any) -> int | None:
    if value is None:
        return None
    seed = _check_int(field, value)
    if not 0 <= seed < 1 << 64:
        raise InvalidArgument(f"seed={seed} must be between 0 and 2**64 - 1")
    return seed


def _check_database(field: str, value: Any) -> str | None:
    if value is False:
        return ""
    if isinstance(value, (str, os.PathLike)):
        return os.fspath(value)
    if value is not None:
        raise InvalidArgument(
            f"database={value!r} must be a directory path, None for the default, "
            'or "" to disable the database'
        )
    return None


def _member(cls: type[E]) -> Any:
    return lambda field, value: _coerce_member(cls, field, value)


def _members(cls: type[E]) -> Any:
    return lambda field, value: _coerce_members(cls, field, value)


_VALIDATORS: dict[str, Any] = {
    "test_cases": _check_test_cases,
    "verbosity": _member(Verbosity),
    "seed": _check_seed,
    "derandomize": _check_bool,
    "database": _check_database,
    "suppress_health_check": _members(HealthCheck),
    "phases": _members(Phase),
    "report_multiple_failures": _check_bool,
    "stop_on_first_failure": _check_bool,
    "show_statistics": _check_bool,
    "unbounded_choices": _check_bool,
    "print_blob": _check_bool,
    "backend": _member(Backend),
    "nondeterminism_strictness": _member(NondeterminismStrictness),
}

_SETTERS: dict[str, Any] = {
    "test_cases": lambda n, v: n.set_test_cases(v),
    "verbosity": lambda n, v: n.set_verbosity(v.value),
    "seed": lambda n, v: n.set_seed(v),
    "derandomize": lambda n, v: n.set_derandomize(v),
    "database": lambda n, v: n.set_database(v),
    "suppress_health_check": lambda n, v: n.set_suppress_health_check(_flags(v)),
    "phases": lambda n, v: n.set_phases(_flags(v)),
    "report_multiple_failures": lambda n, v: n.set_report_multiple_failures(v),
    "stop_on_first_failure": lambda n, v: n.set_stop_on_first_failure(v),
    "show_statistics": lambda n, v: n.set_show_statistics(v),
    "unbounded_choices": lambda n, v: n.set_unbounded_choices(v),
    "print_blob": lambda n, v: n.set_print_blob(v),
    "backend": lambda n, v: n.set_backend(v.value),
    "nondeterminism_strictness": lambda n, v: n.set_nondeterminism_strictness(v.value),
}


def _read_native(native: _native.Settings) -> dict[str, Any]:
    return {
        "test_cases": native.get_test_cases(),
        "verbosity": Verbosity(native.get_verbosity()),
        "seed": native.get_seed(),
        "derandomize": native.get_derandomize(),
        "database": native.get_database(),
        "suppress_health_check": _unflags(HealthCheck, native.get_suppress_health_check()),
        "phases": _unflags(Phase, native.get_phases()),
        "report_multiple_failures": native.get_report_multiple_failures(),
        "stop_on_first_failure": native.get_stop_on_first_failure(),
        "show_statistics": native.get_show_statistics(),
        "unbounded_choices": native.get_unbounded_choices(),
        "print_blob": native.get_print_blob(),
        "backend": Backend(native.get_backend()),
        "nondeterminism_strictness": NondeterminismStrictness(
            native.get_nondeterminism_strictness()
        ),
    }


_FIELDS = tuple(name for name in Settings.__annotations__ if not name.startswith("_"))


def _restore_settings(values: dict[str, Any]) -> Settings:
    settings = object.__new__(Settings)
    object.__setattr__(settings, "_values", values)
    return settings
