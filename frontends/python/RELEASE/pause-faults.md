RELEASE_TYPE: patch

This patch adds a `pause_faults` flag to stateful invariants, next to `always_run`: on `@hegel.invariant(...)`, `Machine.invariant(...)` and the `Invariant` dataclass. An invariant declared with `pause_faults=True` has fault injection paused for the duration of each check, so that inside [Antithesis](https://antithesis.com/) the fault injector stops while the invariant runs and the check observes the system under test without a fault landing in the middle of it.

```python
class Cluster(hegel.StateMachine):
    @hegel.invariant(pause_faults=True)
    def replicas_agree(self, tc):
        assert len({node.state() for node in self.nodes}) == 1
```

Pausing is currently a no-op: the engine's new `pause_faults` / `unpause_faults` calls only validate their handle, and a later libhegel release will make them pause Antithesis fault injection. Declaring the flag now means tests pick that up without changes.
