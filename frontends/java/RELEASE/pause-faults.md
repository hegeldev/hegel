RELEASE_TYPE: patch

This patch adds `pauseFaults` to `@Invariant`. An invariant declared with `@Invariant(pauseFaults = true)` is bracketed by the engine's new `hegel_pause_faults` / `hegel_unpause_faults` calls every time it runs (on the initial state, at each sampled join point and on the final state, in both the sequential and the concurrent driver), and is unpaused even when it throws:

```java
class IntegerStack {
  @Rule
  void push(TestCase tc) { ... }

  @Invariant(pauseFaults = true)
  void sizeIsNonNegative(TestCase tc) {
    assertTrue(stack.size() >= 0);
  }
}
```

The flag is meant for tests that run inside [Antithesis](https://antithesis.com/), where it will pause fault injection for the duration of the invariant so a fault cannot land in the middle of the check. It is currently a no-op: this release fixes the interface, and a later engine release acts on it.

The `dev.hegel:hegel-lowlevel` binding contract gains the matching `Libhegel.pauseFaults` and `Libhegel.unpauseFaults` methods.
