package dev.hegel;

import java.util.Collections;
import java.util.List;
import java.util.Optional;

/**
 * The outcome of one property-test run: verdict, case statistics, and the counterexamples found.
 *
 * <p>Returned by {@link Hegel#run} (which never throws for a property outcome) and by {@link
 * Hegel#test} (which first calls {@link #throwIfFailed()}).
 */
public final class RunReport {
    private final RunStatus status;
    private final RunStatistics statistics;
    private final String error;
    private final RunErrorKind errorKind;
    private final HealthCheck healthCheck;
    private final List<Failure> failures;

    RunReport(RunStatus status, RunStatistics statistics, List<Failure> failures) {
        this(status, statistics, null, null, null, failures);
    }

    RunReport(
            RunStatus status,
            RunStatistics statistics,
            String error,
            RunErrorKind errorKind,
            HealthCheck healthCheck,
            List<Failure> failures) {
        this.status = status;
        this.statistics = statistics;
        this.error = error;
        this.errorKind = errorKind;
        this.healthCheck = healthCheck;
        this.failures = Collections.unmodifiableList(failures);
    }

    /**
     * The run's verdict.
     *
     * @return the status
     */
    public RunStatus status() {
        return status;
    }

    /**
     * Whether the property held for every generated input.
     *
     * @return {@code true} iff {@link #status()} is {@link RunStatus#PASSED}
     */
    public boolean passed() {
        return status == RunStatus.PASSED;
    }

    /**
     * How many cases the run executed, by outcome.
     *
     * @return the statistics
     */
    public RunStatistics statistics() {
        return statistics;
    }

    /**
     * The engine's message for a run that produced no verdict (a failed health check, an engine
     * error).
     *
     * @return the message, or empty unless {@link #status()} is {@link RunStatus#ERROR}
     */
    public Optional<String> error() {
        return Optional.ofNullable(error);
    }

    /**
     * Why the run produced no verdict, as the engine classified it.
     *
     * @return the kind, or empty unless {@link #status()} is {@link RunStatus#ERROR} (or the engine
     *     reported a kind this version of Hegel does not know)
     */
    public Optional<RunErrorKind> errorKind() {
        return Optional.ofNullable(errorKind);
    }

    /**
     * Whether the run was aborted by a health check (for example, the generators rejected almost
     * every input). Suppress it with {@link Settings#suppressHealthCheck(HealthCheck...)} if the
     * behaviour is intentional.
     *
     * @return {@code true} if {@link #errorKind()} is {@link RunErrorKind#HEALTH_CHECK}
     */
    public boolean healthCheckFailed() {
        return errorKind == RunErrorKind.HEALTH_CHECK;
    }

    /**
     * The health check that aborted the run.
     *
     * @return the check, or empty unless {@link #healthCheckFailed()} (or the engine named a check
     *     this version of Hegel does not know)
     */
    public Optional<HealthCheck> failedHealthCheck() {
        return Optional.ofNullable(healthCheck);
    }

    /**
     * The distinct counterexamples the run found, in the order the engine reported them. Empty
     * unless {@link #status()} is {@link RunStatus#FAILED}; more than one only with {@link
     * Settings#reportMultipleFailures(boolean)}.
     *
     * @return an unmodifiable list of failures
     */
    public List<Failure> failures() {
        return failures;
    }

    /**
     * Turn a non-passing report into the exception {@link Hegel#test} throws: nothing for a passed
     * run; for an errored run, a {@link HegelRunException} carrying the engine's message, of the
     * subtype its {@link #errorKind()} names ({@link HealthCheckFailure}, {@link FlakyTestException},
     * {@link UnsatisfiableException}); for a failed run, the single failure's own exception rethrown
     * as-is, or an {@link AssertionError} aggregating several distinct failures (carrying the
     * originals as suppressed exceptions).
     */
    public void throwIfFailed() {
        if (status == RunStatus.PASSED) {
            return;
        }
        if (status == RunStatus.ERROR) {
            throw runError();
        }
        if (failures.size() == 1) {
            throw unchecked(failures.get(0).exception());
        }
        StringBuilder sb = new StringBuilder();
        sb.append("Hegel found ").append(failures.size()).append(" distinct failing examples:");
        for (Failure failure : failures) {
            sb.append("\n\n").append(describe(failure.exception()));
        }
        AssertionError aggregate = new AssertionError(sb.toString());
        for (Failure failure : failures) {
            aggregate.addSuppressed(failure.exception());
        }
        throw aggregate;
    }

    /** The exception for an errored run, typed by its {@link #errorKind()}. */
    private HegelRunException runError() {
        if (errorKind == null) {
            return new HegelRunException(error);
        }
        switch (errorKind) {
            case HEALTH_CHECK:
                return new HealthCheckFailure(error, healthCheck);
            case FLAKY:
            case NONDETERMINISTIC:
                return new FlakyTestException(error);
            case UNSATISFIABLE:
                return new UnsatisfiableException(error);
            default:
                return new HegelRunException(error);
        }
    }

    /**
     * Rethrow {@code t} with its original type: an unchecked exception is returned for the caller to
     * throw, anything else (an {@link Error}, or a checked exception a frontend such as Kotlin or
     * Clojure can raise from the body) is thrown here as-is through generic erasure. The body's
     * exception is the user's own; wrapping it would hide its type and stack trace.
     */
    @SuppressWarnings("unchecked")
    private static <T extends Throwable> RuntimeException unchecked(Throwable t) throws T {
        if (t instanceof RuntimeException) {
            return (RuntimeException) t;
        }
        throw (T) t;
    }

    private static String describe(Throwable e) {
        String msg = e.getMessage();
        return msg == null ? e.getClass().getName() : e.getClass().getName() + ": " + msg;
    }
}
