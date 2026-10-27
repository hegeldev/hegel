# Hegel for Python

:::{important}
Hegel is in beta, and we'd love for you to try it and
[report any feedback](https://github.com/hegeldev/hegel/issues/new). As part
of the beta we may make breaking changes if they make Hegel a better
property-based testing library. See <https://hegel.dev/compatibility> for
details.
:::

Hegel is a library for property-based testing in Python. Rather than writing
tests that check one example each, you write tests that should pass for every
input of some kind, and Hegel generates inputs, looking for one that makes the
test fail. When it finds one, it shrinks it to a minimal example and reports
that.

```python
import hegel
from hegel import generators as gs


def my_sort(ls):
    return sorted(set(ls))


@hegel.test
def test_matches_builtin(tc):
    ls = tc.draw(gs.lists(gs.integers()))
    assert my_sort(ls) == sorted(ls)
```

Run under pytest, this test fails, and Hegel reports the smallest list it could
find that shows the bug:

```
E   assert [0] == [0, 0]
E   ls = [0, 0]
```

Hegel for Python is based on [Hypothesis](https://hypothesis.readthedocs.io),
and uses the same engine as the other [Hegel](https://hegel.dev) libraries. If
you already use Hypothesis, {doc}`explanation/hypothesis` explains how the two
relate.

## Where to start

The documentation is in four parts:

{doc}`tutorial/index`
: A step-by-step introduction to writing tests with Hegel. Start here if you're
  new to Hegel.

{doc}`how-to/index`
: Short guides to specific tasks, such as running Hegel in CI or porting tests
  from Hypothesis.

{doc}`explanation/index`
: Background on how Hegel works and why it behaves the way it does.

{doc}`reference/index`
: The complete API: every generator, setting, and error.

```{toctree}
:hidden:
:caption: Hegel for Python

tutorial/index
how-to/index
explanation/index
reference/index
changelog
```
