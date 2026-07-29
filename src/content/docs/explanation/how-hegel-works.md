---
title: How Hegel works
---

At the highest level, Hegel splits a property-based testing library into two pieces:

- The *engine*, called `libhegel` implements the core of property-based testing. Data generation, shrinking, the example database, and so on. It is written in Rust.
- The *library* implements the user-facing syntax of properties and generators for a particular language. It asks the engine for generated data as your test runs.

:::note
The engine runs **in-process**. `libhegel` is a native shared library (`libhegel.so`, `libhegel.dylib`, `hegel.dll`) that your Hegel library loads and calls directly through a [C ABI](/reference/libhegel).
:::

As an example, suppose we have the following [hegel-rust](https://github.com/hegeldev/hegel-rust) test (the details of the code under test are unimportant):

```rust
use hegel::{TestCase}
use hegel::generators::{integers}

#[hegel::test(test_cases = 200)]
fn test_a(tc: TestCase) {
    let n: i32 = tc.draw(integers().min_value(100))
}
```

When this test runs:

- `hegel-rust` builds a settings handle describing how the test should run. Here, `test_cases=200`, and calls `hegel_run_start` to create a run.
- `hegel-rust` calls `hegel_next_test_case` to ask the engine for a test case. All of the engine's work happens inside this call. On the calling thread, it decides what the next test case should be and hands back a handle for it.
- `hegel-rust` executes `test_a` against that handle. When `tc.draw` is called, `hegel-rust` calls the typed draw primitive for the generator. Here that is `hegel_generate_integer` with `min_value = 100`, and the engine returns some value in the range `[100, i32::MAX]`.
- The test case finishes. `hegel-rust` reports the outcome with `hegel_mark_complete`. It is valid if the body ran to completion, interesting if the property failed, invalid if the test case was rejected, or overran if too much data was generated.
- Once the engine has run 200 valid test cases, or has finished shrinking a failure, `hegel_next_test_case` returns no further test case. `hegel-rust` reads the aggregate outcome with `hegel_run_result`.
- If the test failed, the shrinking already happened inside the engine. The result carries a minimal counterexample per distinct bug, each with a *reproduce blob* that `hegel-rust` replays to display the failing values and re-raise the original panic.

The engine drives the test lifecycle, but the library calls the system under test with engine-generated data and reports outcomes to the engine.

Compound generators need a little more than plain draws. `hegel-rust` wraps each structure in a *span* (`hegel_start_span` / `hegel_stop_span`) so the shrinker knows which draws belong together and can shrink them as a unit, and it uses the engine's *collection* primitives for variable-length values, letting the engine decide how many elements a `Vec` should have rather than drawing a length up front.

We have glossed over some subtlety here. For example, `tc.assume()` and `generator.filter()` can reject test cases during the test, which the library reports back so the engine can discard and retry. And the engine also reports internal errors to the library, for example in the case of a flaky test.

For the full details, see the [libhegel reference](/reference/libhegel).
