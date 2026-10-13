RELEASE_TYPE: patch

This patch makes `Verbosity::Verbose` output announce every executed test case with `Running test case`, including the first, simplest input of a run and the replays of the reuse phase, which previously appeared under their phase heading with no announcement.

It also rewords the health check and `Unsatisfiable` messages to name the check plainly ("suppress the FilterTooMuch health check with the suppress_health_check setting") and to describe rejections as "rejected by assume() or a filter", since the engine does not know which did the rejecting. The `FailedHealthCheck:` and `Unsatisfiable` prefixes are unchanged.
