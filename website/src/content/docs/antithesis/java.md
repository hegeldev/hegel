---
title: Using Hegel for Java for Antithesis workloads
sidebar:
  label: Java
---

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

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/java/examples/sqlite_bank).

For more information on concurrent stateful testing, see the [documentation](https://javadoc.io/doc/dev.hegel/hegel/latest/dev/hegel/Stateful.html).
