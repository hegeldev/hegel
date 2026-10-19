# Using generators

A *generator* describes the values a test can draw: which values are possible,
and how they shrink. Hegel provides generators for the built-in types and many
standard-library ones, and ways to combine them into generators for your own
data. They all live in {mod}`hegel.generators`, which this documentation always
imports as `gs`:

```python
from hegel import generators as gs
```

This page tours the most common ones. {doc}`../reference/generators` lists
every generator and all of its arguments.

## Primitive values

```python
@hegel.test
def test_primitives(tc):
    n = tc.draw(gs.integers(min_value=0))
    x = tc.draw(gs.floats(allow_nan=False))
    flag = tc.draw(gs.booleans())
    s = tc.draw(gs.text())
    b = tc.draw(gs.binary(max_size=10))
```

Most generators take arguments that narrow what they produce. Integers can be
bounded on either side or both, floats can exclude NaN and the infinities,
and text can be limited to an alphabet or to some Unicode categories:

```python
digits = gs.text(alphabet="0123456789", min_size=1)
letters = gs.text(categories=["Lu", "Ll"])
probability = gs.floats(min_value=0.0, max_value=1.0)
```

There are also generators for dates and times ({func}`~hegel.generators.dates`,
{func}`~hegel.generators.datetimes`, {func}`~hegel.generators.timedeltas`, ...),
UUIDs, IP addresses, email addresses, URLs and domain names, and strings
matching a regular expression ({func}`~hegel.generators.from_regex`).

## Collections

Collection generators take a generator for their elements:

```python
@hegel.test
def test_collections(tc):
    xs = tc.draw(gs.lists(gs.integers(), max_size=10))
    unique = tc.draw(gs.lists(gs.integers(), unique=True))
    tags = tc.draw(gs.sets(gs.text(min_size=1)))
    ages = tc.draw(gs.dicts(gs.text(), gs.integers(min_value=0)))
    point = tc.draw(gs.tuples(gs.floats(), gs.floats()))
```

{func}`~hegel.generators.fixed_dicts` generates dictionaries with a known set
of keys, each with its own generator, and is a good fit for JSON-like records:

```python
user = gs.fixed_dicts(
    {"name": gs.text(min_size=1), "age": gs.integers(0, 120)},
    optional={"email": gs.emails()},
)
```

## Choosing between values

{func}`~hegel.generators.sampled_from` picks one of a fixed collection of values,
or one member of an `Enum`. {func}`~hegel.generators.just` always produces the
same value, and {func}`~hegel.generators.none` always produces `None`:

```python
colour = gs.sampled_from(["red", "green", "blue"])
```

{func}`~hegel.generators.one_of`, or the `|` operator, draws from one of
several generators. {func}`~hegel.generators.optional` is shorthand for a
generator or `None`:

```python
number = gs.integers() | gs.floats()
maybe_name = gs.optional(gs.text())
```

## Transforming generators

Every generator has three methods that build new generators from it.

{meth}`~hegel.generators.Generator.map` transforms each value:

```python
even = gs.integers().map(lambda n: n * 2)
```

{meth}`~hegel.generators.Generator.filter` keeps only the values satisfying a
condition:

```python
odd = gs.integers().filter(lambda n: n % 2 == 1)
```

A filter that rejects most values makes the test slow, because Hegel has to
throw away and redraw what it generates. Prefer `map`, or an argument of the
generator, when one expresses the same thing: `gs.integers().map(lambda n: 2 * n
+ 1)` generates odd numbers without ever rejecting one.

{meth}`~hegel.generators.Generator.flat_map` draws a value and then uses it to
choose the next generator:

```python
@hegel.test
def test_flat_map(tc):
    xs = tc.draw(
        gs.integers(min_value=1, max_value=10).flat_map(
            lambda n: gs.lists(gs.integers(), min_size=n, max_size=n)
        )
    )
    assert 1 <= len(xs) <= 10
```

In a Hegel test you rarely need `flat_map`: drawing one value and then
drawing from a generator built with it, as in {doc}`introduction`, does the
same thing and is easier to read.

## Generating your own types

{func}`~hegel.generators.builds` calls a class or function with arguments
drawn from generators. Arguments you don't give a generator for are inferred
from their type annotations:

```python
from dataclasses import dataclass


@dataclass
class Point:
    x: float
    y: float
    label: str = ""


points = gs.builds(Point, x=gs.floats(-1, 1), y=gs.floats(-1, 1))
```

{func}`~hegel.generators.from_type` goes further and builds a generator from a
type alone, covering built-in and standard-library types, `typing` constructs
such as `list[int]` and `int | None`, enums, dataclasses and annotated
classes:

```python
points = gs.from_type(Point)
pairs = gs.from_type(tuple[int, list[str]])
```

When your types need more control than `builds` gives, write a generator with
{func}`@hegel.composite <hegel.composite>`, the subject of the next page.
