RELEASE_TYPE: minor

This release changes how a state machine is declared through the C ABI, and lets a rule belong to several concurrency groups.

`hegel_new_state_machine` no longer takes the rules and invariants. It now creates an empty machine from its concurrency bounds and step count, and the caller declares each rule with `hegel_state_machine_add_rule` (its name, selection weight, and the array of concurrency-group ids it belongs to) and each invariant with `hegel_state_machine_add_invariant` (its name and always-check flag), then seals the declaration with `hegel_state_machine_start`, which draws the concurrency level and reports it through `out_concurrency`. Rules and invariants are numbered in declaration order from 0, which is what `hegel_state_machine_next_rule` and `hegel_state_machine_should_check_invariant` use. No rule or invariant may be added after `hegel_state_machine_start`, and no round may be driven before it; both are `HEGEL_E_INVALID_ARG`.

Before:

```c
hegel_new_state_machine(ctx, tc, rule_names, rule_groups, rule_weights, num_rules,
                        invariant_names, invariant_always_check, num_invariants,
                        1, 4, 50, &machine, &concurrency);
```

After:

```c
hegel_new_state_machine(ctx, tc, 1, 4, 50, &machine);
hegel_state_machine_add_rule(ctx, tc, machine, rule_names[0], 1.0, groups, num_groups);
hegel_state_machine_add_invariant(ctx, tc, machine, invariant_names[0], false);
hegel_state_machine_start(ctx, tc, machine, &concurrency);
```

A rule listed in several groups is handed out during rounds of each of them, so the "may run concurrently" relation between rules is now any symmetric relation rather than a partition: rules overlap when they share a group. Swarm testing's guarantee that every group keeps at least one rule enabled accounts for shared membership, so a rule that is the last enabled candidate of any of its groups is kept enabled whichever group's round asks about it.
