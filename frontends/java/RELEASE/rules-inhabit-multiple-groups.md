RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups, and renames `@Rule(group = ...)` to `@Rule(groups = ...)`. A rule in several groups runs in the rounds of each of them, so it can overlap the rules of every group it names. A bare `@Rule` still indicates the anonymous group. The groups of one rule must be distinct; the engine rejects a repeated group with an `IllegalArgumentException`.

Before:

```java
@Rule(group = "write")
void write(TestCase tc) { /* ... */ }
```

After:

```java
@Rule(groups = "write")
void write(TestCase tc) { /* ... */ }

@Rule(groups = {"write", "flush"})
void read(TestCase tc) { /* ... */ }
```

Here `read` may run concurrently with `write` in one round and with `flush` in another, while `write` and `flush` never overlap each other, which is how a read can catch a flush that briefly hides a key in a two-tier store.
