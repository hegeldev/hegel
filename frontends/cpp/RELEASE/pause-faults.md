RELEASE_TYPE: patch

This patch adds a `pause_faults` flag to `hegel::stateful::Invariant`. An invariant declared with it asks the engine to pause fault injection for the duration of each of its checks, in sequential and concurrent state machines alike, so that inside Antithesis no fault lands in the middle of the check. Fault injection resumes when the check returns or throws.

```cpp
std::vector<hegel::stateful::Invariant<Bank>> invariants() const {
    return {hegel::stateful::Invariant<Bank>(
        "balances_sum_to_total",
        [](const Bank& m) { m.verify_total(); }, true)};
}
```

The flag defaults to `false`, and `Invariant::pause_faults()` reads it back. Pausing is currently a no-op: a later engine release will act on it to pause Antithesis fault injection.
