RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups, and changes how a rule names its groups: `#[rule(groups = ["write", "flush"])]` replaces `#[rule(group = "...")]`, and a rule in one group lists just that one. A rule in several groups is handed out whenever any of them is the current group, so it may overlap the rules of each group even though those groups never overlap each other. That is how to say that a `read` may run alongside writes and alongside flushes while writes and flushes stay serialized. The list must name at least one group and must not repeat one; both are compile errors. A bare `#[rule]` still means the anonymous group.

Before:

```rust
#[rule(group = "write")]
fn write(&self, tc: TestCase) { /* ... */ }
```

After:

```rust
#[rule(groups = ["write"])]
fn write(&self, tc: TestCase) { /* ... */ }

#[rule(groups = ["write", "flush"])]
fn read(&self, tc: TestCase) { /* ... */ }
```

The `examples/concurrent_tiered_store.rs` example shows this on a two-tier store whose flush briefly hides a key: only a read that overlaps a flush can observe it.

For hand-written `ConcurrentStateMachine` implementations, `ConcurrentRule`'s `group: String` field is now `groups: Vec<String>`, and `ConcurrentRule::new` takes the group list, `ConcurrentRule::new(name, &["a", "b"], weight, apply)`, in place of the one group. A rule whose `groups` is empty or repeats a group is reported as a usage error when the machine runs.
