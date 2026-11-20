package dev.hegel.generators.temporal;

import dev.hegel.EngineAccess;
import dev.hegel.Generator;
import dev.hegel.TestCase;
import java.time.Instant;
import java.time.ZoneOffset;
import java.time.temporal.ChronoUnit;
import java.util.Objects;

/**
 * Generates {@link Instant} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is {@code 0001-01-01T00:00:00Z} to {@code 9999-12-31T23:59:59.999999999Z},
 * the years the other date generators default to; set either end with the fluent {@link
 * #min(Instant)} / {@link #max(Instant)} methods, anywhere in the engine's range of years {@code
 * -999999} to {@code 999999} ({@link Instant} itself reaches a thousand times further), and make
 * every drawn instant a whole number of some unit since the epoch with {@link
 * #precision(ChronoUnit)}. Values shrink toward 2000-01-01T00:00:00Z, or the nearest bound when that is out of range. One
 * draw in twenty on average targets an interesting instant: the Unix epoch, the 32-bit and 64-bit
 * boundaries, a leap-second boundary or a daylight-saving transition of some time zone.
 *
 * <p>The fluent methods can be called in any order. Each rejects, with an {@link
 * IllegalArgumentException}, what cannot work whatever is set afterwards: a bound outside the years
 * {@code -999999} to {@code 999999}, a precision that is not one of the six units, and the settings
 * given so far when they do not fit together ({@code min} after {@code max}, or bounds with no
 * value at the precision). A setting that was not given is not held against the others until the
 * first draw, where it counts as its default: {@code min} alone may be beyond the year 9999, since
 * a {@code max} can follow in either order, and without one the first draw fails with an {@link
 * IllegalArgumentException} because the default {@code max} is the last day of 9999.
 */
public final class InstantGenerator implements Generator<Instant> {
    /** The engine's instant range: the civil range of years {@code [-999999, 999999]}. */
    static final Instant MIN_ENGINE = Instant.ofEpochSecond(-31_619_087_596_800L);

    static final Instant MAX_ENGINE = Instant.ofEpochSecond(31_494_784_780_799L, 999_999_999);

    /** The default range every date-bearing generator shares: years 1 to 9999. */
    static final Instant DEFAULT_MIN = DateTimeGenerator.DEFAULT_MIN.toInstant(ZoneOffset.UTC);

    static final Instant DEFAULT_MAX = DateTimeGenerator.DEFAULT_MAX.toInstant(ZoneOffset.UTC);

    private final Instant min;
    private final Instant max;
    private final Instant lo;
    private final Instant hi;
    private final ChronoUnit precision;
    private final int precisionCode;

    public InstantGenerator() {
        this(null, null, ChronoUnit.NANOS);
    }

    /**
     * @param min the inclusive lower bound
     * @param max the inclusive upper bound
     * @throws IllegalArgumentException if {@code min} is after {@code max}
     */
    public InstantGenerator(Instant min, Instant max) {
        this(Objects.requireNonNull(min), Objects.requireNonNull(max), ChronoUnit.NANOS);
    }

    private InstantGenerator(Instant min, Instant max, ChronoUnit precision) {
        this.lo = Bounds.or(min, DEFAULT_MIN);
        this.hi = Bounds.or(max, DEFAULT_MAX);
        validate(lo, "instants");
        validate(hi, "instants");
        this.min = min;
        this.max = max;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "instants");
        Instant widestLo = Bounds.or(min, MIN_ENGINE);
        Instant widestHi = Bounds.or(max, MAX_ENGINE);
        Bounds.ordered("instants", min, max, widestLo, widestHi);
        EngineAccess.setup().validateTimestamp(widestLo, widestHi, precisionCode);
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
        return new InstantGenerator(Objects.requireNonNull(min), max, precision);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public InstantGenerator max(Instant max) {
        return new InstantGenerator(min, Objects.requireNonNull(max), precision);
    }

    /**
     * Draw only instants that are a whole number of {@code precision} units since the epoch:
     * {@link ChronoUnit#NANOS} (the default), {@link ChronoUnit#MICROS}, {@link ChronoUnit#MILLIS},
     * {@link ChronoUnit#SECONDS}, {@link ChronoUnit#MINUTES} or {@link ChronoUnit#HOURS}. Bounds
     * holding no such instant are rejected with an {@link IllegalArgumentException}.
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
        Bounds.ordered("instants", min, max, lo, hi);
        return tc.generateTimestamp(lo, hi, precisionCode);
    }
}
