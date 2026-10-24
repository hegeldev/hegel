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
