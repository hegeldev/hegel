# See what your test generates

A property-based test is only as good as the inputs it checks, and it's not
always obvious what those are. Hegel has several ways to look.

## Statistics

Turn on the `show_statistics` setting, and Hegel prints a summary of the
*events* the test recorded at the end of each run: {meth}`tc.event(label)
<hegel.TestCase.event>` counts the test cases in which `label` occurred, and
{meth}`tc.event_value(label, value) <hegel.TestCase.event_value>` summarises a
number across test cases:

```python
@hegel.test(show_statistics=True, test_cases=200)
def test_lists(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    tc.event("empty" if not xs else "non-empty")
    tc.event_value("length", len(xs))
    ...
```

```
Statistics (over 200 test cases):
  * empty: 5.0% of test cases
  * non-empty: 95.0% of test cases
  * length: count 200, min 0, median 5, mean 5.25, p90 8, max 19
```

Hegel writes the statistics to standard error. Under pytest, run with `-s`
to see them for passing tests.

To turn on statistics for a whole run without editing any tests, set the
`HEGEL_STATISTICS` environment variable:

```bash
HEGEL_STATISTICS=1 pytest -s test_lists.py
```

## Verbose output

With `verbosity="verbose"`, Hegel prints every test case as it runs, with the
values it drew and any notes:

```bash
pytest -s --hegel-verbosity=verbose test_lists.py
```

This is a lot of output, but it's the most direct way to see what a test is
doing.

## Looking at a generator's values

To get a feel for a generator by itself, draw from it in a small test that
fails on purpose, or prints:

```python
@hegel.test(test_cases=10, database="")
def test_show(tc):
    print(tc.draw(gs.emails()))
```
