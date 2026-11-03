RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups. `hegel::stateful::ConcurrentRule` now takes a list of group names in place of one name, and `group()` becomes `groups()`, returning that list. A rule constructed without groups still belongs to `anonymous_group`.

Before:

```cpp
hegel::stateful::ConcurrentRule<Store>("write", "write", write);
```

After:

```cpp
hegel::stateful::ConcurrentRule<Store>("write", {"write"}, write);
hegel::stateful::ConcurrentRule<Store>("read", {"write", "flush"}, read);
```

A rule in several groups runs in the rounds of each of them: here `read` can overlap with `write` in a `write` round and with `flush` in a `flush` round, which is the only way to observe a read racing a flush. The list must name at least one group and must not repeat one; the engine reports either as a `std::invalid_argument` when the machine starts.
