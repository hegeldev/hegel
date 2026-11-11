package dev.hegel.examples.sqlitebank;

import static dev.hegel.Generators.longs;

import dev.hegel.Invariant;
import dev.hegel.Rule;
import dev.hegel.TestCase;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import java.util.Comparator;
import java.util.Deque;
import java.util.List;
import java.util.concurrent.ConcurrentLinkedDeque;
import java.util.concurrent.atomic.AtomicLong;
import java.util.stream.Stream;
import org.sqlite.SQLiteConfig;
import org.sqlite.SQLiteDataSource;

/**
 * A file-backed SQLite database in WAL mode with one {@code accounts(account_id, balance, opCount)}
 * table, the workload's client-side counts (attempted, committed, total), and a pool of idle
 * connections that rules use for each transaction.
 *
 * <p>In WAL mode, the verification rules can read while transfers are running, without blocking
 * them or failing with {@code SQLITE_BUSY}, and each read sees a consistent snapshot. Writes are
 * still serialized, so transfers take the write lock at the start with {@code BEGIN IMMEDIATE}.
 *
 * <p>Each verify check is both a rule and an invariant. As rules, they run while transfers are
 * running, and check what a reader sees during a write. As invariants, they run on the final state
 * and sometimes between rounds, so a lost commit is caught by the end of the run.
 */
final class Bank implements AutoCloseable {
    /**
     * How many milliseconds a connection waits on a locked database before failing with {@code
     * SQLITE_BUSY}. A transfer that fails this way counts as attempted, but not known to have
     * committed.
     */
    private static final int BUSY_TIMEOUT_MILLIS = 5000;

    /**
     * @param accounts {@code N}, the number of accounts
     * @param initialBalance {@code B}, the initial balance of every account
     * @param maxAmount the largest amount a transfer can move. It can be larger than {@code B}, so
     *     that some transfers have insufficient funds.
     * @param injectBug skip the balance check in transfers, so the run fails the "no negative
     *     balance" check
     */
    record Config(long accounts, long initialBalance, long maxAmount, boolean injectBug) {}

    /** What the client observed for a transfer attempt. */
    enum Outcome {
        /** {@code COMMIT} returned successfully. */
        COMMITTED,
        /** Rolled back because the source balance was insufficient. */
        INSUFFICIENT_FUNDS,
        /** SQLite returned an error, such as {@code SQLITE_BUSY}. Whether it committed is unknown. */
        UNKNOWN,
    }

    @FunctionalInterface
    private interface SqlWork<T> {
        T apply(Connection db) throws SQLException;
    }

    private final Config config;
    private final long total;
    private final AtomicLong attempted = new AtomicLong();
    private final AtomicLong committed = new AtomicLong();
    private final Path dir;
    private final SQLiteDataSource dataSource;

    /** Rules run concurrently on worker threads, so each rule takes a connection for its transaction. */
    private final Deque<Connection> idle = new ConcurrentLinkedDeque<>();

    /** Creates the {@code accounts} table with {@code N} rows of balance {@code B} and opCount 0. */
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

    /** The transfer transaction. Returns what the client observed. */
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

    /** No money is created or destroyed. */
    @Rule(groups = "bank")
    @Invariant
    void verifyTotalBalance(TestCase tc) throws SQLException {
        long sum = withConnection(db -> queryLong(db, "SELECT SUM(balance) FROM accounts"));
        check(sum == total, "money was created or destroyed: SUM(balance) = " + sum + ", expected " + total);
    }

    /**
     * No committed transfer is lost. {@code committed} is read before the snapshot, so every
     * transfer it counts must appear in the snapshot. {@code attempted} is read after the snapshot,
     * so it counts every transfer that appears in it.
     */
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

    /** No account balance is below zero. */
    @Rule(groups = "bank")
    @Invariant
    void verifyNoNegative(TestCase tc) throws SQLException {
        long negative = withConnection(db -> queryLong(db, "SELECT COUNT(*) FROM accounts WHERE balance < 0"));
        check(negative == 0, negative + " account(s) have a negative balance");
    }

    @Rule(groups = "bank", weight = 6)
    void transferRule(TestCase tc) {
        long source = tc.draw(longs().min(0).max(config.accounts() - 1), "source");
        long dest = tc.draw(longs().min(0).max(config.accounts() - 1), "dest");
        long amount = tc.draw(longs().min(0).max(config.maxAmount()), "amount");
        Outcome outcome = transfer(source, dest, amount);
        tc.note("transfer " + amount + " from " + source + " to " + dest + ": " + outcome);
    }

    /**
     * Runs {@code work} on an idle connection, or a new one if none is idle. Every connection stays
     * in auto-commit mode and transactions are begun with SQL, because the driver's {@link
     * Connection#commit} begins the next transaction straight away, which an idle connection would
     * then hold open. A connection that fails is closed rather than reused, since it may be left
     * inside a transaction; closing it rolls that back.
     */
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

    private static void execute(Connection db, String sql) throws SQLException {
        try (Statement statement = db.createStatement()) {
            statement.execute(sql);
        }
    }

    private static void update(Connection db, String sql, long amount, long account) throws SQLException {
        try (PreparedStatement statement = db.prepareStatement(sql)) {
            statement.setLong(1, amount);
            statement.setLong(2, account);
            statement.executeUpdate();
        }
    }

    private static long queryLong(Connection db, String sql, long... parameters) throws SQLException {
        try (PreparedStatement statement = db.prepareStatement(sql)) {
            for (int i = 0; i < parameters.length; i++) {
                statement.setLong(i + 1, parameters[i]);
            }
            try (ResultSet row = statement.executeQuery()) {
                row.next();
                return row.getLong(1);
            }
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    @Override
    public void close() throws SQLException {
        SQLException failure = null;
        for (Connection db = idle.poll(); db != null; db = idle.poll()) {
            try {
                db.close();
            } catch (SQLException e) {
                if (failure == null) {
                    failure = e;
                } else {
                    failure.addSuppressed(e);
                }
            }
        }
        deleteDirectory();
        if (failure != null) {
            throw failure;
        }
    }

    private void deleteDirectory() {
        try (Stream<Path> walk = Files.walk(dir)) {
            List<Path> files = walk.sorted(Comparator.reverseOrder()).toList();
            for (Path file : files) {
                Files.delete(file);
            }
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }
}
