RELEASE_TYPE: patch

This patch bumps our pinned libhegel ([hegeldev/hegel](https://github.com/hegeldev/hegel)) from [0.45.1](https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v0.45.1) to [0.48.0](https://github.com/hegeldev/hegel/releases/tag/libhegel/0.48.0).

Changes:
- A run that never generates a valid test case now always fails with `Unsatisfiable`.
- Verbose output now prints `Running test case` before every test case.
- Character classes and text categories now use Unicode 17.0 instead of 15.1.
- A new `Settings.stop_on_first_failure` field (default `true`). Set it to `false` to spend the whole `test_cases` budget before reporting. A profile's `stop_on_first_failure` key now carries through `Settings.from_profile`.
- Filtering `Stateful.Pool.values_consumed` or `Stateful.Concurrent_pool.values_consumed` now removes only the value it returns. Before, every value the filter rejected was also removed from the pool.
- `one_of []` now raises `Usage_error` instead of `Failure`.
- Hegel now looks for a local libhegel build in `../hegel/target/`.
- General shrinking improvements
