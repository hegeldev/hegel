package dev.hegel;

/**
 * Thrown when a run told to replay stored failures ({@link Settings#reproduceFailure(String...)},
 * {@link HegelTest#reproduceFailure()}) saw none of them fail: the failure may have been fixed,
 * or, for a nondeterministic one, not recurred within the replay budget.
 */
public final class DidNotReproduceException extends HegelException {
    DidNotReproduceException(String message) {
        super(message);
    }
}
