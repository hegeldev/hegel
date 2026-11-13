RELEASE_TYPE: patch

This patch adds `PauseFaults` to `[Invariant]`. An invariant declared with `[Invariant(PauseFaults = true)]` runs every check (on the initial state, at the sampled join points and on the final state, sequentially or with `MaxConcurrency` above one) with fault injection paused: inside [Antithesis](https://antithesis.com/) the engine runs the fault-control program before the invariant and again after it, even when the invariant throws, so no fault lands in the middle of the check. It combines with `AlwaysRun`, and outside Antithesis it has no effect.

```csharp
[Invariant(PauseFaults = true)]
internal void ReplicasAgree(TestCase tc) => Assert.Single(nodes.Select(n => n.State).Distinct());
```
