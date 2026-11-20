---
title: Using Hegel for Python for Antithesis workloads
sidebar:
  label: Python
---

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

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/python/examples/sqlite_bank).

For more information on concurrent stateful testing, see the [documentation](https://hegel.readthedocs.io/en/latest/tutorial/stateful.html#concurrency).
