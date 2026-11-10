package dev.hegel.generators.temporal;

import dev.hegel.EngineAccess;
import dev.hegel.Generator;
import dev.hegel.Generators;
import dev.hegel.TestCase;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.OffsetDateTime;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;
import java.time.temporal.ChronoUnit;
import java.util.Objects;

/**
 * Generates {@link LocalDateTime} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is {@code 0001-01-01T00:00} to {@code 9999-12-31T23:59:59.999999999}; set
 * either end with the fluent {@link #min(LocalDateTime)} / {@link #max(LocalDateTime)} methods,
 * anywhere in the years {@code -999999} to {@code 999999}, and make every drawn time of day a
 * whole number of some unit with {@link #precision(ChronoUnit)}. Values shrink toward 2000-01-01T00:00:00, or the nearest
 * bound when that is out of range.
 *
 * <p>Bounds in the wrong order, and bounds that hold no value at the precision, are rejected with
 * an {@link IllegalArgumentException} by the method that makes them so. A bound that was not set
 * counts as its default, so to draw beyond the year 9999 set {@code max} before {@code min}, and
 * before the year 1 {@code min} before {@code max}.
 *
 * <p>Attach a time zone to produce zone-aware values: {@link #zones(Generator)} draws each
 * wall-clock time in a {@link ZoneId} to make a DST-aware {@link ZonedDateTime} (see {@link
 * ZonedDateTimeGenerator}), and {@link #offsets(Generator)} pairs it with a fixed {@link
 * ZoneOffset} to make an {@link OffsetDateTime}.
 */
public final class DateTimeGenerator implements Generator<LocalDateTime> {
    /** The engine's civil range: years {@code [-999999, 999999]}. */
    static final LocalDateTime MIN_CIVIL = LocalDateTime.of(-DateGenerator.MAX_ENGINE_YEAR, 1, 1, 0, 0);

    static final LocalDateTime MAX_CIVIL =
            LocalDateTime.of(DateGenerator.MAX_ENGINE_YEAR, 12, 31, 23, 59, 59, 999_999_999);

    /** The default range every date-bearing generator shares: years 1 to 9999. */
    static final LocalDateTime DEFAULT_MIN = LocalDateTime.of(DateGenerator.DEFAULT_MIN, LocalTime.MIDNIGHT);

    static final LocalDateTime DEFAULT_MAX = LocalDateTime.of(DateGenerator.DEFAULT_MAX, LocalTime.MAX);

    private final LocalDateTime min;
    private final LocalDateTime max;
    private final LocalDateTime lo;
    private final LocalDateTime hi;
    private final ChronoUnit precision;
    private final int precisionCode;

    public DateTimeGenerator() {
        this(null, null, ChronoUnit.NANOS);
    }

    /**
     * @param min the inclusive lower bound
     * @param max the inclusive upper bound
     */
    public DateTimeGenerator(LocalDateTime min, LocalDateTime max) {
        this(Objects.requireNonNull(min), Objects.requireNonNull(max), ChronoUnit.NANOS);
    }

    private DateTimeGenerator(LocalDateTime min, LocalDateTime max, ChronoUnit precision) {
        this.lo = min != null ? min : DEFAULT_MIN;
        this.hi = max != null ? max : DEFAULT_MAX;
        validateCivil(lo, "datetimes");
        validateCivil(hi, "datetimes");
        Bounds.ordered("datetimes", min, max, lo, hi);
        this.min = min;
        this.max = max;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "datetimes");
        EngineAccess.setup().validateDatetime(lo, hi, precisionCode);
    }

    static void validateCivil(LocalDateTime dt, String what) {
        if (dt.isBefore(MIN_CIVIL) || dt.isAfter(MAX_CIVIL)) {
            throw new IllegalArgumentException(what + ": year of " + dt + " is outside [-999999, 999999]");
        }
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public DateTimeGenerator min(LocalDateTime min) {
        return new DateTimeGenerator(min, max, precision);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public DateTimeGenerator max(LocalDateTime max) {
        return new DateTimeGenerator(min, max, precision);
    }

    /**
     * Draw only datetimes whose time of day is a whole number of {@code precision} units: {@link
     * ChronoUnit#NANOS} (the default), {@link ChronoUnit#MICROS}, {@link ChronoUnit#MILLIS}, {@link
     * ChronoUnit#SECONDS}, {@link ChronoUnit#MINUTES} or {@link ChronoUnit#HOURS}. Bounds holding
     * no such datetime are rejected with an {@link IllegalArgumentException}.
     *
     * @param precision the unit every drawn time of day is a whole number of
     * @return a copy with the precision set
     */
    public DateTimeGenerator precision(ChronoUnit precision) {
        return new DateTimeGenerator(min, max, precision);
    }

    /** @hidden */
    @Override
    public LocalDateTime doDraw(TestCase tc) {
        return tc.generateDatetime(lo, hi, precisionCode);
    }

    /**
     * Produce DST-aware {@link ZonedDateTime} values: what {@link
     * Generators.Temporals#zonedDatetimes()} gives with this generator's wall-clock bounds and
     * precision and {@link ZonedDateTimeGenerator#zones(Generator) zones(zones)}, which also says
     * which zone source the zones are looked up in (e.g. {@link Generators.Temporals#zones()} for
     * every zone the zone source and the JVM both know, or {@code just(ZoneId.of("Europe/London"))}
     * to pin one). Skipped wall times, which a {@link ZonedDateTime} cannot hold, are never drawn,
     * and both repeats of a repeated one are; see {@link ZonedDateTimeGenerator#ambiguous} to pick
     * one.
     *
     * @param zones the zone generator
     * @return a generator of zone-aware datetimes
     */
    public ZonedDateTimeGenerator zones(Generator<? extends ZoneId> zones) {
        return ZonedDateTimeGenerator.of(zones, min, max, precision);
    }

    /**
     * Produce DST-aware {@link ZonedDateTime} values; the old name of {@link #zones(Generator)}.
     *
     * @param zones the zone generator
     * @return a generator of zone-aware datetimes
     * @deprecated use {@link #zones(Generator)}
     */
    @Deprecated
    public ZonedDateTimeGenerator timezones(Generator<? extends ZoneId> zones) {
        return zones(zones);
    }

    /**
     * Produce {@link OffsetDateTime} values by pairing each generated wall-clock datetime with a
     * fixed offset drawn from {@code offsets} (e.g. {@link Generators.Temporals#offsets()}, or {@code
     * just(ZoneOffset.UTC)} to pin one).
     *
     * @param offsets the offset generator
     * @return a generator of offset-aware datetimes
     */
    public Generator<OffsetDateTime> offsets(Generator<ZoneOffset> offsets) {
        return Generators.tuples(this, offsets).map(parts -> OffsetDateTime.of(parts.value1(), parts.value2()));
    }
}
