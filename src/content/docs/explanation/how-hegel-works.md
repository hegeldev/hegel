---
title: How Hegel works
---

At the highest level, Hegel splits a property-based testing library into two parts:

- `libhegel` implements the core of property-based testing, including data generation, shrinking, the example database, and so on. It is written in Rust.
- The library implements the user-facing syntax of properties and generators for a particular language. It asks `libhegel` for generated data as your test runs.

As an example, suppose we have the following [hegel-rust](https://github.com/hegeldev/hegel-rust) test:

```rust
use hegel::{TestCase}
use hegel::generators::{integers}

#[hegel::test(test_cases = 100)]
fn test_a(tc: TestCase) {
    let n: i32 = tc.draw(integers().min_value(100))
}
```

When this test runs, `hegel-rust`:

- Builds a settings handle describing how the test should run. It then calls `hegel_run_start` to create a run.
- Calls `hegel_next_test_case` to ask `libhegel` for a test case.
- Executes `test_a`. When `tc.draw` is called, `hegel-rust` calls the typed draw primitive for the generator. Here, that is `hegel_generate_integer` with `min_value = 100`. `libhegel` returns some value in the range `[100, i32::MAX]`.
- The test case finishes. `hegel-rust` reports the outcome with `hegel_mark_complete`. It is valid if the body ran to completion, interesting if the property failed, invalid if the test case was rejected, or overran if too much data was generated.
- If no failure is found after 100 valid test cases, `hegel_next_test_case` returns `NULL` and the test finishes.
- If the test finds a failure, `hegel-rust` communicates this to `libhegel`, and `libhegel` shrinks the failing test case. `hegel-rust` replays the failure and displays it to the user.

We have glossed over some subtlety here. For example, `tc.assume()` and `generator.filter()` can reject test cases during the test, which needs to be communicated back to `libhegel`. And `libhegel` needs the ability to communicate errors to `hegel-rust`, for example in the case of a flaky test or an invalid generator definition.

For the full details, see the [libhegel reference](/reference/libhegel).
