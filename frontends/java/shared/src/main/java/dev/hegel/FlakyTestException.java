package dev.hegel;

/**
 * Thrown when the engine aborts a run because the test behaved nondeterministically, under {@link
 * NondeterminismStrictness#ERROR}: replaying the same generated choices produced a different
 * outcome, or different draws. The other strictness levels instead confirm such a failure by
 * repeated replay and report it with a {@linkplain Failure#caveat() caveat}.
 */
public final class FlakyTestException extends HegelRunException {
    FlakyTestException(String message) {
        super(message);
    }
}
