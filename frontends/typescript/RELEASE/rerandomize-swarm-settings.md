RELEASE_TYPE: patch

This patch lets a long state-machine run explore more than one configuration. Between steps, a state machine now occasionally redraws its value distribution and which of its rules are enabled. Previously these were fixed for the whole test case, so a single long-running test case kept one configuration for its whole run.
