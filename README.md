> [!IMPORTANT]
> We're excited you're checking out Hegel! Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel-python/issues/new).
>
> As part of our beta, we may make breaking changes if it makes Hegel a better property-based testing library. If that instability bothers you, please check back in a few months for a stable release!
>
> See https://hegel.dev/compatibility for more details.

# Hegel for Python

* [Website](https://hegel.dev)

Property-based testing for Python, built on the
[Hegel](https://github.com/hegeldev/hegel-rust) engine (`libhegel`). The engine
generates inputs, shrinks failures, keeps an example database and prints the
failing test case; this package binds it to Python through a
[pyo3](https://pyo3.rs) extension module and provides the test-writing API.

The API is the Hegel API, shared in shape with the Rust, Go and TypeScript
bindings: tests take a `TestCase` and draw values from generators. How it
compares with [Hypothesis](https://hypothesis.readthedocs.io) is set out in
`docs/comparison-with-hypothesis.md`; engine behaviour that differs from
Python's `re` or from Hypothesis is recorded in `docs/engine-issues.md`.

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

### Draw names

A draw assigned to a plain variable (`xs = tc.draw(...)`) is printed under that
name. A draw made in a loop, or assigned to a name that is assigned more than once
in the test, is numbered (`x_1`, `x_2`, ...), and a draw that is not assigned to a
variable is printed as `draw_1`, `draw_2`, and so on, as in the Rust library:

```python
@hegel.test
def test_totals(tc):
    n = tc.draw(gs.integers(0, 10))
    for _ in range(n):
        x = tc.draw(gs.integers())
    total = tc.draw(gs.integers()) + tc.draw(gs.integers())
    ...
```

```
Falsifying example:
n = 2
x_1 = 0
x_2 = 0
draw_1 = 0
draw_2 = 0
```

`tc.draw(generator, name="...")` gives a draw an explicit (always numbered) name,
and `tc.draw_silent(generator)` draws without printing. Draws made inside a
`composite` generator are not printed separately.

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

### Test case methods

Besides `draw`, a test case has `assume(condition)` and `reject()` to discard the
current input, `note(message)` to add a line to the failure report,
`target(score, label)` to steer generation toward higher scores,
`event(label)` and `event_value(label, value)` for the run's statistics, and
`repeat(body)`, which runs `body` an engine-chosen number of times and then ends
the test case.

### Configuration

`@hegel.test(test_cases=1000, seed=42)` sets options for one test. Settings cover
the number of test cases, verbosity, seeding and derandomization, the example
database, phases, health checks, multiple-failure reporting and the random backend.

Settings start from a profile. The engine provides `development` (the default
locally), `ci` (chosen automatically on a CI server) and `workload` (under
Antithesis); a project can pick one in a `hegel.toml` file or with the
`HEGEL_DEFAULT_PROFILE` environment variable. Your own profiles are registered
in Python:

```python
hegel.Settings.register_profile("thorough", test_cases=10_000)
hegel.Settings.set_default_profile("thorough")
```

Failing examples are saved in `.hegel/examples` and replayed first on the next run.
`database="path"` keeps them elsewhere, and `database=""` turns the database off.

Under pytest, `--hegel-profile`, `--hegel-seed`, `--hegel-verbosity`,
`--hegel-derandomize` and `--hegel-database` adjust every Hegel test in the run.

### Explicit test cases and reproduction

```python
@hegel.explicit_test_case(xs=[1, 1])
@hegel.test
def test_no_duplicates(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    ...
```

A failure report ends with a `@hegel.reproduce_failure("...")` line; add that
decorator below `@hegel.test` to replay exactly that failure.

### Stateful testing

```python
class Counter(hegel.StateMachine):
    def __init__(self):
        self.count = 0

    @hegel.rule()
    def increment(self, tc):
        n = tc.draw(gs.integers(1, 10))
        self.count += n

    @hegel.invariant()
    def non_negative(self, tc):
        assert self.count >= 0


test_counter = hegel.state_machine_test(Counter)
```

Rules and invariants take the test case and draw their own values. The machine's
constructor takes either no arguments or the test case.

`hegel.Pool` holds values that rules produce for later rules to use, with
generators that draw a value and leave it in the pool or take it out:

```python
class Files(hegel.StateMachine):
    def __init__(self, tc):
        self.handles = hegel.Pool(tc)

    @hegel.rule()
    def open(self, tc):
        name = tc.draw(gs.text(min_size=1))
        self.handles.add(open_file(name))

    @hegel.rule()
    def close(self, tc):
        handle = tc.draw(self.handles.values_consumed())
        handle.close()
```

Setting `max_concurrency` above 1 on the class runs the rules from several threads
at once, in rounds grouped by the rules' `group`, with Hegel choosing the
concurrency level and attributing the output to each worker; `hegel.ConcurrentPool`
is the pool to share between them.

## Coming from Hypothesis

A Hypothesis test maps onto a Hegel one draw by draw:

```python
# Hypothesis                                  # Hegel
@given(st.lists(st.integers()))               @hegel.test
def test_reversing_twice(xs):                 def test_reversing_twice(tc):
    ...                                           xs = tc.draw(gs.lists(gs.integers()))
                                                  ...
```

`docs/comparison-with-hypothesis.md` lists the correspondences and the
differences. Hypothesis's own test suite, ported to the Hegel API, runs under
`tests/hypothesis_port`.

## Development

```
just setup        # create the virtualenv and build the extension
just check        # formatting, lint, types, tests with coverage, the ported suite
```
