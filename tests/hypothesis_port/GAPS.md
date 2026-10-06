# Differences from Hypothesis found while porting its test suite

Each entry: the Hypothesis behaviour, what Hegel does instead, and whether
the gap is in the compatibility layer (fixable here) or in the engine.

## Core and settings (agent B)

Files: test_core, test_testdecorators, test_given_error_conditions,
test_explicit_examples, test_example, test_find, test_control, test_settings,
test_phases, test_health_checks, test_deadline, test_flakiness,
test_reproduce_failure, test_seed_printing, test_reporting, test_verbosity,
test_failing_test_case_output, test_debug_information, test_error_in_draw,
test_draw_values, test_arbitrary_data, test_composite,
test_composite_kwonlyargs, test_targeting, test_statistical_events,
test_detection, test_executors, test_setup_teardown, test_unittest, test_mock,
test_regressions, test_validation, test_slippage, test_threading,
test_posonly_args_py38, test_exceptiongroup, test_escalation,
test_traceback_elision, test_runner_strategy, test_caching,
test_interactive_example, test_sideeffect_warnings, test_lazy_import,
test_compat, test_database_backend, test_filestorage, test_replay_logic,
test_asyncio.

### Failure reporting

- Hypothesis prints `Falsifying example: test(x=0)` as a call. Hegel adds one
  `__notes__` entry (and output line) `Falsifying example:` followed by
  `name = value` lines; `data.draw(..., label=...)` values appear as
  `label = value` and positional varargs are not shown. Tests assert on
  `"\n".join(err.__notes__)` instead of the call form. (Compat layer; the
  format is produced by `hegel.runner`.)
- Hypothesis logs `Trying example: ...`; Hegel's verbose output says
  `Running test case` / `Test case passed` / `Starting phase: Generate` and
  `Trying explicit example: ...` for explicit examples. (Engine output.)
- Notes made with `note()` are emitted after the test body's own output and
  only in the falsifying report, never interleaved with it. (Engine.)
- The reproducer note is rewritten to `@reproduce_failure('<version>', b'...')`
  for tests, but stdout keeps Hegel's `@hegel.reproduce_failure("blob")` form,
  and the blob is an opaque engine blob rather than an encoded choice
  sequence, so no decoding tests apply. (Engine.)
- No `Highest target score:` line is reported with multiple failures.
  (Engine: no target-score bookkeeping is exposed.)
- Hegel never prints an `@seed(...)` hint, so the seed-printing tests are
  skipped or xfail. `core.global_force_seed` is honoured as the seed when no
  `@seed` is given. (Compat layer; nothing to print until the engine exposes
  the seed.)
- `InvalidArgument` is not a `TypeError` subclass (`hegel._native.InvalidArgument`).

### Running tests

- A finite search space is not detected as exhausted early; Hegel keeps
  running up to `max_examples`, and an exhausted space surfaces as a health
  check failure rather than silently stopping. (Engine.)
- When every input is rejected and `filter_too_much` is suppressed, the test
  passes silently instead of raising `Unsatisfiable`. (Engine.)
- `deadline` is enforced in the Python layer: 1.25x the deadline while
  generating and shrinking, the deadline itself on the final replay, with
  garbage-collection time and `data.draw()` time excluded. Hegel re-executes
  shrink candidates instead of using a tree cache, so timing-dependent tests
  (`test_keeps_you_well_above_the_deadline`) are skipped. (Compat layer.)
- `HealthCheck.too_slow` only counts engine time: slow Python-side strategy
  work (`.map(sleep)`) does not trigger it. (Engine.)
- `UnsatisfiedAssumption` is a `BaseException` (it is Hegel's `AssumeFailed`),
  so it cannot be nested inside a `FlakyFailure` group; the flaky report
  describes it in a note instead. (Compat layer.)
- After a flaky failure the engine keeps generating and replays the flaky
  example last; a flaky failure among several bugs aborts the run with
  `Flaky` rather than being reported alongside the others. (Engine.)
- Non-deterministic data generation is reported by the engine as a `RunError`
  ("Your data generation is non-deterministic..."), mapped to
  `FlakyStrategyDefinition` only where the compat layer sees it. (Engine.)
- There is no five-minute shrinking cutoff or "exited early because it took
  too long to shrink" message. (Engine.)
- `ReadOnlyDatabase` shares the wrapped database's directory and does not
  block writes; `ExampleDatabase()` is a bare base class rather than a
  deprecated alias for the in-memory or directory database. (Compat layer.)
- The database directory layout is `<db>/<hash(key)>/<hash(value)>` plus a
  second directory recording the test's name; tests filter that entry out of
  `database.data`. There are no `.pareto` or secondary keys. (Engine.)
- No `hypothesis.configuration` module: no settable home directory, no
  `.gitignore` written into the storage directory. (Compat layer, if wanted.)

### Strategies and helpers touched by these files

- Strategies validate eagerly at construction, so invalid-argument tests
  move the construction inside `pytest.raises`. `recursive()` does not
  validate its `extend` function and `text()` does not warn about codecs or a
  strategy's `__bool__`. (Compat layer.)
- `.example()` is minimal and deterministic, with no "non-interactive use"
  warning and no refusal inside a test. (Compat layer.)
- Strategy objects are not cached by argument, so identity tests are xfail.
- No statistics collector (`hegel.hypothesis.statistics`), no import-time
  side-effect tracking (`_hypothesis_globals`), and no
  `hypothesis.internal.compat` helpers: those modules' tests are skipped.
- `RuleBasedStateMachine` does not note the `draw` made inside a rule, so the
  stateful flakiness test is xfail (the stateful agent's area).
