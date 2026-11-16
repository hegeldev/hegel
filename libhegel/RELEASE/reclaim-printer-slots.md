RELEASE_TYPE: patch

This patch fixes the memory a long-running test case keeps for the output regions of its test-case handles. Every `hegel_test_case_clone` and `hegel_test_case_block` handle opens a region in the test case's document, and these regions were kept until the test case ended, so a concurrent state machine that never stops, such as a `#[hegel::main]` workload in Antithesis, grew without bound. A region that holds no output is now freed when the last handle writing into it is freed.
