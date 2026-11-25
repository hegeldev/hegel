---
title: Using Hegel for Python for Antithesis workloads
sidebar:
  label: Python
---

This is a simple test workload we call the bank test, which checks atomicity and isolation in a SQLite database and a functional correctness property
on a model of a bank. Note that some code is omitted for brevity. See [Further reading](#further-reading) for the full code.

## Bank state

```python
import hegel
from hegel import generators as gs

BUSY_TIMEOUT = 5.0

INJECT_BUG = "BANK_INJECT_BUG" in os.environ

@dataclass(frozen=True)
class Config:
    accounts: int
    initial_balance: int
    max_amount: int


@dataclass(frozen=True)
class Workload:
    bank: Config
    steps: int
    min_concurrency: int
    max_concurrency: int


class Outcome(enum.Enum):
    COMMITTED = "Committed"
    INSUFFICIENT_FUNDS = "Insufficient_funds"
    UNKNOWN = "Unknown"


class InsufficientFunds(Exception):
    pass
```

`BUSY_TIMEOUT` is how many seconds a connection waits on a locked database before it fails with `SQLITE_BUSY`.

The database has one `accounts` table with an `account_id`, a `balance` and an `opCount` for each account.

The database runs in WAL mode, which allows non-blocking reads while transfers are executing, and each read sees a consistent snapshot. Since writers are serialized, transfers must take the write lock first with `BEGIN IMMEDIATE`.

`Config` sets the number of accounts, the starting balance of each account, and the largest transfer amount.

Transfers can have one of the following outcomes (`Outcome`):

- `COMMITTED`: `COMMIT` returned successfully.
- `INSUFFICIENT_FUNDS`: the transaction was rolled back because the source balance was too low.
- `UNKNOWN`: SQLite returned an error, such as `SQLITE_BUSY`. The client does not know if the transaction committed.

The test state `Bank` consists of the database path, a pool of idle connections, counts of attempted and committed transfers, and the total balance. A lock guards the connection pool and updates to the transfer counters.

## Database helpers

```python
def _connect(self) -> sqlite3.Connection:
    return sqlite3.connect(
        self._path, timeout=BUSY_TIMEOUT, isolation_level=None, check_same_thread=False
    )

@contextlib.contextmanager
def connection(self) -> Generator[sqlite3.Connection]:
    with self._lock:
        db = self._idle.pop() if self._idle else self._connect()
    try:
        yield db
    finally:
        if db.in_transaction:
            db.close()
        else:
            with self._lock:
                self._idle.append(db)

@contextlib.contextmanager
def transaction(self) -> Generator[sqlite3.Connection]:
    with self.connection() as db:
        db.execute("BEGIN IMMEDIATE")
        try:
            yield db
            db.execute("COMMIT")
        except BaseException:
            with contextlib.suppress(sqlite3.Error):
                db.execute("ROLLBACK")
            raise
```

`_connect` opens a connection with the busy timeout set. `connection` takes an idle connection or opens a new one if the pool is empty. It returns the connection to the pool unless a transaction is still open, in which case it closes the connection. `transaction` takes the write lock with `BEGIN IMMEDIATE`, then commits on success or attempts a rollback if the body raises.

## Setup

```python
class Bank:
    def __init__(self, config: Config) -> None:
        self.config = config
        self.total = config.accounts * config.initial_balance
        self.attempted = 0
        self.committed = 0
        self._dir = tempfile.TemporaryDirectory(prefix="sqlite_bank")
        self._path = os.path.join(self._dir.name, "bank.sqlite3")
        self._lock = threading.Lock()
        self._idle: list[sqlite3.Connection] = []
        with contextlib.closing(self._connect()) as db:
            db.execute("PRAGMA journal_mode = WAL")
            db.execute("PRAGMA synchronous = FULL")
            db.execute(
                "CREATE TABLE accounts (account_id INTEGER PRIMARY KEY, "
                "balance INTEGER NOT NULL, opCount INTEGER NOT NULL)"
            )
            db.execute("BEGIN")
            db.executemany(
                "INSERT INTO accounts (account_id, balance, opCount) VALUES (?, ?, 0)",
                [(account, config.initial_balance) for account in range(config.accounts)],
            )
            db.execute("COMMIT")

    def __enter__(self) -> Bank:
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        for db in self._idle:
            db.close()
        self._idle.clear()
        self._dir.cleanup()
```

`Bank.__init__` creates the database in a temporary directory, switches it to WAL mode, and creates the `accounts` table with one row per account. Each account starts with the initial balance and an `opCount` of 0. `Bank.__init__` also records the expected total `config.accounts * config.initial_balance`. `Bank.__exit__` closes the connections in the connection pool and deletes the database files.

`connection` takes an idle connection from the pool or opens a new one if the pool is empty, and returns it to the pool when the function returns if no transaction is left open.

## Transfers

```python
def transfer(self, *, source: int, dest: int, amount: int) -> Outcome:
    with self._lock:
        self.attempted += 1
    try:
        with self.transaction() as db:
            (balance,) = db.execute(
                "SELECT balance FROM accounts WHERE account_id = ?", (source,)
            ).fetchone()
            if not INJECT_BUG and balance < amount:
                raise InsufficientFunds
            db.execute(
                "UPDATE accounts SET balance = balance - ?, opCount = opCount + 1 "
                "WHERE account_id = ?",
                (amount, source),
            )
            db.execute(
                "UPDATE accounts SET balance = balance + ?, opCount = opCount + 1 "
                "WHERE account_id = ?",
                (amount, dest),
            )
    except InsufficientFunds:
        return Outcome.INSUFFICIENT_FUNDS
    except sqlite3.Error:
        return Outcome.UNKNOWN
    with self._lock:
        self.committed += 1
    return Outcome.COMMITTED
```

`transfer` increments `attempted`, then runs one transaction. If the source balance is less than the transfer amount, the transaction is rolled back. Otherwise, `transfer` subtracts the amount from the source, adds the amount to the destination, and increments `opCount` on both accounts. If a statement raises an exception, `transaction` attempts a rollback and propagates the exception to `transfer`.

If the transaction commits, `transfer` increments `committed`. Any SQLite error becomes `Outcome.UNKNOWN`.

## Properties

```python
def verify_total_balance(self) -> None:
    with self.connection() as db:
        (total,) = db.execute("SELECT SUM(balance) FROM accounts").fetchone()
    assert total == self.total, (
        f"money was created or destroyed: SUM(balance) = {total}, expected {self.total}"
    )

def verify_ops_not_lost(self) -> None:
    committed = self.committed
    with self.connection() as db:
        (total,) = db.execute("SELECT SUM(opCount) FROM accounts").fetchone()
    attempted = self.attempted
    assert total % 2 == 0, f"SUM(opCount) = {total} is odd: a transfer was half-applied"
    assert total >= 2 * committed, (
        f"committed transfers were lost: SUM(opCount) = {total} < 2 * {committed} committed"
    )
    assert total <= 2 * attempted, (
        f"phantom transfers: SUM(opCount) = {total} > 2 * {attempted} attempted"
    )

def verify_no_negative(self) -> None:
    with self.connection() as db:
        (negative,) = db.execute("SELECT COUNT(*) FROM accounts WHERE balance < 0").fetchone()
    assert negative == 0, f"{negative} account(s) have a negative balance"
```

`verify_total_balance` checks that `SUM(balance)` equals the starting total (i.e. no money was created or destroyed).

`verify_ops_not_lost` checks `SUM(opCount)`. Each transfer increments two counts, so the sum must be even. The sum must be at least `2 * committed`, or a committed transfer was lost. It can be higher because some transfers can commit without the client knowing, but it cannot exceed `2 * attempted`.

`verify_no_negative` checks that no account has a balance below zero. If two transfers
both read the old balance before either withdraws, the account can be overdrawn.

## Rules and invariants

```python
class Machine(hegel.StateMachine):
    def __init__(self, bank: Bank) -> None:
        self.bank = bank

    @hegel.invariant
    def total_balance_preserved(self, tc: hegel.TestCase) -> None:
        self.bank.verify_total_balance()

    @hegel.invariant
    def no_committed_transfer_lost(self, tc: hegel.TestCase) -> None:
        self.bank.verify_ops_not_lost()

    @hegel.invariant
    def no_negative_balance(self, tc: hegel.TestCase) -> None:
        self.bank.verify_no_negative()

    @hegel.rule(groups=["bank"], weight=6)
    def transfer(self, tc: hegel.TestCase) -> None:
        n = self.bank.config.accounts
        source = tc.draw(gs.integers(min_value=0, max_value=n - 1))
        dest = tc.draw(gs.integers(min_value=0, max_value=n - 1))
        amount = tc.draw(gs.integers(min_value=0, max_value=self.bank.config.max_amount))
        outcome = self.bank.transfer(source=source, dest=dest, amount=amount).value
        tc.event(f"transfer: {outcome}")
        tc.note(f"transfer {amount} from {source} to {dest}: {outcome}")

    @hegel.rule(groups=["bank"])
    def verify_total_balance(self, tc: hegel.TestCase) -> None:
        self.bank.verify_total_balance()

    @hegel.rule(groups=["bank"])
    def verify_ops_not_lost(self, tc: hegel.TestCase) -> None:
        self.bank.verify_ops_not_lost()

    @hegel.rule(groups=["bank"])
    def verify_no_negative(self, tc: hegel.TestCase) -> None:
        self.bank.verify_no_negative()
```

We can now define our workload.

We are required to define actions on the state, which are called rules in Hegel. Rules can belong to
one or more groups. Only rules in the same group may execute concurrently. If no group is provided,
the rule belongs to a default group. In this case, all the rules belong to the `bank` group for
demonstration, but the group could have been omitted. Rules also have a weight, which gives Hegel
a hint of how frequently a rule should run relative to other rules in its group(s). The default
weight is 1.

We can optionally define statements about the system that should always be true, which are called
invariants. By default, invariants are sampled for performance. For example, if we had 1 million
accounts instead of 16, we might not want check `no_negative_balance` after every single round
because it requires checking all accounts. Using `@hegel.invariant(always_run=True)` forces the invariant
to always be checked between rounds.

Notice that we define the verification functions both as invariants and rules. Invariants check
the state of the system at some settled state when no actions are running. When we define the
verification function as a rule, they can run while other actions are mutating the state.

For example, the invariant `total_balance_preserved` checks that after all transfers are done,
the total amount of money has not changed. The rule `verify_total_balance` checks that while
transfers are executing, we never see a state in which the total amount of money has changed.

## Running the workload

```python
import sys

def run(tc: hegel.TestCase, workload: Workload) -> None:
    with Bank(workload.bank) as bank:
        hegel.run_state_machine(
            tc,
            Machine(bank),
            steps=sys.maxsize,
            min_concurrency=workload.min_concurrency,
            max_concurrency=workload.max_concurrency,
        )


@hegel.main(show_statistics=True)
def workload(tc: hegel.TestCase) -> None:
    run(tc, bank_workload(tc))


if __name__ == "__main__":
    workload()
```

`@hegel.main` turns the workload into a standalone program, which allows us to pass test settings as command line options,
such as `uv run python examples/sqlite_bank/bank.py --seed 1 --verbosity verbose`.

Since the account balances are stored in the database, we only pass the bank handle,
which holds the database path, the connection pool, the transfer counters, and the configuration. `steps` sets how many rounds execute within a test. Inside of Antithesis, this number should be very high to allow
the fuzzer ample time to explore a branch. Finally, `min_concurrency` and `max_concurrency` set the bounds for the number of concurrent workers. Hegel spawns some number of workers within those bounds.

<figure class="diagram">
<svg viewBox="0 0 724 200" width="100%" role="img" aria-label="Two rounds of the bank workload with three workers. Each round runs rules from the group bank. Workers run transfer and the verify rules concurrently, and the invariants run after every worker finishes the round.">
<text x="0" y="65" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">Worker 1</text>
<text x="0" y="109" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">Worker 2</text>
<text x="0" y="153" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">Worker 3</text>
<text x="92" y="20" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-white)">Round 1: group “bank”</text>
<rect x="92" y="44" width="248" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="104" y="65" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, transfer, transfer</text>
<rect x="92" y="88" width="248" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="104" y="109" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, verify_no_negative</text>
<rect x="92" y="132" width="264" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="104" y="153" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">verify_total_balance, transfer</text>
<line x1="372" y1="36" x2="372" y2="174" style="stroke:var(--sl-color-accent-high);stroke-width:2"/>
<text x="372" y="194" text-anchor="middle" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">invariants</text>
<text x="402" y="20" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-white)">Round 2: group “bank”</text>
<rect x="402" y="44" width="256" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="414" y="65" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, verify_ops_not_lost</text>
<rect x="402" y="88" width="168" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="414" y="109" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, transfer</text>
<rect x="402" y="132" width="248" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="414" y="153" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, transfer, transfer</text>
<line x1="674" y1="36" x2="674" y2="174" style="stroke:var(--sl-color-accent-high);stroke-width:2"/>
<text x="674" y="194" text-anchor="middle" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">invariants</text>
<text x="704" y="109" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">…</text>
</svg>
</figure>

The diagram above shows how the workload is executed. In each round, Hegel chooses a group, then gives each worker a set of rules to run.
Each worker executes its rules concurrently with other workers. Once all workers have finished executing their rules in a round, Hegel runs
the invariants.

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/python/examples/sqlite_bank).

For more information on concurrent stateful testing, see the [documentation](https://hegel.readthedocs.io/en/latest/tutorial/stateful.html#concurrency).
