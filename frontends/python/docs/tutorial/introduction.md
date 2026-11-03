# Your first test

This page walks you through installing Hegel and writing your first tests.

## Install Hegel

Hegel for Python is the `hegeltest` package on PyPI. It needs Python 3.11 or
later:

```bash
pip install hegeltest
```

The package is imported as `hegel`. Wheels are published for Linux (x86-64
and ARM64), Apple-silicon macOS and Windows. Anywhere else, pip builds the
package from source, which needs a [Rust toolchain](https://rustup.rs).

## Write a test

Hegel works with any test runner, but it integrates with
[pytest](https://pytest.org), which we'll use in this tutorial. Create a file
called `test_example.py`:

```python
import hegel
from hegel import generators as gs


@hegel.test
def test_integer_self_equality(tc):
    n = tc.draw(gs.integers())
    assert n == n
```

Run it with `pytest test_example.py`. The test passes.

Let's look at what's happening. The `@hegel.test` decorator turns the function
into a property-based test, which runs the function body many times (100, by
default). The function takes a test case, `tc`, whose
{meth}`~hegel.TestCase.draw` method draws a value from a *generator*. Here the
generator is {func}`gs.integers() <hegel.generators.integers>`, so each time
the body runs `n` is a different integer, and the test checks that it is
equal to itself.

The decorated function is still an ordinary function: pytest runs the whole
property-based test when it calls it, and so can you. Calling
`test_integer_self_equality()` runs it outside pytest.

## Find a bug

Next, try a test that fails:

```python
@hegel.test
def test_integers_always_below_50(tc):
    n = tc.draw(gs.integers())
    assert n < 50
```

This test claims that every integer is less than 50, which is wrong. Hegel
finds an integer that makes the assertion fail, then *shrinks* it to the
simplest one it can find, and pytest reports the result:

```
    @hegel.test
    def test_integers_always_below_50(tc):
        n = tc.draw(gs.integers())
>       assert n < 50
E       assert 50 < 50
E       n = 50
E       To reproduce this failure, add this decorator below @hegel.test:
E           @hegel.reproduce_failure("AAEAAAAACgEAAAAy")
```

The failing integer Hegel first found was probably much larger, but `50` is the
smallest counterexample, and it is the one reported. The report lists every
value the test drew, named after the variable it was assigned to. {doc}`failures` covers the rest of the report.

To fix this test, constrain the integers you generate with the `min_value`
and `max_value` arguments:

```python
@hegel.test
def test_bounded_integers_always_below_50(tc):
    n = tc.draw(gs.integers(min_value=0, max_value=49))
    assert n < 50
```

Run the test again. It now passes.

## Draw as you go

A test can draw as many values as it needs, whenever it needs them, and later
draws can depend on earlier ones:

```python
@hegel.test
def test_index_in_bounds(tc):
    xs = tc.draw(gs.lists(gs.integers(), min_size=1))
    i = tc.draw(gs.integers(min_value=0, max_value=len(xs) - 1))
    assert xs[i] in xs
```

Drawing in the body of the test, rather than declaring every input up front,
is how all Hegel tests are written. If you have used Hypothesis, this is like
using `st.data()` in every test; {doc}`../how-to/port-from-hypothesis` shows how
to translate Hypothesis tests.

## Discard inputs that don't apply

Sometimes a generated input isn't one the test should check. Call
{meth}`tc.assume(condition) <hegel.TestCase.assume>` to discard the current
test case unless `condition` holds:

```python
@hegel.test
def test_division(tc):
    a = tc.draw(gs.integers())
    b = tc.draw(gs.integers())
    tc.assume(b != 0)
    assert (a // b) * b + a % b == a
```

Hegel doesn't count a discarded test case as a failure, and moves on to
another one. Assumptions that discard most inputs make a test slow and weak,
and Hegel reports a {class}`~hegel.FailedHealthCheck` when that happens, so it's
better to generate valid inputs directly when you can: here,
`gs.integers().filter(lambda b: b != 0)` would do, and so would two ranges
joined with `|`. The next page covers both.

## Use pytest fixtures

A Hegel test takes the test case as its first argument. Any other parameters
are passed through, so pytest fixtures work as usual:

```python
@hegel.test
def test_write_and_read(tc, tmp_path):
    data = tc.draw(gs.binary())
    path = tmp_path / "data.bin"
    path.write_bytes(data)
    assert path.read_bytes() == data
```

A fixture is set up once for the whole Hegel test, not once per test case, so
every test case shares it. Here each test case overwrites the same file.

Test methods on a class work too: the test case comes after `self`.

## Next steps

You've written tests that draw integers. The next page,
{doc}`generators`, introduces the rest of the generators Hegel provides and
how to combine them.
