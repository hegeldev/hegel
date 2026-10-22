package dev.hegel.generators;

import dev.hegel.lowlevel.Abi;
import java.time.temporal.ChronoUnit;

/** Maps the {@link ChronoUnit}s the engine can count a time of day in to its precision constants. */
final class Precisions {
    private Precisions() {}

    static int engine(ChronoUnit precision, String what) {
        switch (precision) {
            case NANOS:
                return Abi.PRECISION_NANOSECONDS;
            case MICROS:
                return Abi.PRECISION_MICROSECONDS;
            case MILLIS:
                return Abi.PRECISION_MILLISECONDS;
            case SECONDS:
                return Abi.PRECISION_SECONDS;
            case MINUTES:
                return Abi.PRECISION_MINUTES;
            case HOURS:
                return Abi.PRECISION_HOURS;
            default:
                throw new IllegalArgumentException(what
                        + ": precision must be NANOS, MICROS, MILLIS, SECONDS, MINUTES or HOURS, not " + precision);
        }
    }
}
