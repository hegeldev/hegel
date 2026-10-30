package dev.hegel;

import dev.hegel.lowlevel.Abi;

/**
 * Why the engine ended a run in an error rather than a verdict on the property ({@link
 * RunStatus#ERROR}). {@link RunReport#throwIfFailed()} raises a {@link HegelRunException} of the
 * matching type.
 */
public enum RunErrorKind {
    /** A {@link HealthCheck} failed: {@link HealthCheckFailure}. */
    HEALTH_CHECK(Abi.RUN_ERROR_HEALTH_CHECK),
    /**
     * Under {@link NondeterminismStrictness#ERROR}, the test produced different outcomes when
     * replayed on identical data: {@link FlakyTestException}.
     */
    FLAKY(Abi.RUN_ERROR_FLAKY),
    /**
     * Under {@link NondeterminismStrictness#ERROR}, data generation diverged when the same choices
     * were replayed: {@link FlakyTestException}.
     */
    NONDETERMINISTIC(Abi.RUN_ERROR_NONDETERMINISTIC),
    /** No valid test case was ever generated: {@link UnsatisfiableException}. */
    UNSATISFIABLE(Abi.RUN_ERROR_UNSATISFIABLE),
    /** The engine was driven or configured incorrectly; the message says what to fix. */
    USAGE(Abi.RUN_ERROR_USAGE),
    /** A violated internal invariant of the engine: a bug in Hegel. */
    INTERNAL(Abi.RUN_ERROR_INTERNAL);

    private final int code;

    RunErrorKind(int code) {
        this.code = code;
    }

    /** The kind the engine reported, or {@code null} for none or one this version does not know. */
    static RunErrorKind fromCode(int code) {
        for (RunErrorKind kind : values()) {
            if (kind.code == code) {
                return kind;
            }
        }
        return null;
    }
}
