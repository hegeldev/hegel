RELEASE_TYPE: minor

This release adds weights, a hint for how often to choose the rule relative to the others, to stateful rules. `hegel::stateful::Rule` and `hegel::stateful::ConcurrentRule` each take an optional `weight`, and `weight()` returns it. A rule without a weight has weight 1.0.

`hegel::stateful::Invariant` also now takes an optional `always_check` flag. An invariant with the flag runs at every step of a state machine, or every round of a concurrent one, instead of being sampled. `always_check()` returns it.

These can be passed positionally or by name through the new `RuleParams`, `ConcurrentRuleParams` and `InvariantParams` structs:

```cpp
hegel::stateful::ConcurrentRule<Bank>("transfer", {"bank"}, 6.0, transfer);

hegel::stateful::ConcurrentRule<Bank>({.name = "transfer",
                                       .groups = {"bank"},
                                       .weight = 6.0,
                                       .step = transfer});
```

`ConcurrentRule::function()` is now `step()`, and `ConcurrentRule::Function` is now `Step`, to match `Rule`.
