RELEASE_TYPE: patch

This release makes the engine print each test case's drawn values and notes as soon as they can no longer change, instead of Hegel for C++ reading them back after the test case ends. With `Verbosity::Verbose` or `Verbosity::Debug`, the output of a long-running or stateful test now appears while the test runs (a concurrent state machine prints round by round), and a test case no longer keeps all of its output in memory until it ends. Failure reports at `Verbosity::Normal` are unchanged.

A failing test case is now followed by its diagnostic: at `Verbosity::Verbose`, one line with the exception's type and the first line of its message (for a failed GoogleTest assertion, its `file:line` and the first line of its message), and at `Verbosity::Debug` the whole message.
