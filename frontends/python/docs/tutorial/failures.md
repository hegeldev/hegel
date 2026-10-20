# Understanding failures

When a test fails, Hegel shrinks the failing input and reports the result.
This page explains what is in the report, and the tools Hegel gives you for
debugging a failure and making sure it stays fixed.

## The failure report

Here is a test with a bug, and the report it produces:

```python
@hegel.test
def test_sorted_concatenation(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    ys = tc.draw(gs.lists(gs.integers()))
    assert sorted(xs + ys) == sorted(xs) + sorted(ys)
```

```
E       assert [-1, 0] == [0, -1]
E       Falsifying example:
E       xs = [0]
E       ys = [-1]
E       To reproduce this failure, add this decorator below @hegel.test:
E           @hegel.reproduce_failure("AXicY2NgYGBkZOBiZAAx4Mz/jAwABSkBIg==")
```

The exception is the one your test raised, for the shrunk input: Hegel re-runs
the test on the minimal example and lets that run's exception propagate, so
the traceback and pytest's assertion introspection describe the reported
input. Hegel attaches the rest of the report to the exception as
[notes](https://docs.python.org/3/library/exceptions.html#BaseException.add_note).

## Draw names

Under `Falsifying example:` Hegel lists every value the test drew, in the
order it drew them. A draw assigned to a variable is named after the variable.
A draw made in a loop, or assigned to a variable that is assigned more than
once in the test, is numbered:

```python
@hegel.test
def test_totals(tc):
    n = tc.draw(gs.integers(0, 10))
    total = 0
    for _ in range(n):
        x = tc.draw(gs.integers())
        total += x
    assert total < 1000
```

```
E       Falsifying example:
E       n = 1
E       x_1 = 1000
```

A draw that is not assigned to a variable at all is named `draw_1`, `draw_2`,
and so on. To choose the name yourself, pass `name=` to
{meth}`~hegel.TestCase.draw`; to leave a draw out of the report, use
{meth}`~hegel.TestCase.draw_silent`.

## Adding notes

{meth}`tc.note(message) <hegel.TestCase.note>` adds a line to the report of a
failing test case, in order with the drawn values. Use it to show values the
test computes along the way:

```python
@hegel.test
def test_sorted_concatenation(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    ys = tc.draw(gs.lists(gs.integers()))
    tc.note(f"xs + ys = {xs + ys}")
    assert sorted(xs + ys) == sorted(xs) + sorted(ys)
```

```
E       Falsifying example:
E       xs = [0]
E       ys = [-1]
E       xs + ys = [0, -1]
```

Notes appear only in the report of the final, shrunk test case (and in verbose
output), not every time the test runs, so they cost nothing to leave in.

## Replaying a failure

Every failure Hegel finds is saved in its *example database*, a directory
called `.hegel/examples` under the directory the tests run from. The next time
the test runs, Hegel tries the saved failures before generating anything new,
so a failing test keeps failing in the same way until you fix it. You'll
probably want to add `.hegel/` to your `.gitignore`.

The database is local to one machine. To reproduce a failure somewhere else,
for example one that happened on CI, use the decorator printed at the end of
the report:

```python
@hegel.test
@hegel.reproduce_failure("AXicY2NgYGBkZOBiZAAx4Mz/jAwABSkBIg==")
def test_sorted_concatenation(tc):
    ...
```

With it, the test runs exactly the failing test case instead of generating
any. Remove it once the bug is fixed: the blob describes the choices Hegel
made for one version of the test, so it stops reproducing anything once the
test or the generators change, and Hegel then raises
{class}`~hegel.DidNotReproduce`.

## Explicit test cases

{func}`@hegel.explicit_test_case <hegel.explicit_test_case>` adds an input that
the test always runs, before it generates any. Use it for edge cases you want
checked every time, or to keep a regression test for a bug Hegel found:

```python
@hegel.test
@hegel.explicit_test_case(xs=[0], ys=[-1])
def test_sorted_concatenation(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    ys = tc.draw(gs.lists(gs.integers()))
    assert sorted(xs + ys) == sorted(xs) + sorted(ys)
```

Keyword arguments supply the value of the draw with that name. Positional
arguments supply draws in order, whatever their names. An explicit test case
must supply every value the test draws.

## Multiple failures

A test can fail in more than one way. By default Hegel reports a single
failure, the smallest it found. With the `report_multiple_failures` setting on,
it shrinks and reports each distinct failure (distinguished by the type of
exception and where it was raised), and raises them together as a
{class}`~hegel.MultipleFailures` exception group.
