RELEASE_TYPE: minor

This release moves the printing of a test case's draws and notes into the engine, which keeps the memory of a long-running test case flat at every verbosity. A run with `Verbosity.Verbose` now streams each test case's output to the reporter's `EngineOutput` as it is printed, so a stateful test prints step by step (and a concurrent one round by round) instead of all at once when the test case ends. Failure reports at the default verbosity are unchanged.

A verbose run now also prints one line after each failing test case: the file and line in your test that threw, then the exception's type and the first line of its message, such as `/src/Tests.cs:16:13: InvalidOperationException: too big: 331`. `Verbosity.Debug` prints the full exception, stack trace included, instead.

`Reporter.Silent` now tells the engine the run has no output destination, so a silent run spends nothing on verbose or debug output.

A run with `Verbosity.Quiet` now leaves `Failure.Output` empty.

`TestCase.IsFinal` is removed. The engine now decides which test cases' output it uses, and that set (every test case of a verbose run, and the runs a failure report is built from) no longer matches what the name promised.
