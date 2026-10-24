package dev.hegel;

import dev.hegel.lowlevel.Abi;

/**
 * Health checks the engine runs to catch tests that are misbehaving rather than buggy. Suppress one
 * via {@link Settings#suppressHealthCheck} when its behaviour is intentional.
 */
public enum HealthCheck {
    /** Too many draws rejected via {@code assume}/{@code filter}. */
    FILTER_TOO_MUCH(Abi.HC_FILTER_TOO_MUCH),
    /** Individual test cases take so long the run is impractical. */
    TOO_SLOW(Abi.HC_TOO_SLOW),
    /** Generated values are too large to retain for shrinking. */
    TEST_CASES_TOO_LARGE(Abi.HC_TEST_CASES_TOO_LARGE),
    /** The first generated test case is already disproportionately large (a warning). */
    LARGE_INITIAL_TEST_CASE(Abi.HC_LARGE_INITIAL_TEST_CASE),
    /**
     * No check: suppressing only this runs every health check, even where the settings profile
     * suppresses some. It is how {@link HegelTest#suppressHealthCheck()} says "suppress nothing",
     * which an empty array, the annotation's default, cannot.
     */
    NONE(0);

    final int bit;

    HealthCheck(int bit) {
        this.bit = bit;
    }

    /** The check whose bit is {@code bit}, or {@code null} for one this version does not know. */
    static HealthCheck fromBit(int bit) {
        for (HealthCheck check : values()) {
            if (check != NONE && check.bit == bit) {
                return check;
            }
        }
        return null;
    }
}
