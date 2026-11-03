# Draw from several threads

A test of concurrent code often wants each thread to make its own choices: how
many operations to perform, and with which values. A
{class}`~hegel.TestCase` may only be used by one thread at a time, so give each
thread a *clone* of it with {meth}`TestCase.clone() <hegel.TestCase.clone>`:

```python
import threading

import hegel
from hegel import generators as gs


class Counter:
    def __init__(self):
        self.value = 0
        self.lock = threading.Lock()

    def add(self, n):
        with self.lock:
            self.value += n


@hegel.test
def test_concurrent_adds(tc):
    counter = Counter()
    clones = [tc.clone() for _ in range(2)]
    added = [[] for _ in clones]
    errors = []

    def worker(i):
        try:
            clones[i].set_worker(i)
            for _ in range(clones[i].draw(gs.integers(0, 3), name="adds")):
                n = clones[i].draw(gs.integers(1, 10), name="n")
                counter.add(n)
                added[i].append(n)
        except BaseException as error:
            errors.append(error)

    threads = [threading.Thread(target=worker, args=(i,)) for i in range(2)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    if errors:
        raise errors[0]
    assert counter.value == sum(map(sum, added))
```

Make the clones before starting the threads, one per thread. Each clone draws
from a choice stream of its own, so one thread's draws never shift another's,
and the test case as a whole still shrinks and replays like any other.

A draw can end the test case by raising (when the test case is rejected, say,
or has used up its choices), so catch everything a thread raises and re-raise
it from the test once the threads have finished, as `worker` does above.
Letting it escape the thread would lose it.

{meth}`TestCase.set_worker() <hegel.TestCase.set_worker>` is optional: it
attributes everything a clone prints to a numbered worker, with the time since
the test case started, so that a report shows how the threads' draws
interleaved. Had `Counter.add` added `min(n, 9)` instead of `n`, the failure
would read:

```
E       assert 9 == 10
E       [worker 0 +0.054ms] adds_1 = 0
E       [worker 1 +0.109ms] adds_2 = 1
E       [worker 1 +0.122ms] n_1 = 10
```

The clones share draw names with the test case and with each other, so a name
drawn on several of them is numbered across all of them (`adds_1`, `adds_2`).

The scheduling of the threads is up to the operating system, not Hegel, so a
bug that needs a particular interleaving to show may not reproduce on every
replay. Hegel confirms such failures by replaying them several times and
reports how reliably they reproduced; see {doc}`../explanation/nondeterminism`.
For tests that call the operations of a system from several threads in rounds
that Hegel schedules, see the concurrent state machines in
{doc}`../tutorial/stateful`.
