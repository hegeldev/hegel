RELEASE_TYPE: patch

This patch has the engine print each test case's draws and notes. A failing test's report looks the same as before. A verbose run now prints each test case as it runs, not after the test case finishes, so you see a long-running or stateful test's progress, step by step, as it happens. Each test case's output is no longer kept until the test case ends, so memory stays flat however much a test case prints.

A verbose run now also prints a one-line description after each failing test case: the error's type and message, after the place in your test that threw it (`tests/sum.test.ts:12:7: Error: too big: 331`). At `Verbosity.Debug` it prints the error's full stack there instead.

The engine's own lines, such as a verbose run's phases and the statistics printed with `showStatistics`, now go wherever the run's report goes (`console.error`) rather than straight to the process's standard error. In browsers, which used to drop them, they now appear too.
