RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups. The list must name at least one group and must not repeat one; both are compile errors. A bare `#[rule]` still indicates the anonymous group.

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
