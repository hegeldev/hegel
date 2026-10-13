# Hegel for Python compared with Hypothesis

This page records what Hegel for Python has, what it does differently, and
what it lacks, relative to Hypothesis 6.168. `docs/engine-issues.md` lists the
engine bugs found along the way, and `tests/hypothesis_port` is Hypothesis's
own test suite ported to the Hegel API, with the tests of behaviour Hegel does
differently marked `xfail` and a note per file of what was dropped.

## Same job, different shape

| Hypothesis | Hegel | Notes |
| --- | --- | --- |
| `@given(st.integers())` on `def test(x)` | `@hegel.test` on `def test(tc)`, `x = tc.draw(gs.integers())` | Draws happen in the body; names come from the assignment. |
| `strategies` | `generators` | `map`, `filter`, `flat_map`, `one_of`/`\|` and `composite` exist in both. |
| `@st.composite` with `draw` first | `@hegel.composite` with the test case first | The body draws with `tc.draw`. |
| `assume`, `note`, `event`, `target` as functions | `tc.assume`, `tc.note`, `tc.event`, `tc.target` | The test case is explicit rather than thread-local. |
| `data()` | the test case itself | Interactive draws are the normal way to draw. |
| `@example(...)` | `@hegel.explicit_test_case(...)` | Positional values feed draws in order; keywords match draw names. |
| `@reproduce_failure(version, blob)` | `@hegel.reproduce_failure(blob)` | The blob is Hegel's, printed when `print_blob` is on. |
| `@settings(...)`, profiles | `@hegel.test(...)` keywords, `hegel.Settings` profiles | `max_examples` is `test_cases`, `report_multiple_bugs` is `report_multiple_failures`, `database=None` is `database=""`. Engine profiles `default`, `ci` and `workload` exist. |
| `RuleBasedStateMachine`, `Bundle` | `hegel.StateMachine`, `hegel.Machine`, `hegel.Pool` | Rules take the test case and draw their own arguments; a precondition is `tc.assume` at the top of a rule. |
| `find(strategy, condition)` | none; write a test | A test whose failure is finding the value gets it shrunk. |
| `dictionaries`, `fixed_dictionaries` | `dicts`, `fixed_dicts` | |
| `from_regex(pattern)` matches anywhere | `gs.from_regex(pattern)` matches the whole string | Hegel defaults to `fullmatch=True`, as the Rust library does. |
| `is_hypothesis_test(f)`, `f.hypothesis.inner_test` | `f.is_hegel_test`, `f.hegel_inner_test` | |
| `InvalidArgument`, `Unsatisfiable`, `FailedHealthCheck`, `DidNotReproduce`, `StopTest` | the same names in `hegel` | `InvalidArgument` is not a `TypeError` subclass. |
| `Flaky` and its subclasses | `hegel.Flaky` | Only under `nondeterminism_strictness="error"`; see below. |
| `MultipleFailures` | `hegel.MultipleFailures` | An `ExceptionGroup`. |
| `UnsatisfiedAssumption` | `hegel.AssumeFailed` | A `BaseException`. |

## Hegel has, Hypothesis lacks

- Concurrent stateful testing: `max_concurrency > 1` runs rules from several
  threads in engine-scheduled rounds, with per-worker output attribution.
- `tc.repeat(body)`: engine-sized repetition of a block of a test.
- Nondeterministic-run handling: when a test's behaviour changes under
  replay (a concurrent machine, say), the engine confirms each failure by
  repeated replay and reports it with a caveat describing how reliably it
  reproduced. `nondeterminism_strictness` makes that silent (the default), a
  one-line notice, or a `Flaky` error, which is Hypothesis's behaviour.
- Statistics for every run (`show_statistics`), including `event_value`
  numeric observations.
- The `urandom` backend for workload-style runs.
- An engine shared with the Rust, Go, TypeScript and other bindings, so the
  example database and reproduce blobs behave the same everywhere.

## Hypothesis has, Hegel for Python lacks

- `deadline`: Hegel's engine has no deadline concept, and `HealthCheck.too_slow`
  counts engine time only, so slow Python-side generator work does not
  trigger it.
- The health checks `return_value`, `function_scoped_fixture`, `nested_given`
  (a Hegel test called from inside another is an `InvalidArgument` instead),
  `differing_executors`, `not_a_test_method`, `data_too_large` and
  `large_base_example`.
- `find()`, `.example()`, `@seed` (set `seed` in the settings instead),
  `register_random` and the per-test-case seeding of the global `random`.
- The strategies `shared`, `runner`, `slices`, `functions`, `randoms`,
  `random_module`, `decimals`, `fractions`, `complex_numbers`,
  `timezone_keys`, and `floats(width=16)`.
- `@precondition`, `@initialize`, `consumes`, `multiple` and `Bundle`
  semantics for state machines, `stateful_step_count` and a minimum step
  count, and `MachineClass.TestCase`. A pool shrinks toward the first value
  added, where a bundle prefers the most recent.
- The key/value example database API (`save`/`fetch`/`move`, `.pareto` and
  secondary keys, `ReadOnlyDatabase`, `InMemoryExampleDatabase`): Hegel's
  database is an opaque directory, `database=""` disables it, and
  `Phase.reuse` replays it.
- `Phase.explain` (the explain phase's comments on which parts of an input
  matter), `settings.backend` providers other than Hegel's own
  (`crosshair`, ...), `fuzz_one_input`, observability callbacks and the
  `HYPOTHESIS_EXPERIMENTAL_OBSERVABILITY` output.
- The `hypothesis.extra` packages (numpy, pandas, django, pytz, dateutil,
  lark, redis, ...), the ghostwriter, the CLI and `hypothesis.statistics`.
- `from_type` of most of what Hypothesis's type lookup covers beyond value
  types, generics, enums, dataclasses, TypedDicts, protocols' registered
  implementations and annotated constructors: builtins without annotations
  (exceptions, `range`, `slice`, ...), typing special forms such as
  `Callable[...]` and `Final`, and forward references found in caller
  frames. Recursive types are resolved eagerly rather than deferred, and a
  `Protocol` with no resolvable subclass is `ResolutionFailed` when the
  generator is built rather than a `TypeError` when drawn.
  `characters(codec=...)` beyond ascii, latin-1 and utf-8.
- Lambda source in reprs: a filtered or mapped generator reprs with the
  function's `__name__`, so `<lambda>` rather than the lambda's text.
- Strategy objects cached by argument: two calls of `gs.integers()` are two
  objects.
- The pytest plugin's `--hypothesis-*` options and statistics in the terminal
  summary; Hegel's plugin has `--hegel-profile`, `--hegel-seed`,
  `--hegel-verbosity`, `--hegel-derandomize` and `--hegel-database`.

## Different because the engine behaves differently

- The failing example is printed as `name = value` lines rather than a call,
  verbose output says `Running test case` rather than `Trying example`, and
  the drawn values and `note()` lines form one document that the binding
  renders after the final replay, in draw order, so they do not interleave
  with anything the test itself prints to stdout. See "Output format" below.
- Reproduce blobs are opaque engine blobs, printed as
  `@hegel.reproduce_failure("...")`. No `@seed(...)` hint is printed, and no
  `Highest target score` line.
- `target()` steers generation, but the engine only starts targeting halfway
  through the run's budget, so a target-driven failure takes more test cases
  to find than in Hypothesis (see `docs/engine-issues.md`).
- By default a failure that does not reproduce on replay is confirmed by
  repeated replay and reported with a caveat, rather than aborting the run
  as `Flaky`; non-deterministic data generation is reported as a run error.
  A failure that reproduces only once is dropped once another failure is
  confirmed (see `docs/engine-issues.md`).
- A finite search space is not detected as exhausted early; the engine runs
  up to `test_cases` and does not suppress duplicate inputs. There is no
  shrinking time limit.
- Shrinking differs in places: Hegel does not rewrite a `sampled_from` branch
  of a `one_of` into the wide branch (Hypothesis's widening pass; see
  `docs/engine-issues.md`), and the bounded integer distribution is weighted
  toward boundaries. `integers(0, 1e100)` draws many more huge values than
  Hypothesis does.
- A skipped test ends the run before anything is saved to the database.
- Generator arguments are validated when the generator is built, not when
  the test runs.
- `sets(sampled_from(...), min_size=n)` and the keys of a `dicts` do not
  sample without replacement.
- `datetimes()` and `times()` never set `fold`, and there is no targeting of
  DST folds, gaps or leap seconds.
- `from_regex` depends on the engine's regex generator; see
  `docs/engine-issues.md` for where it departs from Python's `re`. Patterns
  that can match nothing, and character classes with nothing in the
  alphabet, are not reported up front.
- Deeply self-referential `recursive(deferred(...))` generators hit Python's
  recursion limit.
- A test whose every input is rejected fails the `FilterTooMuch` health
  check (or passes quietly when that check is suppressed) rather than
  raising `Unsatisfiable`.
- `Pool.values_consumed()` removes a value from the pool as soon as it is
  drawn, so a rule rejected by `tc.assume` afterwards, or a `.filter` on the
  consumed values, loses values that a Hypothesis `consumes(bundle)` would
  keep.

## Output format

Hegel prints a failing input as the draws it made, one per line, and attaches
the same text to the exception as a note:

```
Falsifying example:
xs = [0, 0]
```

Hypothesis prints the test call, `test_f(xs=[0, 0])`.
