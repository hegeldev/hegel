# Stateful testing

The tests so far draw some values, do something with them, and check the
result. Many bugs only show up after a particular *sequence* of operations:
a cache that goes wrong after an eviction followed by an update, or a file
format that breaks when a record is deleted and then re-added. Stateful
testing has Hegel generate the sequence of operations too.

## A state machine

A state machine test describes the operations, called *rules*, that can be
applied to the system under test, and the *invariants* that should always
hold. Here is one for a queue with a bug in it:

```python
import hegel
from hegel import generators as gs


class Queue:
    def __init__(self):
        self.items = []

    def push(self, x):
        self.items.append(x)

    def pop(self):
        return self.items.pop()  # Bug: this takes from the wrong end.


class QueueMachine(hegel.StateMachine):
    def __init__(self):
        self.queue = Queue()
        self.model = []

    @hegel.rule()
    def push(self, tc):
        x = tc.draw(gs.integers())
        self.queue.push(x)
        self.model.append(x)

    @hegel.rule()
    def pop(self, tc):
        tc.assume(self.model)
        assert self.queue.pop() == self.model.pop(0)

    @hegel.invariant()
    def same_length(self, tc):
        assert len(self.queue.items) == len(self.model)


test_queue = hegel.state_machine_test(QueueMachine)
```

The machine keeps a simple *model* of what the queue should contain, a plain
list, alongside the real queue, and checks that they agree.
{func}`~hegel.state_machine_test` makes a test function for pytest to collect.
Each test case creates a fresh instance of the machine, then applies a
sequence of rules chosen by Hegel, checking the invariants as it goes.

Invariants are always checked on the initial state and at the end, and only
some of the time in between. Pass
`always_run=True` to {func}`@hegel.invariant <hegel.invariant>` for an
invariant that should be checked after every step.

Rules and invariants are methods taking the test case, and draw any values
they need from it. A rule that doesn't apply in the current state calls
{meth}`~hegel.TestCase.assume`, as `pop` does when the queue is empty: Hegel
discards that step and picks another.

When the test fails, Hegel shrinks the sequence of steps, as well as the
values drawn in each step, and reports a minimal one:

```
E       assert 1 == 0
E       Checking invariants on the initial state.
E       Step 1: push {
E         x = 0
E       }
E       Step 2: push {
E         x = 1
E       }
E       Step 3: pop {
E       }
E       Rule pop failed:
```

The constructor takes either no arguments, as here, or the test case, so that
it can draw the machine's initial state.

## Pools of values

Some rules create things that other rules use later: one rule opens an
account, others deposit into it or close it. A {class}`hegel.Pool` holds such
values. Rules add values to it, and draw from it with
{meth}`~hegel.Pool.values_reusable`, which leaves the value in the pool, or
{meth}`~hegel.Pool.values_consumed`, which removes it:

```python
class Accounts(hegel.StateMachine):
    def __init__(self, tc):
        self.accounts = hegel.Pool(tc)
        self.balances = {}

    @hegel.rule()
    def open_account(self, tc):
        name = tc.draw(gs.text(min_size=1, max_size=5))
        tc.assume(name not in self.balances)
        self.balances[name] = 0
        self.accounts.add(name)

    @hegel.rule()
    def deposit(self, tc):
        name = tc.draw(self.accounts.values_reusable())
        amount = tc.draw(gs.integers(1, 100))
        self.balances[name] += amount

    @hegel.rule()
    def close_account(self, tc):
        name = tc.draw(self.accounts.values_consumed())
        assert self.balances.pop(name) >= 0


test_accounts = hegel.state_machine_test(Accounts)
```

Drawing from an empty pool discards the step, so `deposit` and
`close_account` only run once an account exists.

## Machines without classes

{class}`hegel.Machine` builds the same kind of state machine from functions,
inside an ordinary Hegel test, which is convenient when the state is a few
local variables:

```python
@hegel.test
def test_stack(tc):
    stack = []
    machine = hegel.Machine(tc)

    @machine.rule("push")
    def push(tc):
        x = tc.draw(gs.integers())
        stack.append(x)

    @machine.rule("pop")
    def pop(tc):
        tc.assume(stack)
        stack.pop()

    @machine.invariant("bounded")
    def bounded(tc):
        assert len(stack) <= 50

    machine.run()
```

## Concurrency

Setting the class attribute `max_concurrency` above 1 runs the machine's rules
from several threads at once, against the same instance, to find bugs that
only appear under concurrent use. Hegel chooses how many threads to use, and
runs the rules in rounds; a rule's `group` controls which rules may run in the
same round. Share values between threads with a {class}`hegel.ConcurrentPool`
rather than a `Pool`. {doc}`../reference/stateful` has the details.
