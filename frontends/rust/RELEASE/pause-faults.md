RELEASE_TYPE: patch

This patch adds a `pause_faults` flag to stateful invariants. An invariant declared with it runs every check (on the initial state, at the sampled join points, and on the final state) with fault injection paused: inside [Antithesis](https://antithesis.com/), the fault injector is asked to stand down from just before the invariant's body until just after it returns or panics, so the check observes the system under test without a fault landing in the middle of it. The flag works on sequential and concurrent state machines alike, and combines with `always_run`:

```rust
#[invariant(pause_faults = true)]
fn replicas_agree(&self, _: TestCase) { /* ... */ }

#[invariant(always_run, pause_faults = true)]
fn ledger_balances(&self, _: TestCase) { /* ... */ }
```

For hand-written `StateMachine` and `ConcurrentStateMachine` implementations, `Invariant` and `ConcurrentInvariant` gain a `pause_faults` field and a `.pause_faults(true)` builder method that composes with `new` and `new_always_run`.

The flag is currently a no-op: the engine fixes the interface now and will pause Antithesis fault injection in a later release.
