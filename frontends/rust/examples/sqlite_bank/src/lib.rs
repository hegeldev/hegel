//! Bank workload for testing an ACID database (here, SQLite) with Hegel's
//! concurrent stateful testing.
//!
//! The crate runs two ways: `src/main.rs` is a `#[hegel::main]` binary whose
//! every invocation is one run of the workload, and `tests/bank.rs` is a
//! `#[hegel::test]` that runs it as a regular Hegel test. Both build the
//! machine with [`bank_machine`]. Sizing comes from the `BANK_*`
//! environment variables documented on [`WorkloadConfig`].
//!
//! The state-machine object is a [`Bank`]: a handle to the system under
//! test (a file-backed SQLite database in WAL mode with one
//! `accounts(account_id, balance, opCount)` table) plus the client-side
//! bookkeeping of the workload (attempted, committed, total).
//!
//! WAL mode is what makes the verification rules sound while transfers are
//! in flight: readers see a consistent snapshot and never block the writer.
//! Writers are still serialized, so transfers take the write lock up front
//! with `BEGIN IMMEDIATE`.

use hegel::TestCase;
use hegel::generators as gs;
use hegel::stateful::{Machine, machine};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tempfile::TempDir;

/// How long a connection waits on a locked database before failing with
/// `SQLITE_BUSY`. Such a failure counts as "attempted but not known to have
/// committed", like a timeout against a networked database.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// How the bank is initialised.
#[derive(Debug, Clone, Copy)]
pub struct BankConfig {
    /// `N`, the number of accounts.
    pub accounts: i64,
    /// `B`, the initial balance of every account.
    pub initial_balance: i64,
    /// The largest amount a transfer may move. Larger than `B` so that the
    /// insufficient-funds path is exercised regularly.
    pub max_amount: i64,
}

/// How a run is sized. Every field can be fixed from the environment;
/// anything left unset is drawn from the test case, so a run with no
/// variables set explores configurations and a run with all of them set
/// soaks one fixed shape.
///
/// | Variable | Meaning | Unset |
/// |---|---|---|
/// | `BANK_ACCOUNTS` | `N`, number of accounts (≥ 1) | drawn, 1..=16 |
/// | `BANK_INITIAL_BALANCE` | `B`, starting balance per account (≥ 0) | drawn, 0..=1000 |
/// | `BANK_MAX_AMOUNT` | largest transfer amount (≥ 0) | drawn, 0..=2B+10 |
/// | `BANK_STEPS` | rounds per test case (≥ 1) | 200 |
/// | `BANK_MIN_CONCURRENCY` | fewest worker threads (≥ 1) | 2 |
/// | `BANK_MAX_CONCURRENCY` | most worker threads (≥ min) | 8 |
#[derive(Debug, Clone, Copy)]
pub struct WorkloadConfig {
    pub bank: BankConfig,
    pub steps: i64,
    pub min_concurrency: i64,
    pub max_concurrency: i64,
}

impl WorkloadConfig {
    /// Resolve the configuration from the environment, drawing whatever the
    /// environment leaves unset. Panics on a malformed variable.
    pub fn from_env_or_draw(tc: &TestCase) -> Self {
        let accounts = env_i64("BANK_ACCOUNTS", 1)
            .unwrap_or_else(|| tc.draw(gs::integers::<i64>().min_value(1).max_value(16)));
        let initial_balance = env_i64("BANK_INITIAL_BALANCE", 0)
            .unwrap_or_else(|| tc.draw(gs::integers::<i64>().min_value(0).max_value(1_000)));
        let max_amount = env_i64("BANK_MAX_AMOUNT", 0).unwrap_or_else(|| {
            tc.draw(
                gs::integers::<i64>()
                    .min_value(0)
                    .max_value(2 * initial_balance + 10),
            )
        });
        let min_concurrency = env_i64("BANK_MIN_CONCURRENCY", 1).unwrap_or(2);
        let max_concurrency =
            env_i64("BANK_MAX_CONCURRENCY", min_concurrency).unwrap_or(8.max(min_concurrency));
        WorkloadConfig {
            bank: BankConfig {
                accounts,
                initial_balance,
                max_amount,
            },
            steps: env_i64("BANK_STEPS", 1).unwrap_or(200),
            min_concurrency,
            max_concurrency,
        }
    }
}

fn env_i64(name: &str, min: i64) -> Option<i64> {
    let raw = std::env::var(name).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    let value: i64 = raw
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("{name}={raw:?} is not an integer"));
    assert!(value >= min, "{name}={value} must be at least {min}");
    Some(value)
}

/// What the client observed for a transfer attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOutcome {
    /// `COMMIT` returned successfully.
    Committed,
    /// Rolled back because the source balance was insufficient.
    InsufficientFunds,
    /// SQLite returned an error, such as `SQLITE_BUSY`. Whether the
    /// transaction committed is unknown.
    Error,
}

pub struct Bank {
    _dir: TempDir,
    path: PathBuf,
    /// Idle connections. Rules run on worker threads with `&self`, and a
    /// `Connection` is `Send` but not `Sync`, so each rule checks one out
    /// for the duration of a transaction.
    pool: Mutex<Vec<Connection>>,
    config: BankConfig,
    attempted: AtomicU64,
    committed: AtomicU64,
    total: i64,
}

impl Bank {
    /// Create the `accounts` table with `N` rows of balance `B` and
    /// `opCount` 0.
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

    pub fn attempted(&self) -> u64 {
        self.attempted.load(Ordering::SeqCst)
    }

    pub fn committed(&self) -> u64 {
        self.committed.load(Ordering::SeqCst)
    }

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

    /// The transfer transaction. Returns what the client observed.
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

    /// No money is created or destroyed.
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

    /// No committed transfer is lost. `committed` is read before the
    /// snapshot, so every transfer it counts must be visible in it;
    /// `attempted` is read after, so every transfer visible in the snapshot
    /// is counted by it.
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

    /// No account balance is below zero.
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

/// All rules share the `"bank"` group, so any of them may overlap with any
/// other: transfers race transfers, and the verifiers read while transfers
/// are in flight. The same verifications are also invariants, which Hegel
/// checks on the initial and final state and samples between rounds.
#[hegel::concurrent_state_machine]
impl Bank {
    #[rule(group = "bank", weight = 6)]
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

    #[rule(group = "bank")]
    fn verify_total_balance_rule(&self, _: TestCase) {
        self.verify_total_balance();
    }

    #[rule(group = "bank")]
    fn verify_ops_not_lost_rule(&self, _: TestCase) {
        self.verify_ops_not_lost();
    }

    #[rule(group = "bank")]
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

/// Build a fresh bank for this test case and configure the machine from
/// `config`. The caller runs it with `.run_concurrent(tc)`.
pub fn bank_machine(tc: &TestCase, config: WorkloadConfig) -> Machine<Bank> {
    let bank = Bank::init(config.bank).expect("initialise the bank database");
    tc.note(&format!("workload: {config:?}"));
    machine(bank)
        .steps(config.steps)
        .min_concurrency(config.min_concurrency)
        .max_concurrency(config.max_concurrency)
}
