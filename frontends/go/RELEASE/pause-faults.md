RELEASE_TYPE: patch

This patch adds `WithPauseFaultsInvariants`, a `RunStateful` option naming invariants that run with fault injection paused. Each time such an invariant runs (on the initial state, at a sampled join point, or on the final state, with or without `WithConcurrency`) the engine is asked to pause fault injection immediately before the invariant and to resume it immediately after, even when the invariant fails, so the invariant observes the system under test without a fault landing in the middle of the check.

```go
hegel.RunStateful(tc, &machine,
	hegel.WithAlwaysCheckInvariants("InvariantRing"),
	hegel.WithPauseFaultsInvariants("InvariantRing"),
)
```

Pausing is currently a no-op: the bundled engine accepts the calls but does not act on them yet. A later libhegel release will pause the Antithesis fault injector for the duration of each such invariant when the test runs inside Antithesis.
