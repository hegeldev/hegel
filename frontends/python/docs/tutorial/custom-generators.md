# Writing your own generators

`map`, `filter` and `builds` cover a lot of ground, but some data is easier to
describe as a sequence of draws, each one depending on the ones before. The
{func}`@hegel.composite <hegel.composite>` decorator turns a function that
draws values into a generator.

## Composite generators

A composite function takes a test case as its first argument, draws whatever
it needs from it, and returns the value to generate:

```python
import hegel
from hegel import generators as gs


@hegel.composite
def people(tc):
    age = tc.draw(gs.integers(min_value=0, max_value=120))
    name = tc.draw(gs.text(min_size=1))
    return {"name": name, "age": age}


@hegel.test
def test_people(tc):
    person = tc.draw(people())
    assert person["age"] < 100
```

Calling `people()` returns a generator, which the test draws from like any
other. When the test fails, Hegel shrinks the person as a whole and reports the
value the composite returned:

```
E       assert 100 < 100
E       Falsifying example:
E       person = {'name': '0', 'age': 100}
```

The draws inside the composite are not reported separately, only its result.

Any arguments after the test case become arguments of the generator, so a
composite can be parameterised:

```python
@hegel.composite
def people(tc, max_age=120):
    age = tc.draw(gs.integers(min_value=0, max_value=max_age))
    name = tc.draw(gs.text(min_size=1))
    return {"name": name, "age": age}


children = people(max_age=17)
```

## Dependent draws

Because the body of a composite is ordinary Python, later draws can depend on
earlier ones. Here only adults may have a driving licence:

```python
@hegel.composite
def drivers(tc):
    age = tc.draw(gs.integers(min_value=0, max_value=120))
    licensed = tc.draw(gs.booleans()) if age >= 18 else False
    return {"age": age, "licensed": licensed}
```

And here the second value is always at least the first:

```python
@hegel.composite
def ordered_pairs(tc):
    low = tc.draw(gs.integers())
    high = tc.draw(gs.integers(min_value=low))
    return (low, high)
```

Generating values that satisfy a constraint by construction, like this, is
much better than generating any pair and filtering out the ones in the wrong
order: no work is wasted, and the values shrink well.

A composite can also call {meth}`~hegel.TestCase.assume` to discard a test case,
just as a test can.

## Recursive data

Trees, nested lists, JSON documents and other recursive data need a generator
that refers to itself. {func}`~hegel.generators.recursive` takes a generator
for the leaves and a function that, given a generator for sub-values, returns a
generator for one level of structure:

```python
json = gs.recursive(
    gs.none() | gs.booleans() | gs.floats(allow_nan=False) | gs.text(),
    lambda children: gs.lists(children) | gs.dicts(gs.text(), children),
    max_leaves=50,
)
```

`max_leaves` bounds the size of each value, so drawing one always terminates.

For data that is more naturally written as mutually recursive definitions, use
{func}`~hegel.generators.deferred`, which delays building a generator until it
is first drawn from, so it can refer to generators defined after it:

```python
trees = gs.deferred(lambda: gs.just("leaf") | gs.tuples(trees, trees))
```

A `deferred` generator has no size limit of its own, so give it base cases that
are common enough for its values to stay small, as `just("leaf")` is here.

## Making `from_type` know your types

{func}`~hegel.generators.register_type_generator` tells
{func}`~hegel.generators.from_type` (and the argument inference of
{func}`~hegel.generators.builds`) which generator to use for a type:

```python
class Temperature(float):
    pass


gs.register_type_generator(Temperature, gs.floats(-273.15, 1000).map(Temperature))
```

## Generator subclasses

The generators on this page cover almost every need. For the rare case they
don't, generators are subclasses of {class}`~hegel.generators.Generator`;
{doc}`../reference/generators` describes the interface.
