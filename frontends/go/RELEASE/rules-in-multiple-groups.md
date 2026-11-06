RELEASE_TYPE: patch

This patch allows a state machine rule to belong to several concurrency groups. Naming a rule in more than one `WithRuleGroup` option used to make `RunStateful` panic; it now puts the rule in each of those groups, so the rule runs in the rounds of every group it belongs to and may overlap the rules of each, while those groups' other rules still never overlap each other.

```go
hegel.RunStateful(tc, machine,
    hegel.WithConcurrency(),
    hegel.WithRuleGroup("operations", "RuleRegister", "RuleIncrement", "RuleRead"),
    hegel.WithRuleGroup("snapshot", "RuleRead", "RuleSnapshot"),
)
```

Here reads overlap registrations and increments in the rounds of `operations`, and overlap snapshots in the rounds of `snapshot`, but a snapshot never overlaps a registration or an increment.
