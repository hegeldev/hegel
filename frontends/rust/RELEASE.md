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

- A run that errors instead of passing or failing now panics with a message whose start names the kind of error, read from the engine's error kind rather than its wording: `FailedHealthCheck: <check>` (for example `FailedHealthCheck: FilterTooMuch`), `Flaky` (nondeterminism under `NondeterminismStrictness::Error`), `Unsatisfiable`, or `RunError` for any other error the engine reports. The engine's explanation follows. A usage error that a test case hits while it runs, such as a generator with invalid bounds, still panics with its own message alone, for example `Cannot have max_value < min_value`. `Hegel::run` documents the forms, and `#[should_panic(expected = "...")]` can match on them. Messages for nondeterministic data generation and for other run errors gain a `Flaky: ` or `RunError: ` prefix that they did not have before.

- The `unbounded_choices` setting is now available like the others: `Settings::unbounded_choices`, `#[hegel::test(unbounded_choices = true)]`, the `unbounded_choices` key in `hegel.toml` and the `HEGEL_UNBOUNDED_CHOICES` environment variable. It removes the limit of 2^20 choices per test case without suppressing the `TestCasesTooLarge` health check, and `Settings::register_profile` keeps it.

- `Settings::database` now takes the new `hegel::Database` enum, or an `Option<String>` as before, so `Settings::database(Database::Default)` can restore the default database after a profile disabled it or set a path.

- `Settings::database_key` sets the key a test's failing examples are stored under, which `#[hegel::test]` otherwise derives from the test's module path and name. Setting it keeps a test's stored failures when it is renamed or moved, or shares them between tests.

- A failed `tc.assume` or `tc.reject()` in an explicit test case (`#[hegel::explicit_test_case]`), including inside `tc.repeat`, fails the test with the message `Explicit test cases must not be rejected`, as in the other Hegel frontends, rather than `Explicit test case: assumption violated (tc.assume / tc.reject)`. An explicit test case is one the test asked for, so rejecting it is a mistake in the test rather than a case to skip. `ExplicitTestCase` gains `draw_named`, `event`, `event_value` (which do nothing, as statistics only count generated test cases) and `repeat` (which runs its body once, since an explicit value can only be drawn once).

- An explicit test case now looks up a draw that can run more than once by its numbered name, `x_1`, `x_2`, ..., as the `#[hegel::explicit_test_case]` documentation already said, and as a failing test case prints it. Before, such a draw looked up the bare name `x`, so the documented example failed. An unnamed `tc.draw` is likewise `draw_1`, `draw_2`, .... As in a generated test case, a name drawn both as a draw that can run more than once and as one that cannot, or a name of the second kind drawn twice, is now an error in an explicit test case too.

- The reproducer line a failure prints now names the attribute the test was defined with: `#[hegel::test]`, `#[hegel::main]` or `#[hegel::standalone_function]`, and `#[hegel::reproduce_failure]` now works below `#[hegel::main]` and `#[hegel::standalone_function]` as well as `#[hegel::test]`. A run built with the `Hegel` builder directly is told to call `.reproduce_failure("…")` on the builder.

- Stacked `#[hegel::reproduce_failure]` attributes, and repeated calls to `Hegel::reproduce_failure`, now replay every blob, one after another, through the engine's `hegel_run_start_blob`, and report every failure they reproduce, as a run reports several distinct failures. Before, only the first blob replayed and the rest were ignored. A blob that no longer reproduces is not an error while another one does, but every blob must decode: a stacked placeholder that is not a blob now fails the test.

- The string generators (`gs::characters()`, `gs::text()`, `gs::from_regex(..)` and `gs::from_lark(..)`) now follow the Unicode version of the Rust standard library Hegel was compiled with, `char::UNICODE_VERSION`, which Hegel passes to the engine (`hegel_set_unicode_version`) before generating anything. A character's category, and the character classes of a pattern, now agree with `char::is_alphabetic` and friends: on a toolchain with Unicode tables older than the engine's, characters the toolchain does not know yet are in no category, rather than in the category a newer version gives them.

- `TestCase::set_worker(index)` attributes a test case's output to a worker, as the other Hegel frontends' `set_worker` / `setWorker` do: every line it prints from then on, drawn values and notes alike, is prefixed with `[worker N +X.XXXms]` (`hegel_test_case_set_worker`), and clones made from it afterwards inherit the attribution. Call it on the clone handed to each thread, so a failure report shows which thread printed each line and how their lines interleaved.

- `HealthCheck::all()` now returns a `Vec<HealthCheck>` rather than a `[HealthCheck; 4]`, so that adding a health check does not change its type. It is no longer a `const fn`.

- The `exclude_min` and `exclude_max` methods of `gs::floats()`, deprecated since 0.42.0 and panicking whenever called, are removed. Use `min_value_exclusive` and `max_value_exclusive`.

- The documentation of `TestCase::repeat` now gives the note it prints at the start of each iteration correctly, as `// Repetition #N`.

- `PrettyPrinter::if_break` emits text only when the enclosing group breaks across lines, through the engine's `hegel_printer_if_break`: for example a trailing comma that only a value's multi-line form needs.

- `VecGenerator::unique_by` makes `gs::vecs(..)` generate vectors whose elements have distinct keys, as computed by a function of each element: `gs::vecs(people()).unique_by(|person: &Person| person.id)`. It and `unique` replace each other.

- A `#[hegel::main]` binary now takes every setting as a command-line flag, named after its `hegel.toml` key with `-` for `_`: `--phases`, `--report-multiple-failures`, `--stop-on-first-failure`, `--show-statistics`, `--print-blob`, `--nondeterminism-strictness`, `--unbounded-choices` and `--database-key` join `--seed`, `--verbosity`, `--derandomize`, `--database`, `--suppress-health-check` and `--backend`. The engine parses every flag's value (`hegel_settings_set_from_string`), in the vocabulary of `hegel.toml`, and its error messages are the engine's, naming the setting by its flag: `--seed expects an integer or "none", got "abc"`. Each flag also takes the form `--flag=VALUE`. A flag followed directly by another flag, as in `--seed --verbosity quiet`, is reported as missing its value, and `--database-key` rejects an empty key. `--test-cases` is still rejected, now with a message saying why.

- `gs::text()` and `gs::binary()` without a `max_size` now leave the default maximum to the engine (`HEGEL_MAX_SIZE_DEFAULT`), which applies the same default as before, 100 or `min_size + 100`, so every frontend shares it. A `max_size` of `usize::MAX - 1` now means no upper bound, as `usize::MAX` does. Filters take their number of attempts, 3, from the engine's `HEGEL_FILTER_ATTEMPTS`.

- `gs::from_lark(..).explicit(..)` now has the engine splice explicit terminals' values into the drawn string (`hegel_grammar_draw_new` and friends) rather than doing it in Rust: the engine finds each terminal's marker, keeps the string, values included, under `max_size` by drawing it again with less room for the grammar's own text, and rejects the test case after `HEGEL_FILTER_ATTEMPTS` attempts that do not fit. What it generates is unchanged in kind. A grammar without a `max_size` now takes the engine's default cap, 8192, from the engine.

- `tc.repeat` now has the engine decide how many repetitions to run and which one failed, through libhegel's new `hegel_repeat_t` functions, rather than drawing the loop's length and the failure marker itself. The choices are the same, so a `#[hegel::reproduce_failure]` blob from an earlier release still replays through the same repetitions.
