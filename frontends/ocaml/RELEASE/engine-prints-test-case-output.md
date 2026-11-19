RELEASE_TYPE: patch

This release moves the printing of a test case's output (its drawn values, notes and state machine steps) into the engine. Under `Verbose` and `Debug` verbosity, each test case's output now streams as it happens instead of appearing once the test case finishes, so a long-running or stateful test shows its progress and a concurrent state machine prints round by round. A test case no longer holds all its output in memory until it completes, so memory stays flat however long it runs.

A failing test case is now followed by a one-line summary under `Verbose` (`Exception: <exn>`, led by the failing line in your test when a backtrace is recorded) and by the exception with its backtrace under `Debug`. Failure reports are unchanged.
