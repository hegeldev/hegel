# Port tests from Hypothesis

Hegel for Python is based on Hypothesis, and most Hypothesis tests translate
into Hegel tests mechanically. This guide gives the translation. For the full
list of what each library has that the other lacks, see
{doc}`../explanation/comparison-with-hypothesis`.

## Tests

A Hypothesis test declares its inputs with `@given`. A Hegel test draws them in
its body, in the order the arguments were declared:

```python
# Hypothesis
from hypothesis import given, strategies as st


@given(st.lists(st.integers()), st.integers())
def test_insert(xs, x):
    ...
```

```python
# Hegel
import hegel
from hegel import generators as gs


@hegel.test
def test_insert(tc):
    xs = tc.draw(gs.lists(gs.integers()))
    x = tc.draw(gs.integers())
    ...
```

Draw into variables with the same names as the old arguments, and the failure
report reads much as Hypothesis's did. A test that used `st.data()` already
draws in its body: replace `data.draw(...)` with `tc.draw(...)`.

## Strategies

`hypothesis.strategies` becomes `hegel.generators`. Most strategies keep their
names and arguments. The ones that change:

| Hypothesis | Hegel |
| --- | --- |
| `st.dictionaries(k, v)` | `gs.dicts(k, v)` |
| `st.fixed_dictionaries(m)` | `gs.fixed_dicts(m)` |
| `st.from_regex(p)` | `gs.from_regex(p, fullmatch=False)` |
| `st.data()` | the test case itself |
| `@st.composite` with `draw` as the first argument | `@hegel.composite` with the test case as the first argument, drawing with `tc.draw` |

`gs.from_regex` defaults to matching the whole string. Pass
`fullmatch=False` to keep Hypothesis's default, though matching the whole
pattern is usually what a test wants.

## Assumptions, notes, events and targets

These are module-level functions in Hypothesis and methods of the test case in
Hegel:

| Hypothesis | Hegel |
| --- | --- |
| `assume(condition)` | `tc.assume(condition)` |
| `note(message)` | `tc.note(message)` |
| `event(label)` | `tc.event(label)` |
| `target(score, label=...)` | `tc.target(score, label)` |

A helper function that called `assume` or `note` needs the test case passed
to it.

## Settings

`@settings(...)` becomes keyword arguments to `@hegel.test`, and a few settings
are renamed:

| Hypothesis | Hegel |
| --- | --- |
| `@settings(max_examples=500)` | `@hegel.test(test_cases=500)` |
| `report_multiple_bugs=True` | `report_multiple_failures=True` |
| `database=None` | `database=""` |
| `@example(...)` | `@hegel.explicit_test_case(...)` |
| `settings.register_profile(...)` | `hegel.Settings.register_profile(...)` |

`deadline` has no equivalent and can be dropped. Explicit test cases supply
draws, by name or in order, rather than arguments; see
{doc}`../tutorial/failures`.

## State machines

A `RuleBasedStateMachine` becomes a {class}`hegel.StateMachine`. Rules and
invariants take the test case and draw their own arguments, rather than
receiving them from strategies in `@rule(...)`. A precondition becomes a call
to `tc.assume` at the start of the rule, and a `Bundle` becomes a
{class}`hegel.Pool`:

```python
# Hypothesis
class Machine(RuleBasedStateMachine):
    keys = Bundle("keys")

    @rule(target=keys, k=st.text())
    def add_key(self, k):
        return k

    @precondition(lambda self: self.ready)
    @rule(k=keys, v=st.integers())
    def set_value(self, k, v):
        ...


TestMachine = Machine.TestCase
```

```python
# Hegel
class Machine(hegel.StateMachine):
    def __init__(self, tc):
        self.keys = hegel.Pool(tc)
        self.ready = False

    @hegel.rule()
    def add_key(self, tc):
        self.keys.add(tc.draw(gs.text()))

    @hegel.rule()
    def set_value(self, tc):
        tc.assume(self.ready)
        k = tc.draw(self.keys.values_reusable())
        v = tc.draw(gs.integers())
        ...


test_machine = hegel.state_machine_test(Machine)
```

## Exceptions

`InvalidArgument`, `Unsatisfiable`, `FailedHealthCheck`, `DidNotReproduce`,
`Flaky` and `MultipleFailures` have the same names in `hegel`.
`UnsatisfiedAssumption` is {class}`hegel.AssumeFailed`.

## Letting a model do it

The translation is regular enough that a coding assistant can port a test
suite for you. Give it this page and
{doc}`../explanation/comparison-with-hypothesis`, and have it run the tests
as it goes.
