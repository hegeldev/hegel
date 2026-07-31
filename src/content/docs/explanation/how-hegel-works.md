---
title: How Hegel works
---

At the highest level, Hegel splits a property-based testing library into two parts:

- `libhegel` implements the core of property-based testing, including data generation, shrinking, the example database, and so on. It is written in Rust.
- The *library* implements the user-facing syntax of properties and generators for a particular language. It asks `libhegel` for generated data as your test runs.

As an example, suppose we have the following [hegel-rust](https://github.com/hegeldev/hegel-rust) test:

```rust
use hegel::{TestCase}
use hegel::generators::{integers}

#[hegel::test(test_cases = 200)]
fn test_a(tc: TestCase) {
    let n: i32 = tc.draw(integers().min_value(100))
}
```

When this test runs:

- `hegel-rust` builds a settings handle describing how the test should run. It then calls `hegel_run_start` to create a run.
- `hegel-rust` calls `hegel_next_test_case` to ask `libhegel` for a test case.
- `hegel-rust` executes `test_a` against that test case. When `tc.draw` is called, `hegel-rust` calls the typed draw primitive for the generator. Here that is `hegel_generate_integer` with `min_value = 100`, and `libhegel` returns some value in the range `[100, i32::MAX]`.
- The test case finishes. `hegel-rust` reports the outcome with `hegel_mark_complete`. It is valid if the body ran to completion, interesting if the property failed, invalid if the test case was rejected, or overran if too much data was generated.
- Once `libhegel` has run 200 valid test cases or has finished shrinking a failure, `hegel_next_test_case` returns no further test case. `hegel-rust` reads the outcome with `hegel_run_result`.
- If the test failed, `libhegel` shrinks and reports the counterexample.

For composite generators, `hegel-rust` wraps them in a *span* (`hegel_start_span` / `hegel_stop_span`) so the shrinker knows which draws belong together and can shrink them as a unit. For collections, it uses `libhegel`'s *collection* primitives for variable-length values, letting `libhegel` decide how many elements a collection, such as a vector, should have.

For the full details, see the [libhegel reference](/reference/libhegel).
