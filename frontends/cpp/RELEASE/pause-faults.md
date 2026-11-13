RELEASE_TYPE: patch

This patch adds a `pause_faults` flag to `hegel::stateful::InvariantParams`, next to `always_check`. An invariant declared with it asks the engine to pause fault injection for the duration of each of its checks, in sequential and concurrent state machines alike, so that inside Antithesis no fault lands in the middle of the check. Fault injection resumes when the check returns or throws.

```cpp
std::vector<hegel::stateful::Invariant<Bank>> invariants() const {
    return {hegel::stateful::Invariant<Bank>(
        {.name = "balances_sum_to_total",
         .pause_faults = true,
         .invariant = [](const Bank& m) { m.verify_total(); }})};
}
```

The flag defaults to `false`, combines with `always_check`, and `Invariant::pause_faults()` reads it back. Outside Antithesis the flag has no effect.
