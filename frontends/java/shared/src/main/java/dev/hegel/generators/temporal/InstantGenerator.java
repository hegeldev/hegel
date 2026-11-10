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
 * <p>Bounds in the wrong order, and bounds that hold no value at the precision, are rejected with
 * an {@link IllegalArgumentException} by the method that makes them so. A bound that was not set
 * counts as its default, so to draw beyond the year 9999 set {@code max} before {@code min}, and
 * before the year 1 {@code min} before {@code max}.
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
     */
    public InstantGenerator(Instant min, Instant max) {
        this(Objects.requireNonNull(min), Objects.requireNonNull(max), ChronoUnit.NANOS);
    }

    private InstantGenerator(Instant min, Instant max, ChronoUnit precision) {
        this.lo = min != null ? min : DEFAULT_MIN;
        this.hi = max != null ? max : DEFAULT_MAX;
        validate(lo, "instants");
        validate(hi, "instants");
        Bounds.ordered("instants", min, max, lo, hi);
        this.min = min;
        this.max = max;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "instants");
        EngineAccess.setup().validateTimestamp(lo, hi, precisionCode);
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
        return tc.generateTimestamp(lo, hi, precisionCode);
    }
}
