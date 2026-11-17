RELEASE_TYPE: patch

This patch makes verbose output (`verbosity="verbose"` or `"debug"`) stream as the test runs. A test case's drawn values and notes are printed as soon as they are final, rather than all at once when the test case ends, so a long-running test shows its progress as it goes and a stateful test prints each step, or each round of a concurrent state machine, as it happens. Hegel also no longer holds a test case's output in memory until it finishes, so memory use stays flat however long a test case runs. Failure reports are unchanged.

An output sink installed with `with_output_override` may now be called while a test case is running, including from a test's own threads, and an exception it raises is reported through `sys.unraisablehook` rather than propagating into the test.

A failing test case in a verbose run now ends with one line, the traceback's line for the frame in your test that raised the exception, followed by its type and the first line of its message (`File "tests/test_x.py", line 12, in test_x: AssertionError: x too big: 10`), instead of its full traceback, which is printed at `verbosity="debug"` instead. Hegel only formats a traceback when it will be printed or reported.

`with_output_override(None)` sends Hegel's output nowhere: nothing is printed, and verbose output costs nothing, while a failure is still reported on its exception as usual.
