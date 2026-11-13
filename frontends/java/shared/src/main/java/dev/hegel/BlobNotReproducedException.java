package dev.hegel;

/**
 * Thrown when replaying a {@link Settings#reproduceFailure} blob does not reproduce its failure:
 * either every replay passed (the blob is stale) or the replay ended in an engine error.
 */
final class BlobNotReproducedException extends HegelException {
    private static final long serialVersionUID = 1L;

    private final String replayError;

    BlobNotReproducedException(String replayError) {
        super(
                replayError == null
                        ? Runner.STALE_BLOB
                        : "reproduceFailure: the blob replay ended in an error: " + replayError);
        this.replayError = replayError;
    }

    boolean stale() {
        return replayError == null;
    }

    String replayError() {
        return replayError;
    }
}
