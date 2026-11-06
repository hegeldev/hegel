package dev.hegel;

/**
 * Thrown when the engine could not find any input satisfying the test's assumptions and filters.
 * With the {@link HealthCheck#FILTER_TOO_MUCH} health check enabled, a run that rejects nearly
 * everything fails that check ({@link HealthCheckFailure}) first.
 */
public final class UnsatisfiableException extends HegelRunException {
    UnsatisfiableException(String message) {
        super(message);
    }
}
