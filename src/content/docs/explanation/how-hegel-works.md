---
title: How Hegel works
---

At the highest level, Hegel defines a *protocol* for communication between a *server* and a *client*.

- The server implements the core of property-based testing. Data generation, shrinking, and so on.
- The client implements the user-facing syntax of properties and generators. It asks the server for generated data via protocol requests.

:::note
Currently, the only Hegel server is [`hegel-core`](https://github.com/hegeldev/hegel-core), which itself uses [Hypothesis](https://github.com/hypothesisworks/hypothesis). It is therefore safe to substitute "server" with "hegel-core" on this page, if it helps with your understanding. However, any library which implements the Hegel protocol is a valid server here.
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

- If this is the first Hegel test to run in the test suite, `hegel-rust` spawns the server as a subprocess and negotiates an initial handshake with it. This negotiated connection and subprocess is then reused for any further tests.
  - The handshake is for example used to communicate the server version to `hegel-rust`.
- `hegel-rust` tells the server a new test is being run, and with what settings. Here, the settings are `test_cases=200`.
- `hegel-rust` tells the server a new test case is being started.
- `hegel-rust` executes `test_a` for that test case. When `tc.draw` is called, `hegel-rust` sends the schema representing the generator to the server. The server generates an arbitrary matching value and returns it.
  - Here, the schema is simply `{"type": "integers", "min_value": 100}`. The server might return anything in the range `[100, inf]`.
- The test case finishes. `hegel-rust` communicates this to the server.
- After 200 test cases, the test finishes. `hegel-rust` communicates this to the server.
- If the test fails, `hegel-rust` communicates this to the server. The server then shrinks the failing test case and returns the minimal failing test case to `hegel-rust`, who displays it to the user.

The transport layer of the protocol is currently unix sockets, but the protocol is agnostic to the particular choice of transport layer and this could in principle be swapped for something else.

We have glossed over some subtlety here. For example, `tc.assume()` and `generator.filter()` can reject test cases during the test, which needs to be communicated back to the server. And the server needs the ability to communicate errors to the client, for example in the case of a flaky test or an invalid generator definition.
