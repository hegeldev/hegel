# Hegel for Python compared with Hypothesis

This page records what Hegel for Python has, what it does differently, and
what it lacks, relative to Hypothesis 6.168. `tests/hypothesis_port/GAPS.md`
holds the finer-grained list produced while porting Hypothesis's test suite
to `hegel.hypothesis`, and `docs/engine-issues.md` lists the engine bugs found
along the way.

## Same job, different shape

| Hypothesis | Hegel | Notes |
| --- | --- | --- |
| `@given(st.integers())` on `def test(x)` | `@hegel.test` on `def test(tc)`, `x = tc.draw(gs.integers())` | Draws happen in the body; names come from the assignment. |
| `strategies` | `generators` | `map`, `filter`, `flat_map`, `one_of`/`\|` and `composite` exist in both. |
| `assume`, `note`, `event`, `target` as functions | `tc.assume`, `tc.note`, `tc.event`, `tc.target` | The test case is explicit rather than thread-local. |
| `@example(...)` | `@hegel.explicit_test_case(...)` | Positional values feed draws in order; keywords match draw names. |
| `@reproduce_failure(version, blob)` | `@hegel.reproduce_failure(blob)` | The blob is Hegel's, printed when `print_blob` is on. |
| `@settings(...)`, profiles | `@hegel.test(...)` keywords, `hegel.Settings` profiles | Engine profiles `default`, `ci` and `workload` exist. |
| `RuleBasedStateMachine`, `Bundle` | `hegel.StateMachine`, `hegel.Machine`, `hegel.Pool` | Rules take the test case and draw their own arguments. |
| `data()` | the test case itself | Interactive draws are the normal way to draw. |
| `find(strategy, condition)` | `hegel.find_example(generator, condition)` | Both shrink. |

## Hegel has, Hypothesis lacks

- Concurrent stateful testing: `max_concurrency > 1` runs rules from several
  threads in engine-scheduled rounds, with per-worker output attribution.
- `tc.repeat(body)`: engine-sized repetition of a block of a test.
- Nondeterministic-run handling: a run that is nondeterministic (a concurrent
  machine, say) reports the failure as discovered instead of replaying it.
- Statistics for every run (`show_statistics`), including `event_value`
  numeric observations.
- The `urandom` backend for workload-style runs.
- An engine shared with the Rust, Go, TypeScript and other bindings, so the
  example database and reproduce blobs behave the same everywhere.

## Hypothesis has, Hegel for Python lacks or emulates

Emulated in `hegel.hypothesis` (the Hegel API itself does not offer them):

- `deadline`: enforced in Python per test case, with Hypothesis's 1.25x
  allowance while generating and shrinking and the deadline itself on the
  final replay; garbage-collection and `data.draw()` time are excluded.
  Hegel's engine has no deadline concept, and `HealthCheck.too_slow` counts
  engine time only, so slow Python-side strategy work does not trigger it.
- `HealthCheck.return_value`, `function_scoped_fixture`, `nested_given`,
  `differing_executors` and `not_a_test_method`: checked in Python.
  `data_too_large` and `large_base_example` are accepted and never fire.
- `floats(width=16)`: drawn as 32-bit floats rounded to 16-bit values.
- `shared`, `runner`, `slices`, `functions`, `randoms`, `random_module`,
  `decimals`, `fractions`, `complex_numbers`, `timezone_keys`: built from
  Hegel generators in Python.
- `Bundle`: one of Hegel's engine-managed pools per bundle, so the engine
  chooses and shrinks which value a rule gets (toward the first value
  added, where Hypothesis prefers the most recent). `.filter` retries a
  few times, putting back a consumed value the filter rejects, and a rule
  whose arguments cannot be drawn is skipped rather than aborting the case.
  `assume()` inside a rule body aborts the whole test case, as in Hypothesis.
- `InMemoryExampleDatabase`: a temporary directory, since Hegel's database is
  always on disk. `ReadOnlyDatabase` shares the wrapped database's directory
  and does not block writes. There is no key/value database API
  (`save`/`fetch`/`move`), no `.pareto` or secondary keys, and no
  `hypothesis.configuration` home directory.
- `UnsatisfiedAssumption` is Hegel's `AssumeFailed`, a `BaseException`, so a
  flaky assumption is described in a note rather than nested in a
  `FlakyFailure` group.
- `find()` reports a search space filtered out by a health check as
  `NoSuchExample`.

Different because the engine behaves differently:

- The failing example is printed as `name = value` lines rather than a call,
  verbose output says `Running test case` rather than `Trying example`, and
  `note()` output follows the test's own output instead of interleaving with
  it. See "Output format" below.
- Reproduce blobs are opaque engine blobs, printed as
  `@reproduce_failure('<version>', b'...')` in the exception note and as
  `@hegel.reproduce_failure("...")` on stdout. No `@seed(...)` hint is
  printed, and no `Highest target score` line: `target()` is accepted but
  generation is not steered by it.
- Failures are told apart by exception type and location, not by the cause
  chain. After a flaky failure the run aborts with `Flaky` rather than
  reporting it alongside other bugs, and non-deterministic data generation
  is reported as `FlakyStrategyDefinition` only where the compat layer sees
  the engine's error.
- A finite search space is not detected as exhausted early; the engine runs
  up to `max_examples` and does not suppress duplicate inputs. There is no
  shrinking time limit.
- Shrinking differs in places: Hegel does not rewrite a `sampled_from` branch
  of a `one_of` into the wide branch (Hypothesis's widening pass; see
  `docs/engine-issues.md`), and the bounded integer distribution is weighted
  toward boundaries. `integers(0, 1e100)` draws
  many more huge values than Hypothesis does.
- A skipped test ends the run before anything is saved to the database.
- Strategy objects are not cached by argument, and `.example()` is
  deterministic and allowed inside tests.
- The repr of a `sampled_from` of an enum lists the members, and
  `characters()` reprs without its arguments. `sets(sampled_from(...),
  min_size=n)` does not sample without replacement.
- `randoms()` yields seeded `random.Random` subclasses rather than values
  drawn from the test data, so exact boundary results cannot be targeted,
  and `random_module()` draws its seed.
- `datetimes()` and `times()` reject timezone-aware bounds and never set
  `fold`; there is no targeting of DST folds, gaps or leap seconds.
- `from_regex` depends on the engine's regex generator; see
  `docs/engine-issues.md` for where it departs from Python's `re`.
- Deeply self-referential `recursive(deferred(...))` strategies hit Python's
  recursion limit.

Not available:

- `Phase.explain` (the explain phase's comments on which parts of an input
  matter), `settings.backend` providers other than Hegel's own
  (`crosshair`, ...), `fuzz_one_input`, observability callbacks and the
  `HYPOTHESIS_EXPERIMENTAL_OBSERVABILITY` output.
- The `hypothesis.extra` packages (numpy, pandas, django, pytz, dateutil,
  lark, redis, ...), the ghostwriter and the CLI.
- `register_random`: Hegel does not manage the state of external PRNGs.
- `hypothesis.statistics` and `hypothesis.internal.*`: there is no
  statistics collector or import-time side-effect tracking.
- `from_type` of most of what Hypothesis's type lookup covers beyond value
  types, generics, enums, dataclasses, TypedDicts, protocols' registered
  implementations and annotated constructors: builtins without annotations
  (exceptions, `range`, `slice`, ...), typing special forms such as
  `Callable[...]`, `Type[...]` and `Final`, and forward references found in
  caller frames. `characters(codec=...)` beyond ascii, latin-1 and utf-8.
- Hypothesis's typed `SearchStrategy` protocol: `hegel.hypothesis` strategies
  are Hegel generators (`SearchStrategy` is an alias of `hegel.generators.Generator`),
  so `.do_draw` receives a `TestCase` rather than `ConjectureData`.
  `InvalidArgument` is not a `TypeError` subclass.
- The pytest plugin's `--hypothesis-show-statistics`, `--hypothesis-explain`
  and statistics in the terminal summary. `--hypothesis-seed`,
  `--hypothesis-profile` and `--hypothesis-verbosity` are supported, as is
  the "pointless decorator" check for `@example`/`@settings` without `@given`.

## Output format

Hegel prints a failing input as the draws it made, one per line, and attaches
the same text to the exception as a note:

```
Falsifying example:
xs = [0, 0]
```

Hypothesis prints the test call, `test_f(xs=[0, 0])`. Tests that match on
Hypothesis's format need adapting (the ported suite's helpers accept both).
