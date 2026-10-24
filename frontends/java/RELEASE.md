RELEASE_TYPE: patch

This patch fixes three native memory leaks. Every list a test drew left its engine-side collection allocated, every `Pool` and `ConcurrentPool` stayed allocated after its test case ended, and in a concurrent stateful run each worker thread left its engine error context behind when it exited, once per worker per test case. Long runs, and long concurrent state machines in particular, now hold steady memory.

`dev.hegel.lowlevel.Libhegel` gains `collectionFree`, `poolFree` and `releaseThreadContext` for `hegel_collection_free`, `hegel_pool_free` and `hegel_context_free`, and `TestCase.collectionFree` releases a collection from `TestCase.newCollection`.
