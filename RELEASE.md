RELEASE_TYPE: patch

This patch adds the `stop_on_first_failure` setting. By default (`true`) a run stops generating once it has found a failure and moves on to shrinking and reporting it. Set it to `false` and the run uses its whole `test_cases` budget before reporting, whatever it finds along the way: a failure replayed from the database no longer skips generation, every stored failure is replayed, and failing test cases count against the budget like valid ones, so a test that fails on every input still ends.

```rust
#[hegel::test(stop_on_first_failure = false)]
fn explores_everything(tc: hegel::TestCase) {
    // ...
}
```

Every distinct failure such a run finds is persisted to the failure database. Which of them are shrunk and reported follows `report_multiple_failures`: with it off (the default) only the smallest failure found is shrunk and reported, and the rest are persisted exactly as found. The setting is reachable like the others: `Settings::stop_on_first_failure`, the `stop_on_first_failure` key in `hegel.toml`, and the `HEGEL_STOP_ON_FIRST_FAILURE` environment variable (`true`, `1` or `yes`, or `false`, `0` or `no`).
