---
title: Using Hegel for Java for Antithesis workloads
sidebar:
  label: Java
---

This is a simple test workload we call the bank test, which checks atomicity and isolation in a SQLite database and a functional correctness property
on a model of a bank. Note that some code is omitted for brevity. See [Further reading](#further-reading) for the full code.

## Bank state

```java
private static final int BUSY_TIMEOUT_MILLIS = 5000;

record Config(long accounts, long initialBalance, long maxAmount, boolean injectBug) {}

enum Outcome {
    COMMITTED,
    INSUFFICIENT_FUNDS,
    UNKNOWN,
}

private final Config config;
private final long total;
private final AtomicLong attempted = new AtomicLong();
private final AtomicLong committed = new AtomicLong();
private final Path dir;
private final SQLiteDataSource dataSource;

private final Deque<Connection> idle = new ConcurrentLinkedDeque<>();
```

`BUSY_TIMEOUT_MILLIS` is how long a connection waits on a locked database before it fails with `SQLITE_BUSY`.

The database has one `accounts` table with an `account_id`, a `balance` and an `opCount` for each account.

The database runs in WAL mode, which allows non-blocking reads while transfers are executing, and each read sees a consistent snapshot. Since writers are serialized, transfers must take the write lock first with `BEGIN IMMEDIATE`.

`Config` sets the number of accounts, the starting balance of each account, and the largest transfer amount.

Transfers can have one of the following outcomes (`Outcome`):

- `COMMITTED`: `COMMIT` returned successfully.
- `INSUFFICIENT_FUNDS`: the transaction was rolled back because the source balance was too low.
- `UNKNOWN`: SQLite returned an error, such as `SQLITE_BUSY`. The client does not know if the transaction committed.

The test state `Bank` consists of the database directory and data source, a pool of idle connections, counts of attempted and committed transfers, and the total balance. The counters are atomic, and a concurrent deque holds the idle connections.

## Database helpers

```java
private <T> T withConnection(SqlWork<T> work) throws SQLException {
    Connection db = idle.poll();
    if (db == null) {
        db = dataSource.getConnection();
    }
    T result;
    try {
        result = work.apply(db);
    } catch (SQLException | RuntimeException e) {
        try {
            db.close();
        } catch (SQLException closeError) {
            e.addSuppressed(closeError);
        }
        throw e;
    }
    idle.push(db);
    return result;
}
```

`withConnection` takes an idle connection from the pool or opens a new one. It returns the connection to the pool when the callback returns normally and closes it if the callback throws an exception. `execute` runs a SQL statement, `update` binds the amount and account ID before executing an update, and `queryLong` returns the first column of the first row.

## Setup

```java
Bank(Config config) throws SQLException {
    this.config = config;
    this.total = config.accounts() * config.initialBalance();
    try {
        dir = Files.createTempDirectory("sqlite_bank");
    } catch (IOException e) {
        throw new UncheckedIOException(e);
    }

    SQLiteConfig sqlite = new SQLiteConfig();
    sqlite.setJournalMode(SQLiteConfig.JournalMode.WAL);
    sqlite.setSynchronous(SQLiteConfig.SynchronousMode.FULL);
    sqlite.setBusyTimeout(BUSY_TIMEOUT_MILLIS);
    dataSource = new SQLiteDataSource(sqlite);
    dataSource.setUrl("jdbc:sqlite:" + dir.resolve("bank.sqlite3"));

    try {
        createAccounts();
    } catch (SQLException | RuntimeException e) {
        try {
            close();
        } catch (SQLException | RuntimeException closeError) {
            e.addSuppressed(closeError);
        }
        throw e;
    }
}

private void createAccounts() throws SQLException {
    withConnection(db -> {
        execute(
                db,
                "CREATE TABLE accounts (account_id INTEGER PRIMARY KEY, "
                        + "balance INTEGER NOT NULL, opCount INTEGER NOT NULL)");
        execute(db, "BEGIN");
        try (PreparedStatement insert =
                db.prepareStatement("INSERT INTO accounts (account_id, balance, opCount) VALUES (?, ?, 0)")) {
            for (long account = 0; account < config.accounts(); account++) {
                insert.setLong(1, account);
                insert.setLong(2, config.initialBalance());
                insert.addBatch();
            }
            insert.executeBatch();
        }
        execute(db, "COMMIT");
        return null;
    });
}
```

`Bank` creates the database in a temporary directory, switches it to WAL mode, and creates the `accounts` table with one row per account. Each account starts with the initial balance and an `opCount` of 0. `Bank` also records the expected total `config.accounts() * config.initialBalance()`. `close` closes the connections in the connection pool and deletes the database files.

## Transfers

```java
Outcome transfer(long source, long dest, long amount) {
    attempted.incrementAndGet();
    Outcome outcome;
    try {
        outcome = withConnection(db -> {
            execute(db, "BEGIN IMMEDIATE");
            long balance = queryLong(db, "SELECT balance FROM accounts WHERE account_id = ?", source);
            if (!config.injectBug() && balance < amount) {
                execute(db, "ROLLBACK");
                return Outcome.INSUFFICIENT_FUNDS;
            }
            update(
                    db,
                    "UPDATE accounts SET balance = balance - ?, opCount = opCount + 1 WHERE account_id = ?",
                    amount,
                    source);
            update(
                    db,
                    "UPDATE accounts SET balance = balance + ?, opCount = opCount + 1 WHERE account_id = ?",
                    amount,
                    dest);
            execute(db, "COMMIT");
            return Outcome.COMMITTED;
        });
    } catch (SQLException e) {
        return Outcome.UNKNOWN;
    }
    if (outcome == Outcome.COMMITTED) {
        committed.incrementAndGet();
    }
    return outcome;
}
```

`transfer` increments `attempted`, then runs one transaction. If the source balance is less than the transfer amount, the transaction is rolled back. Otherwise, `transfer` subtracts the amount from the source, adds the amount to the destination, and increments `opCount` on both accounts. If a statement raises an exception, closing the connection rolls back the open transaction.

If the transaction commits, `transfer` increments `committed`. Any SQLite error becomes `Outcome.UNKNOWN`.

## Properties

```java
@Rule(groups = "bank")
@Invariant
void verifyOpsNotLost(TestCase tc) throws SQLException {
    long committedBefore = committed.get();
    long sum = withConnection(db -> queryLong(db, "SELECT SUM(opCount) FROM accounts"));
    long attemptedAfter = attempted.get();
    check(sum % 2 == 0, "SUM(opCount) = " + sum + " is odd: a transfer was half-applied");
    check(
            sum >= 2 * committedBefore,
            "committed transfers were lost: SUM(opCount) = " + sum + " < 2 * " + committedBefore + " committed");
    check(
            sum <= 2 * attemptedAfter,
            "phantom transfers: SUM(opCount) = " + sum + " > 2 * " + attemptedAfter + " attempted");
}

@Rule(groups = "bank")
@Invariant
void verifyNoNegative(TestCase tc) throws SQLException {
    long negative = withConnection(db -> queryLong(db, "SELECT COUNT(*) FROM accounts WHERE balance < 0"));
    check(negative == 0, negative + " account(s) have a negative balance");
}
```

`verifyTotalBalance` checks that `SUM(balance)` equals the starting total (i.e. no money was created or destroyed).

`verifyOpsNotLost` checks `SUM(opCount)`. Each transfer increments two counts, so the sum must be even. The sum must be at least `2 * committedBefore`, or a committed transfer was lost. It can be higher because some transfers can commit without the client knowing, but it cannot exceed `2 * attemptedAfter`.

`verifyNoNegative` checks that no account has a balance below zero. If two transfers
both read the old balance before either withdraws, the account can be overdrawn.

## Rules and invariants

```java
@Rule(groups = "bank", weight = 6)
void transferRule(TestCase tc) {
    long source = tc.draw(longs().min(0).max(config.accounts() - 1), "source");
    long dest = tc.draw(longs().min(0).max(config.accounts() - 1), "dest");
    long amount = tc.draw(longs().min(0).max(config.maxAmount()), "amount");
    Outcome outcome = transfer(source, dest, amount);
    tc.note("transfer " + amount + " from " + source + " to " + dest + ": " + outcome);
}

@Rule(groups = "bank")
@Invariant
void verifyTotalBalance(TestCase tc) throws SQLException {
    long sum = withConnection(db -> queryLong(db, "SELECT SUM(balance) FROM accounts"));
    check(sum == total, "money was created or destroyed: SUM(balance) = " + sum + ", expected " + total);
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
accounts instead of 16, we might not want check `verifyNoNegative` after every single round
because it requires checking all accounts. Using `@Invariant(alwaysRun = true)` forces the invariant
to always be checked between rounds.

Notice that we mark the verification functions as both `@Invariant` and `@Rule`. Invariants check
the state of the system at some settled state when no actions are running. When we define the
verification function as a rule, they can run while other actions are mutating the state.

For example, `verifyTotalBalance` as an invariant checks that after all transfers are done,
the total amount of money has not changed. `verifyTotalBalance` as a rule checks that while
transfers are executing, we never see a state in which the total amount of money has changed.

## Running the workload

```java
static void run(TestCase tc, Workload workload) {
    tc.note("workload: " + workload);
    try (Bank bank = new Bank(workload.bank())) {
        Stateful.run(
                bank,
                tc,
                Stateful.options()
                        .stepCount(Integer.MAX_VALUE)
                        .minConcurrency(workload.minConcurrency())
                        .maxConcurrency(workload.maxConcurrency()));
    } catch (SQLException e) {
        throw new IllegalStateException("bank database error", e);
    }
}

public static void main(String[] args) {
    Hegel.main(args, tc -> run(tc, Workload.fromEnvironmentOrDraw(tc)));
}
```

`Hegel.main` turns the workload into a standalone program, which allows us to pass test settings as command line options,
such as `java dev.hegel.examples.sqlitebank.BankWorkload --seed 1 --verbosity verbose`.

Since the account balances are stored in the database, we only pass the bank handle,
which holds the database location, the connection pool, the transfer counters, and the configuration. `stepCount` sets how many rounds execute within a test. Inside of Antithesis, this number should be very high to allow
the fuzzer ample time to explore a branch. Finally, `minConcurrency` and `maxConcurrency` set the bounds for the number of concurrent workers. Hegel spawns some number of workers within those bounds.

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

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/java/examples/sqlite_bank).

For more information on concurrent stateful testing, see the [documentation](https://javadoc.io/doc/dev.hegel/hegel/latest/dev/hegel/Stateful.html).
