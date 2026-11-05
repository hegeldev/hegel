"""Bank workload that tests an ACID database with Hegel's concurrent
stateful testing.

Each run of this file runs the workload once.

The state is a file-backed SQLite database in WAL mode with one
``accounts(account_id, balance, opCount)`` table, the workload's client-side
counts (attempted, committed, total), and a pool of idle connections that
rules use for each transaction.

In WAL mode, the verification rules can read while transfers are running,
without blocking them or failing with SQLITE_BUSY, and each read sees a
consistent snapshot. Writes are still serialized, so transfers take the write
lock at the start with ``BEGIN IMMEDIATE``.
"""

from __future__ import annotations

import contextlib
import enum
import os
import sqlite3
import tempfile
import threading
from collections.abc import Generator
from dataclasses import dataclass
from types import TracebackType

import hegel
from hegel import generators as gs

BUSY_TIMEOUT = 5.0
"""How many seconds a connection waits on a locked database before failing
with SQLITE_BUSY. A transfer that fails this way counts as attempted, but not
known to have committed."""

INJECT_BUG = "BANK_INJECT_BUG" in os.environ
"""Set ``BANK_INJECT_BUG`` to skip the balance check in transfers. The run
then fails the "no negative balance" check."""


@dataclass(frozen=True)
class Config:
    accounts: int
    """``N``, the number of accounts."""
    initial_balance: int
    """``B``, the initial balance of every account."""
    max_amount: int
    """The largest amount a transfer can move. It can be larger than ``B``, so
    that some transfers have insufficient funds."""


@dataclass(frozen=True)
class Workload:
    bank: Config
    steps: int
    min_concurrency: int
    max_concurrency: int


def env_int(name: str, *, minimum: int) -> int | None:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return None
    try:
        value = int(raw)
    except ValueError:
        raise ValueError(f"{name}={raw!r} is not an integer") from None
    if value < minimum:
        raise ValueError(f"{name}={value} must be at least {minimum}")
    return value


def env_int_or_draw(
    tc: hegel.TestCase, name: str, generator: hegel.Generator[int], *, minimum: int
) -> int:
    value = env_int(name, minimum=minimum)
    return tc.draw(generator) if value is None else value


def bank_workload(tc: hegel.TestCase) -> Workload:
    """Read the workload from the environment, and draw a value for each unset
    variable.

    ::

        BANK_ACCOUNTS         N, number of accounts (>= 1)          drawn, 1..16
        BANK_INITIAL_BALANCE  B, starting balance per account (>= 0) drawn, 0..1000
        BANK_MAX_AMOUNT       largest transfer amount (>= 0)        drawn, 0..2B+10
        BANK_STEPS            rounds per test case (>= 1)           200
        BANK_MIN_CONCURRENCY  fewest worker threads (>= 1)          2
        BANK_MAX_CONCURRENCY  most worker threads (>= min)          8
    """
    accounts = env_int_or_draw(
        tc, "BANK_ACCOUNTS", gs.integers(min_value=1, max_value=16), minimum=1
    )
    initial_balance = env_int_or_draw(
        tc, "BANK_INITIAL_BALANCE", gs.integers(min_value=0, max_value=1_000), minimum=0
    )
    max_amount = env_int_or_draw(
        tc,
        "BANK_MAX_AMOUNT",
        gs.integers(min_value=0, max_value=2 * initial_balance + 10),
        minimum=0,
    )
    min_concurrency = env_int("BANK_MIN_CONCURRENCY", minimum=1) or 2
    max_concurrency = env_int("BANK_MAX_CONCURRENCY", minimum=min_concurrency) or max(
        8, min_concurrency
    )
    return Workload(
        bank=Config(accounts, initial_balance, max_amount),
        steps=env_int("BANK_STEPS", minimum=1) or 200,
        min_concurrency=min_concurrency,
        max_concurrency=max_concurrency,
    )


class Outcome(enum.Enum):
    """What the client observed for a transfer attempt."""

    COMMITTED = "Committed"
    """``COMMIT`` returned successfully."""
    INSUFFICIENT_FUNDS = "Insufficient_funds"
    """Rolled back because the source balance was insufficient."""
    UNKNOWN = "Unknown"
    """SQLite returned an error, such as SQLITE_BUSY. Whether the transaction
    committed is unknown."""


class InsufficientFunds(Exception):
    pass


class Bank:
    def __init__(self, config: Config) -> None:
        """Create the ``accounts`` table with ``N`` rows of balance ``B`` and
        ``opCount`` 0."""
        self.config = config
        self.total = config.accounts * config.initial_balance
        self.attempted = 0
        self.committed = 0
        self._dir = tempfile.TemporaryDirectory(prefix="sqlite_bank")
        self._path = os.path.join(self._dir.name, "bank.sqlite3")
        self._lock = threading.Lock()
        self._idle: list[sqlite3.Connection] = []
        """Idle connections. Rules run concurrently on worker threads, so each
        rule takes a connection for the length of a transaction."""
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

    def transfer(self, *, source: int, dest: int, amount: int) -> Outcome:
        """Run the transfer transaction, and return what the client observed."""
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

    def verify_total_balance(self) -> None:
        """No money is created or destroyed."""
        with self.connection() as db:
            (total,) = db.execute("SELECT SUM(balance) FROM accounts").fetchone()
        assert total == self.total, (
            f"money was created or destroyed: SUM(balance) = {total}, expected {self.total}"
        )

    def verify_ops_not_lost(self) -> None:
        """No committed transfer is lost. ``committed`` is read before the
        snapshot, so every transfer it counts must appear in the snapshot.
        ``attempted`` is read after the snapshot, so it counts every transfer
        that appears in it."""
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
        """No account balance is below zero."""
        with self.connection() as db:
            (negative,) = db.execute("SELECT COUNT(*) FROM accounts WHERE balance < 0").fetchone()
        assert negative == 0, f"{negative} account(s) have a negative balance"


class Machine(hegel.StateMachine):
    """All rules are in the ``"bank"`` group, so any two of them can run at the
    same time. Each verify_* check is both a rule and an invariant. As rules,
    they run while transfers are running, and check what a reader sees during
    a write. As invariants, they run on the final state and sometimes between
    rounds, so a lost commit is caught by the end of the run."""

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


def run(tc: hegel.TestCase, workload: Workload) -> None:
    tc.note(
        f"workload: accounts = {workload.bank.accounts}, "
        f"initial_balance = {workload.bank.initial_balance}, "
        f"max_amount = {workload.bank.max_amount}, steps = {workload.steps}, "
        f"concurrency = [{workload.min_concurrency}, {workload.max_concurrency}]"
    )
    with Bank(workload.bank) as bank:
        hegel.run_state_machine(
            tc,
            Machine(bank),
            steps=workload.steps,
            min_concurrency=workload.min_concurrency,
            max_concurrency=workload.max_concurrency,
        )


@hegel.main(show_statistics=True)
def workload(tc: hegel.TestCase) -> None:
    run(tc, bank_workload(tc))


if __name__ == "__main__":
    workload()
