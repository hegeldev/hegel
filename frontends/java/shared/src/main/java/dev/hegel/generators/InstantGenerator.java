package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.TestCase;
import java.time.Instant;
import java.time.temporal.ChronoUnit;

/**
 * Generates {@link Instant} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is the engine's, years {@code -999999} to {@code 999999} ({@link Instant}
 * itself reaches a thousand times further); narrow it with the fluent {@link #min(Instant)} /
 * {@link #max(Instant)} methods, and make every drawn instant a whole number of some unit since
 * the epoch with {@link #precision(ChronoUnit)}. Values shrink toward 2000-01-01T00:00:00Z, or the
 * nearest bound when that is out of range. One draw in twenty on average targets an interesting
 * instant: the Unix epoch, the 32-bit and 64-bit boundaries, a leap-second boundary or a
 * daylight-saving transition of some time zone.
 */
public final class InstantGenerator implements Generator<Instant> {
    /** The engine's instant range: the civil range of years {@code [-999999, 999999]}. */
    static final Instant MIN_ENGINE = Instant.ofEpochSecond(-31_619_087_596_800L);

    static final Instant MAX_ENGINE = Instant.ofEpochSecond(31_494_784_780_799L, 999_999_999);

    private final Instant min;
    private final Instant max;
    private final ChronoUnit precision;
    private final int precisionCode;

    public InstantGenerator() {
        this(MIN_ENGINE, MAX_ENGINE, ChronoUnit.NANOS);
    }

    private InstantGenerator(Instant min, Instant max, ChronoUnit precision) {
        validate(min, "instants");
        validate(max, "instants");
        if (min.isAfter(max)) {
            throw new IllegalArgumentException("instants: min (" + min + ") > max (" + max + ")");
        }
        this.min = min;
        this.max = max;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "instants");
    }

    static void validate(Instant instant, String what) {
        if (instant.isBefore(MIN_ENGINE) || instant.isAfter(MAX_ENGINE)) {
            throw new IllegalArgumentException(what + ": " + instant + " is outside the years [-999999, 999999]");
        }
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public InstantGenerator min(Instant min) {
        return new InstantGenerator(min, max, precision);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public InstantGenerator max(Instant max) {
        return new InstantGenerator(min, max, precision);
    }

    /**
     * Draw only instants that are a whole number of {@code precision} units since the epoch:
     * {@link ChronoUnit#NANOS} (the default), {@link ChronoUnit#MICROS}, {@link ChronoUnit#MILLIS},
     * {@link ChronoUnit#SECONDS}, {@link ChronoUnit#MINUTES} or {@link ChronoUnit#HOURS}. Bounds
     * holding no such instant are rejected with an {@link IllegalArgumentException} at the first
     * draw.
     *
     * @param precision the unit every drawn instant is a whole number of
     * @return a copy with the precision set
     */
    public InstantGenerator precision(ChronoUnit precision) {
        return new InstantGenerator(min, max, precision);
    }

    /** @hidden */
    @Override
    public Instant doDraw(TestCase tc) {
        return tc.generateTimestamp(min, max, precisionCode);
    }
}
