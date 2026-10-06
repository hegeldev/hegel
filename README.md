# Hegel for Python

Property-based testing for Python, built on the [Hegel](https://github.com/hegeldev)
engine (`libhegel`). The engine generates inputs, shrinks failures, keeps an example
database and prints the failing test case; this package binds it to Python through
a [pyo3](https://pyo3.rs) extension module and provides the test-writing API.

Two APIs are provided:

- `hegel`: the Hegel API, shared in shape with the Rust, Go and TypeScript
  bindings. Tests take a `TestCase` and draw values from generators.
- `hegel.hypothesis`: [Hypothesis](https://hypothesis.readthedocs.io)'s API
  (`@given`, `strategies`, `settings`, `RuleBasedStateMachine`, ...) reimplemented
  on top of Hegel, so existing Hypothesis tests can run on the Hegel engine with an
  import change. The parts of Hypothesis it does not cover are listed in
  `tests/hypothesis_port/GAPS.md`.

## Installation

Hegel for Python needs Python 3.11 or later and, until wheels are published, a Rust
toolchain:

```
pip install git+https://github.com/hegeldev/hegel-python
```

## Quickstart

```python
import hegel
from hegel import generators as gs


@hegel.test
def test_sorting_is_idempotent(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    assert sorted(sorted(xs)) == sorted(xs)
```

The test runs under pytest (or by calling it). When it fails, Hegel shrinks the
input and reports the minimal failing example, named after the variables the draws
were assigned to:

```
Falsifying example:
xs = [0, 0]
```

### Generators

`hegel.generators` provides generators for integers, floats, booleans, text
(by codec, category or alphabet, and from regular expressions), bytes, dates and
times, UUIDs, IP addresses, emails, URLs and domains, lists, sets, dicts, tuples,
permutations and samples, and combinators: `map`, `filter`, `flat_map`,
`one_of` (or `|`), `optional`, `just`, `sampled_from`, `recursive`, `deferred`,
`composite`, `builds` and `from_type`.

```python
@hegel.composite
def sorted_pairs(tc, limit):
    a = tc.draw(gs.integers(0, limit))
    b = tc.draw(gs.integers(a, limit))
    return (a, b)
```

### Settings

`@hegel.test(test_cases=1000, seed=42)` sets options for one test;
`hegel.Settings.register_profile("ci", derandomize=True)` and
`hegel.Settings.load_profile("ci")` set them globally. Settings cover the number of
test cases, verbosity, seeding and derandomization, the example database, phases,
health checks, multiple-failure reporting and the random backend.

### Explicit test cases and reproduction

```python
@hegel.explicit_test_case(xs=[1, 1])
@hegel.test
def test_no_duplicates(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    ...
```

A failure report ends with a `@hegel.reproduce_failure("...")` line; add that
decorator above `@hegel.test` to replay exactly that failure.

### Stateful testing

```python
class Counter(hegel.StateMachine):
    def __init__(self):
        self.count = 0

    @hegel.rule()
    def increment(self, tc):
        self.count += tc.draw(gs.integers(1, 10))

    @hegel.invariant()
    def non_negative(self, tc):
        assert self.count >= 0


test_counter = hegel.state_machine_test(Counter)
```

Setting `max_concurrency` above 1 on the class runs the rules from several threads
at once, in rounds grouped by the rules' `group`, with Hegel choosing the
concurrency level and attributing the output to each worker. `hegel.Pool` and
`hegel.ConcurrentPool` hold values rules produce for later rules to consume.

## Hypothesis on Hegel

```python
from hegel.hypothesis import given, strategies as st


@given(st.lists(st.integers()))
def test_reversing_twice(xs):
    assert list(reversed(list(reversed(xs)))) == xs
```

`hegel.hypothesis` covers `given`, `example`, `settings` and profiles, `seed`,
`reproduce_failure`, `find`, `assume`/`note`/`event`/`target`, the strategies
module, the errors module, and `hegel.hypothesis.stateful` with
`RuleBasedStateMachine`, `Bundle`, `rule`, `initialize`, `precondition`,
`invariant`, `consumes` and `multiple`. Hypothesis's own test suite is ported under
`tests/hypothesis_port`.

## Development

```
just setup        # create the virtualenv and build the extension
just check        # formatting, lint, types, tests with coverage, the ported suite
```
