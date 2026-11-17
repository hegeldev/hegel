RELEASE_TYPE: patch

This patch moves the printing of a test case's drawn values, notes and failure diagnostic into the engine. A verbose run now prints each test case's output as it happens rather than once the test case finishes, so a long-running or stateful test (a concurrent state machine round by round) shows its progress live, and a test case's output no longer accumulates in memory at any verbosity. Failure reports at the default verbosity are unchanged.

A verbose run now follows each failing test case with its panic on one line, `thread '<name>' (<id>) panicked at file:line:col: message` (a message spanning several lines cut at its first line), instead of the full panic diagnostic. The full diagnostic, with a backtrace when `RUST_BACKTRACE` asks for one, is printed per failing test case at `Verbosity::Debug`, and in the failure report as before. A backtrace is now captured only when that full diagnostic will be shown.
