RELEASE_TYPE: patch

This patch adds `hegel_pause_faults` and `hegel_unpause_faults` to the C ABI. A frontend brackets each stateful invariant declared with `pause_faults` with this pair, so that inside [Antithesis](https://antithesis.com/) the fault injector can be paused while the invariant runs. Both calls are currently no-ops that only validate their test-case handle; the engine will act on them in a later release.
