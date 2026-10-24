RELEASE_TYPE: minor

This release brings the Rust frontend further into line with the other Hegel frontends.

- `generators::label_from_name` and `generators::combine_labels` now ask the engine for labels (`hegel_label_from_name` / `hegel_label_combine`) instead of reimplementing its hash, so they are no longer `const fn`. Code that computed a label in a `const` should compute it once in a `static LazyLock<u64>` instead:

  ```rust
  // before
  const PAIRS_LABEL: u64 = gs::label_from_name("mycrate.pairs");

  // after
  static PAIRS_LABEL: LazyLock<u64> = LazyLock::new(|| gs::label_from_name("mycrate.pairs"));
  ```

  The labels themselves are unchanged.

- A run that errors instead of passing or failing now panics with a message whose start names the kind of error, read from the engine's error kind rather than its wording: `FailedHealthCheck: <check>` (for example `FailedHealthCheck: FilterTooMuch`), `Flaky` (nondeterminism under `NondeterminismStrictness::Error`), `Unsatisfiable`, or `RunError` for anything else, such as misuse of Hegel or a bug in Hegel. The engine's explanation follows. `Hegel::run` documents the forms, and `#[should_panic(expected = "...")]` can match on them. Messages for nondeterministic data generation and for other run errors gain a `Flaky: ` or `RunError: ` prefix that they did not have before.

- The `unbounded_choices` setting is now available like the others: `Settings::unbounded_choices`, `#[hegel::test(unbounded_choices = true)]`, the `unbounded_choices` key in `hegel.toml` and the `HEGEL_UNBOUNDED_CHOICES` environment variable. It removes the limit of 2^20 choices per test case without suppressing the `TestCasesTooLarge` health check, and `Settings::register_profile` keeps it.

- `Settings::database` now takes the new `hegel::Database` enum, or an `Option<String>` as before, so `Settings::database(Database::Default)` can restore the default database after a profile disabled it or set a path.

- `Settings::database_key` sets the key a test's failing examples are stored under, which `#[hegel::test]` otherwise derives from the test's module path and name. Setting it keeps a test's stored failures when it is renamed or moved, or shares them between tests.

- A failed `tc.assume` or `tc.reject()` in an explicit test case (`#[hegel::explicit_test_case]`) now skips that case, as it skips a generated one, instead of failing the test. `ExplicitTestCase` gains `draw_named`, `event`, `event_value` (which do nothing, as statistics only count generated test cases) and `repeat` (which runs its body once, since an explicit value can only be drawn once).

- An explicit test case now looks up a draw that can run more than once by its numbered name, `x_1`, `x_2`, ..., as the `#[hegel::explicit_test_case]` documentation already said, and as a failing test case prints it. Before, such a draw looked up the bare name `x`, so the documented example failed. An unnamed `tc.draw` is likewise `draw_1`, `draw_2`, ....

- The reproducer line a failure prints now names the attribute the test was defined with: `#[hegel::test]`, `#[hegel::main]` or `#[hegel::standalone_function]`, and `#[hegel::reproduce_failure]` now works below `#[hegel::main]` and `#[hegel::standalone_function]` as well as `#[hegel::test]`. A run built with the `Hegel` builder directly is told to call `.reproduce_failure("…")` on the builder.

- `HealthCheck::all()` now returns a `Vec<HealthCheck>` rather than a `[HealthCheck; 4]`, so that adding a health check does not change its type. It is no longer a `const fn`.

- The `exclude_min` and `exclude_max` methods of `gs::floats()`, deprecated since 0.42.0 and panicking whenever called, are removed. Use `min_value_exclusive` and `max_value_exclusive`.

- The documentation of `TestCase::repeat` now gives the note it prints at the start of each iteration correctly, as `// Repetition #N`.

- `PrettyPrinter::if_break` emits text only when the enclosing group breaks across lines, through the engine's `hegel_printer_if_break`: for example a trailing comma that only a value's multi-line form needs.

- `VecGenerator::unique_by` makes `gs::vecs(..)` generate vectors whose elements have distinct keys, as computed by a function of each element: `gs::vecs(people()).unique_by(|person: &Person| person.id)`. It and `unique` replace each other.
