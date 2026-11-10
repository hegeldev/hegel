RELEASE_TYPE: patch

This patch fixes the unbounded memory growth of a long-running `#[hegel::main]` binary under Antithesis. A run of a single test case inside Antithesis, with the database disabled as the `workload` profile has it, no longer records the choices its test case makes.

A failure of such a run is now reported as it happened, as the panic's location and message. It is not shrunk or replayed, the values it drew are not printed, and it has no `reproduce_failure` blob.
