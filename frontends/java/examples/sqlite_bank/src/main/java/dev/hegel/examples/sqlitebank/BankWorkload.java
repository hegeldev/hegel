package dev.hegel.examples.sqlitebank;

import static dev.hegel.Generators.longs;

import dev.hegel.Hegel;
import dev.hegel.Stateful;
import dev.hegel.TestCase;
import java.sql.SQLException;
import java.util.OptionalLong;

/**
 * Bank workload that tests an ACID database with Hegel's concurrent stateful testing. Each run of
 * the program runs the workload once.
 */
public final class BankWorkload {
    private BankWorkload() {}

    record Workload(Bank.Config bank, int steps, int minConcurrency, int maxConcurrency) {
        /**
         * Reads the workload from the environment, and draws a value for each unset variable:
         *
         * <ul>
         *   <li>{@code BANK_ACCOUNTS}: {@code N}, at least 1. Drawn from 1..16.
         *   <li>{@code BANK_INITIAL_BALANCE}: {@code B}, at least 0. Drawn from 0..1000.
         *   <li>{@code BANK_MAX_AMOUNT}: at least 0. Drawn from 0..2B+10.
         *   <li>{@code BANK_STEPS}: at least 1. Defaults to 200.
         *   <li>{@code BANK_MIN_CONCURRENCY}: at least 1. Defaults to 2.
         *   <li>{@code BANK_MAX_CONCURRENCY}: at least the minimum. Defaults to 8.
         *   <li>{@code BANK_INJECT_BUG}: set to skip the balance check in transfers.
         * </ul>
         *
         * <p>The upper limits keep {@code N * B} and {@code 2B + 10} within a {@code long}.
         */
        static Workload fromEnvironmentOrDraw(TestCase tc) {
            long accounts = env("BANK_ACCOUNTS", 1, Integer.MAX_VALUE)
                    .orElseGet(() -> tc.draw(longs().min(1).max(16), "accounts"));
            long maxInitialBalance = Math.min(Long.MAX_VALUE / accounts, (Long.MAX_VALUE - 10) / 2);
            long initialBalance = env("BANK_INITIAL_BALANCE", 0, maxInitialBalance)
                    .orElseGet(() -> tc.draw(longs().min(0).max(1000), "initial_balance"));
            long maxAmount = env("BANK_MAX_AMOUNT", 0, Long.MAX_VALUE)
                    .orElseGet(() -> tc.draw(longs().min(0).max(2 * initialBalance + 10), "max_amount"));
            int steps = envInt("BANK_STEPS", 1, 200);
            int minConcurrency = envInt("BANK_MIN_CONCURRENCY", 1, 2);
            int maxConcurrency = envInt("BANK_MAX_CONCURRENCY", minConcurrency, Math.max(8, minConcurrency));
            boolean injectBug = isSet("BANK_INJECT_BUG");
            return new Workload(
                    new Bank.Config(accounts, initialBalance, maxAmount, injectBug),
                    steps,
                    minConcurrency,
                    maxConcurrency);
        }
    }

    private static boolean isSet(String name) {
        String raw = System.getenv(name);
        return raw != null && !raw.isBlank();
    }

    private static OptionalLong env(String name, long minimum, long maximum) {
        if (!isSet(name)) {
            return OptionalLong.empty();
        }
        String raw = System.getenv(name);
        long value;
        try {
            value = Long.parseLong(raw.strip());
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException(name + "=" + raw + " is not an integer", e);
        }
        if (value < minimum || value > maximum) {
            throw new IllegalArgumentException(name + "=" + value + " must be between " + minimum + " and " + maximum);
        }
        return OptionalLong.of(value);
    }

    private static int envInt(String name, int minimum, int defaultValue) {
        return Math.toIntExact(env(name, minimum, Integer.MAX_VALUE).orElse(defaultValue));
    }

    static void run(TestCase tc, Workload workload) {
        tc.note("workload: " + workload);
        try (Bank bank = new Bank(workload.bank())) {
            Stateful.run(
                    bank,
                    tc,
                    Stateful.options()
                            .stepCount(workload.steps())
                            .minConcurrency(workload.minConcurrency())
                            .maxConcurrency(workload.maxConcurrency()));
        } catch (SQLException e) {
            throw new IllegalStateException("bank database error", e);
        }
    }

    public static void main(String[] args) {
        Hegel.main(args, tc -> run(tc, Workload.fromEnvironmentOrDraw(tc)));
    }
}
