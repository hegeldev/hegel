RELEASE_TYPE: patch

`Settings` gains `unbounded_choices` (default `False`), the engine setting that lifts the limit of 2**20 choices per test case, for long-running test cases such as a concurrent state machine driven for hours. `Settings` now mirrors every engine setting, so a profile that sets `unbounded_choices` (one registered by another Hegel library in the same process, say) is no longer reset to the default when a Python test runs with it.
