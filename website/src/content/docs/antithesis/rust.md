---
title: Using Hegel for Rust for Antithesis workloads
sidebar:
  label: Rust
---

This is a simple test workload we call the bank test, which checks atomicity and isolation in a SQLite database and a functional correctness property
on a model of a bank. Note that some code is omitted for brevity. See [Further reading](#further-reading) for the full code.

## Bank state

```rust
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
pub struct BankConfig {
    pub accounts: i64,
    pub initial_balance: i64,
    pub max_amount: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkloadConfig {
    pub bank: BankConfig,
    pub steps: i64,
    pub min_concurrency: i64,
    pub max_concurrency: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOutcome {
    Committed,
    InsufficientFunds,
    Error,
}

pub struct Bank {
    _dir: TempDir,
    path: PathBuf,
    pool: Mutex<Vec<Connection>>,
    config: BankConfig,
    attempted: AtomicU64,
    committed: AtomicU64,
    total: i64,
}
```

`BUSY_TIMEOUT` is how long a connection waits on a locked database before it fails with `SQLITE_BUSY`.

The database has one `accounts` table with an `account_id`, a `balance` and an `opCount` for each account.

The database runs in WAL mode, which allows non-blocking reads while transfers are executing, and each read sees a consistent snapshot. Since writers are serialized, transfers must take the write lock first with `BEGIN IMMEDIATE`.

`BankConfig` sets the number of accounts, the starting balance of each account, and the largest transfer amount.

Transfers can have one of the following outcomes (`TransferOutcome`):

- `Committed`: `COMMIT` returned successfully.
- `InsufficientFunds`: the transaction was rolled back because the source balance was too low.
- `Error`: SQLite returned an error, such as `SQLITE_BUSY`. The client does not know if the transaction committed.

The test state `Bank` consists of the database path, a pool of idle connections, counts of attempted and committed transfers, and the total balance. Mutable fields of `Bank` are
atomic or guarded by a mutex since they may be modified
concurrently.

## Database helpers

```rust
fn with_conn<T>(
    &self,
    f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    let pooled = self.pool.lock().unwrap_or_else(|e| e.into_inner()).pop();
    let mut conn = match pooled {
        Some(conn) => conn,
        None => open(&self.path)?,
    };
    let result = f(&mut conn);
    self.pool
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(conn);
    result
}

fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(conn)
}
```

`open` opens a connection with the busy timeout set. `with_conn` takes an idle connection from the pool or opens a new one if the pool is empty, runs a function on it, and returns it to the pool.

## Setup

```rust
pub fn init(config: BankConfig) -> rusqlite::Result<Self> {
    let dir = tempfile::tempdir().expect("create a temporary directory for the database");
    let path = dir.path().join("bank.sqlite3");

    let conn = open(&path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.execute_batch(
        "CREATE TABLE accounts (
             account_id INTEGER PRIMARY KEY,
             balance    INTEGER NOT NULL,
             opCount    INTEGER NOT NULL
         );",
    )?;
    {
        let tx = conn.unchecked_transaction()?;
        let mut insert = tx.prepare(
            "INSERT INTO accounts (account_id, balance, opCount) VALUES (?1, ?2, 0)",
        )?;
        for id in 0..config.accounts {
            insert.execute(params![id, config.initial_balance])?;
        }
        drop(insert);
        tx.commit()?;
    }

    Ok(Bank {
        _dir: dir,
        path,
        pool: Mutex::new(vec![conn]),
        total: config.accounts * config.initial_balance,
        config,
        attempted: AtomicU64::new(0),
        committed: AtomicU64::new(0),
    })
}
```

`Bank::init` creates the database in a temporary directory, switches it to WAL mode, and creates the `accounts` table with one row per account. Each account starts with the initial balance and an `opCount` of 0. `Bank::init` also records the expected total `config.accounts * config.initial_balance`. The bank owns its connections and temporary directory, which are cleaned up when the bank is dropped.

`with_conn` takes an idle connection from the pool or opens a new one if the pool is empty, and returns it to the pool when the function returns.

## Transfers

```rust
pub fn transfer(&self, source: i64, dest: i64, amount: i64) -> TransferOutcome {
    self.attempted.fetch_add(1, Ordering::SeqCst);
    let result = self.with_conn(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let balance: i64 = tx.query_row(
            "SELECT balance FROM accounts WHERE account_id = ?1",
            params![source],
            |row| row.get(0),
        )?;
        let sufficient = cfg!(feature = "inject-bug") || balance >= amount;
        if !sufficient {
            tx.rollback()?;
            return Ok(TransferOutcome::InsufficientFunds);
        }
        tx.execute(
            "UPDATE accounts SET balance = balance - ?1, opCount = opCount + 1 WHERE account_id = ?2",
            params![amount, source],
        )?;
        tx.execute(
            "UPDATE accounts SET balance = balance + ?1, opCount = opCount + 1 WHERE account_id = ?2",
            params![amount, dest],
        )?;
        tx.commit()?;
        Ok(TransferOutcome::Committed)
    });
    match result {
        Ok(outcome) => {
            if outcome == TransferOutcome::Committed {
                self.committed.fetch_add(1, Ordering::SeqCst);
            }
            outcome
        }
        Err(_) => TransferOutcome::Error,
    }
}
```

`transfer` increments `attempted`, then runs one transaction. If the source balance is less than the transfer amount, the transaction is rolled back. Otherwise, `transfer` subtracts the amount from the source, adds the amount to the destination, and increments `opCount` on both accounts. If a statement returns an error before commit, the transaction is rolled back when it is dropped.

If the transaction commits, `transfer` increments `committed`. Any SQLite error becomes `TransferOutcome::Error`.

## Properties

```rust
pub fn verify_total_balance(&self) {
    let sum: i64 = self
        .with_conn(|conn| conn.query_row("SELECT SUM(balance) FROM accounts", [], |r| r.get(0)))
        .expect("SELECT SUM(balance) must succeed");
    assert_eq!(
        sum, self.total,
        "money was created or destroyed: SUM(balance) = {sum}, expected {}",
        self.total
    );
}

pub fn verify_ops_not_lost(&self) {
    let committed = self.committed();
    let sum: i64 = self
        .with_conn(|conn| conn.query_row("SELECT SUM(opCount) FROM accounts", [], |r| r.get(0)))
        .expect("SELECT SUM(opCount) must succeed");
    let attempted = self.attempted();
    assert!(
        sum % 2 == 0,
        "SUM(opCount) = {sum} is odd: a transfer was half-applied"
    );
    assert!(
        sum >= 2 * committed as i64,
        "committed transfers were lost: SUM(opCount) = {sum} < 2 * {committed} committed"
    );
    assert!(
        sum <= 2 * attempted as i64,
        "phantom transfers: SUM(opCount) = {sum} > 2 * {attempted} attempted"
    );
}

pub fn verify_no_negative(&self) {
    let negative: i64 = self
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM accounts WHERE balance < 0", [], |r| {
                r.get(0)
            })
        })
        .expect("SELECT COUNT(*) must succeed");
    assert_eq!(negative, 0, "{negative} account(s) have a negative balance");
}
```

`verify_total_balance` checks that `SUM(balance)` equals the starting total (i.e. no money was created or destroyed).

`verify_ops_not_lost` checks `SUM(opCount)`. Each transfer increments two counts, so the sum must be even. The sum must be at least `2 * committed`, or a committed transfer was lost. It can be higher because some transfers can commit without the client knowing, but it cannot exceed `2 * attempted`.

`verify_no_negative` checks that no account has a balance below zero. If two transfers
both read the old balance before either withdraws, the account can be overdrawn.

## Rules and invariants

```rust
#[hegel::concurrent_state_machine]
impl Bank {
    #[rule(groups = ["bank"], weight = 6)]
    fn transfer_rule(&self, tc: TestCase) {
        let n = self.config.accounts;
        let source = tc.draw(gs::integers::<i64>().min_value(0).max_value(n - 1));
        let dest = tc.draw(gs::integers::<i64>().min_value(0).max_value(n - 1));
        let amount = tc.draw(
            gs::integers::<i64>()
                .min_value(0)
                .max_value(self.config.max_amount),
        );
        let outcome = self.transfer(source, dest, amount);
        tc.event(format!("transfer: {outcome:?}"));
        tc.note(&format!(
            "transfer {amount} from {source} to {dest}: {outcome:?}"
        ));
    }

    #[rule(groups = ["bank"])]
    fn verify_total_balance_rule(&self, _: TestCase) {
        self.verify_total_balance();
    }

    #[rule(groups = ["bank"])]
    fn verify_ops_not_lost_rule(&self, _: TestCase) {
        self.verify_ops_not_lost();
    }

    #[rule(groups = ["bank"])]
    fn verify_no_negative_rule(&self, _: TestCase) {
        self.verify_no_negative();
    }

    #[invariant]
    fn total_balance_preserved(&self, _: TestCase) {
        self.verify_total_balance();
    }

    #[invariant]
    fn no_committed_transfer_lost(&self, _: TestCase) {
        self.verify_ops_not_lost();
    }

    #[invariant]
    fn no_negative_balance(&self, _: TestCase) {
        self.verify_no_negative();
    }
}
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
because it requires checking all accounts. Using `#[invariant(always_run)]` forces the invariant
to always be checked between rounds.

Notice that we define the verification functions both as invariants and rules. Invariants check
the state of the system at some settled state when no actions are running. When we define the
verification function as a rule, they can run while other actions are mutating the state.

For example, the invariant `total_balance_preserved` checks that after all transfers are done,
the total amount of money has not changed. The rule `verify_total_balance_rule` checks that while
transfers are executing, we never see a state in which the total amount of money has changed.

## Running the workload

```rust
pub fn bank_machine(tc: &TestCase, config: WorkloadConfig) -> Machine<Bank> {
    let bank = Bank::init(config.bank).expect("initialise the bank database");
    tc.note(&format!("workload: {config:?}"));
    machine(bank)
        .steps(i64::MAX)
        .min_concurrency(config.min_concurrency)
        .max_concurrency(config.max_concurrency)
}

#[hegel::main]
fn main(tc: TestCase) {
    let config = WorkloadConfig::draw(&tc);
    bank_machine(&tc, config).run_concurrent(tc);
}
```

`#[hegel::main]` turns the workload into a standalone program, which allows us to pass test settings as command line options,
such as `cargo run --release -- --seed 1 --verbosity verbose`.

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

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/rust/examples/sqlite).

For more information on concurrent stateful testing, see the [documentation](https://docs.rs/hegeltest/latest/hegel/stateful/index.html#concurrent-state-machines).
