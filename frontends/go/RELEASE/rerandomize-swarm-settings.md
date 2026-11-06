RELEASE_TYPE: patch

This patch lets a long state-machine run explore more than one configuration. Between rounds, a state machine now occasionally redraws its value distribution, the rules each worker has enabled, and the concurrency level. Previously these were fixed for the whole test case, so a single long-running test case kept one configuration for its whole run.

A concurrent machine now always starts as many workers as its maximum concurrency, and only the current concurrency level's worth of them run rules in a round. A failure report notes `Concurrency level: n` at the first round and whenever the level changes.
