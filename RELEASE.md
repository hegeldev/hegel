RELEASE_TYPE: patch

This patch stops `#[hegel::reproduce_failure]` reporting a failure its blob did not record. A blob replayed against a test whose draws have changed since it was recorded is no longer continued past the change with fresh draws: such a replay does not reproduce, the attribute panics as for any blob that did not reproduce, and the engine's output names where the test's draws left the recording.
