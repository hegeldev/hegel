RELEASE_TYPE: minor

This release lets a long state-machine run explore more than one configuration. Until now a test case drew its value distribution, each worker's set of enabled rules, and its concurrency level once, at the start. A `#[hegel::main]` binary, which runs a single test case, kept that configuration for its whole life. At the start of each round, a state machine now redraws all three with probability 1/50.

To let the concurrency level change between rounds without starting or stopping threads, the meaning of the concurrency values in the C ABI changes:

* `hegel_state_machine_start` now writes `max_concurrency` into `*out_concurrency`. The caller runs that many workers for the whole test case.
* `hegel_state_machine_next_group` takes a new `out_concurrency` parameter, which receives the round's concurrency level. That many workers, chosen by the engine, are handed rules. The others get `HEGEL_STATE_MACHINE_DONE` from `hegel_state_machine_next_rule`.

Before:

```c
hegel_state_machine_next_group(ctx, tc, machine, &group);
```

After:

```c
hegel_state_machine_next_group(ctx, tc, machine, &group, &concurrency);
```
