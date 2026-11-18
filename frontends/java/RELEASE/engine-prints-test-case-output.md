RELEASE_TYPE: minor

This release moves to the engine's new test-case output protocol, in which libhegel decides what a test case's output is used for and prints it itself.

A run at `Verbosity.VERBOSE` now shows, through the run's `Reporter.engineOutput`, a one-line summary of each failing test case right after that case's draws and notes — the file and line of the test it was thrown from, then the exception, for example `SortTest.java:23: java.lang.AssertionError: not sorted` — and a `Test case stopped: failed assumption` or `Test case stopped: out of data` line for each case that was rejected or ran out of data. At `Verbosity.DEBUG` each failing test case shows its full stack trace instead. The stack trace is only rendered when the engine will use it. The default report of a failure is unchanged.

`TestCase.isFinal()` is removed. The engine now decides which test cases' output it uses, and that set (every test case of a verbose run, and the executions a failure report is built from) no longer matches what the name promised. A frontend built on `dev.hegel:hegel-lowlevel` can ask the engine directly with `Libhegel.testCaseWantsOutput`.

`Reporter.silent()` now gives the engine no output destination, so a verbose or debug run with it renders nothing nobody would see.

The `dev.hegel:hegel-lowlevel` binding contract changes with the C ABI: `Libhegel.testCaseShouldCapture` is replaced by `Libhegel.testCaseWantsOutput` and `Libhegel.testCaseWantsDetail`, `Libhegel.markComplete` takes a failure's `summary` and `detail` after its origin, and `Libhegel.failureOutput` and `Libhegel.failureDiagnostic` are new. A `null` output callback now means no destination rather than stderr; pass the new `Libhegel.STDERR` to have the engine print to stderr. The output callback passed to `Libhegel.testCaseFromBlob` is now kept alive until the test case is released with `testCaseFree`, since the engine prints the replay's output when the test case completes.
