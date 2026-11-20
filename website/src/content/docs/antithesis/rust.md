---
title: Using Hegel for Rust for Antithesis workloads
sidebar:
  label: Rust
---

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

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/rust/examples/sqlite).

For more information on concurrent stateful testing, see the [documentation](https://docs.rs/hegeltest/latest/hegel/stateful/index.html#concurrent-state-machines).
