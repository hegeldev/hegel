# Hegel for Python compared with Hypothesis

This page records what Hegel for Python has, what it does differently, and
what it lacks, relative to Hypothesis 6.168. `tests/hypothesis_port/GAPS.md`
holds the finer-grained list produced while porting Hypothesis's test suite
to `hegel.hypothesis`.

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

- `deadline`: enforced in Python per test case (with Hypothesis's 1.25x
  allowance on the final replay); Hegel's engine has no deadline concept.
- `HealthCheck.return_value`, `function_scoped_fixture`: checked in Python.
  `not_a_test_method`, `differing_executors` and `nested_given` are accepted
  and ignored.
- `floats(width=16)`: drawn as 32-bit floats rounded to 16-bit values.
- `shared`, `runner`, `slices`, `functions`, `randoms`, `random_module`,
  `decimals`, `fractions`, `complex_numbers`, `timezone_keys`: built from
  Hegel generators in Python.
- `Bundle`: a Python list sampled by index, rather than Hegel's engine pool,
  so `.filter`/`.map` and `consumes` keep Hypothesis's semantics exactly.
- `InMemoryExampleDatabase`: a temporary directory, since Hegel's database is
  always on disk.

Not available:

- `Phase.explain` (the explain phase's comments on which parts of an input
  matter), `settings.backend` providers other than Hegel's own
  (`crosshair`, ...), `fuzz_one_input`, observability callbacks and the
  `HYPOTHESIS_EXPERIMENTAL_OBSERVABILITY` output.
- The `hypothesis.extra` packages (numpy, pandas, django, pytz, dateutil,
  lark, redis, ...), the ghostwriter and the CLI.
- `register_random`: Hegel does not manage the state of external PRNGs.
- `recursive(min_leaves=...)`, `characters()` deprecation aliases emit no
  deprecation warnings, `settings.get_current_profile_name`.
- Hypothesis's typed `SearchStrategy` protocol: `hegel.hypothesis` strategies
  are Hegel generators (`SearchStrategy` is an alias of `hegel.generators.Generator`),
  so `.do_draw` receives a `TestCase` rather than `ConjectureData`.
- The pytest plugin's `--hypothesis-show-statistics`, `--hypothesis-explain`
  and statistics in the terminal summary. `--hypothesis-seed`,
  `--hypothesis-profile` and `--hypothesis-verbosity` are supported.

## Output format

Hegel prints a failing input as the draws it made, one per line, and attaches
the same text to the exception as a note:

```
Falsifying example:
xs = [0, 0]
```

Hypothesis prints the test call, `test_f(xs=[0, 0])`. Tests that match on
Hypothesis's format need adapting (the ported suite's helpers accept both).
