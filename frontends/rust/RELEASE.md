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
