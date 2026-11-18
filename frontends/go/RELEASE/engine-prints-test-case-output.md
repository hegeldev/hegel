RELEASE_TYPE: patch

This patch moves printing a test case's output (its draws, notes and failure diagnostic) into the bundled libhegel engine. A run with `WithVerbosity(hegel.VerbosityVerbose)` now prints every test case's draws and notes as they happen, a state machine round by round, and says when a test case was stopped by a failed assumption or by running out of data. The memory of a long-running or stateful test case no longer grows with what it prints.

A failing test case in a verbose run is now summarised in one line, the line of your test that failed and the first line of its message (`sample_test.go:27: panic: x too big`), instead of a full stack trace per test case. `WithVerbosity(hegel.VerbosityDebug)` prints the full stack trace of every failing test case, and the stack trace is only rendered when it will be printed.

The report of a failure at the default verbosity is unchanged. A run with `WithVerbosity(hegel.VerbosityQuiet)` no longer prints the draws and diagnostic of its failures.
