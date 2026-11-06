RELEASE_TYPE: patch

This patch adds weights, a hint for how often to choose the rule relative to the others, to stateful rules. `hegel::stateful::Rule` and `hegel::stateful::ConcurrentRule` each take an optional `weight`. `weight()` returns it. A rule without a weight has weight 1.0.

`hegel::stateful::Invariant` also now takes an optional `always_check` flag. An invariant with the flag runs at every step of a state machine, or every round of a concurrent one, instead of being sampled. `always_check()` returns it.
