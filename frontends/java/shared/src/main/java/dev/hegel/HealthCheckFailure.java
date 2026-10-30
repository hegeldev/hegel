package dev.hegel;

import java.util.Optional;

/**
 * Thrown when the engine aborts a run because a {@link HealthCheck} fired: the test itself is
 * misbehaving — rejecting almost every input, or generating data that is too large or too slow —
 * rather than the property being false. If the behaviour is intentional, disable the offending
 * check with {@link Settings#suppressHealthCheck}.
 */
public final class HealthCheckFailure extends HegelRunException {
    private final HealthCheck healthCheck;

    HealthCheckFailure(String message, HealthCheck healthCheck) {
        super(message);
        this.healthCheck = healthCheck;
    }

    /**
     * The health check that failed.
     *
     * @return the check, or empty if the engine named one this version of Hegel does not know
     */
    public Optional<HealthCheck> healthCheck() {
        return Optional.ofNullable(healthCheck);
    }
}
