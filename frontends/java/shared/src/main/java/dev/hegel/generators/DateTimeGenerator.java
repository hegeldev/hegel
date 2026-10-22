package dev.hegel.generators;

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

/**
 * Generates {@link LocalDateTime} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is {@code 0001-01-01T00:00} to {@code 9999-12-31T23:59:59.999999999}; narrow it
 * with the fluent {@link #min(LocalDateTime)} / {@link #max(LocalDateTime)} methods, and make every
 * drawn time of day a whole number of some unit with {@link #precision(ChronoUnit)}. Values shrink
 * toward 2000-01-01T00:00:00, or the nearest bound when that is out of range.
 *
 * <p>Attach a time zone to produce zone-aware values: {@link #timezones(Generator)} draws each
 * wall-clock time in a {@link ZoneId} to make a DST-aware {@link ZonedDateTime} (see {@link
 * ZonedDateTimeGenerator}), and {@link #offsets(Generator)} pairs it with a fixed {@link
 * ZoneOffset} to make an {@link OffsetDateTime}.
 */
public final class DateTimeGenerator implements Generator<LocalDateTime> {
    /** The engine's civil range: years {@code [-999999, 999999]}. */
    static final LocalDateTime MIN_CIVIL = LocalDateTime.of(-DateGenerator.MAX_ENGINE_YEAR, 1, 1, 0, 0);

    static final LocalDateTime MAX_CIVIL =
            LocalDateTime.of(DateGenerator.MAX_ENGINE_YEAR, 12, 31, 23, 59, 59, 999_999_999);

    private final LocalDateTime min;
    private final LocalDateTime max;
    private final ChronoUnit precision;
    private final int precisionCode;

    public DateTimeGenerator() {
        this(
                LocalDateTime.of(DateGenerator.DEFAULT_MIN, LocalTime.MIDNIGHT),
                LocalDateTime.of(DateGenerator.DEFAULT_MAX, LocalTime.MAX));
    }

    public DateTimeGenerator(LocalDateTime min, LocalDateTime max) {
        this(min, max, ChronoUnit.NANOS);
    }

    private DateTimeGenerator(LocalDateTime min, LocalDateTime max, ChronoUnit precision) {
        validateCivil(min, "datetimes");
        validateCivil(max, "datetimes");
        if (min.isAfter(max)) {
            throw new IllegalArgumentException("datetimes: min (" + min + ") > max (" + max + ")");
        }
        this.min = min;
        this.max = max;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "datetimes");
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
     * no such datetime are rejected with an {@link IllegalArgumentException} at the first draw.
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
        return tc.generateDatetime(min, max, precisionCode);
    }

    /**
     * Produce DST-aware {@link ZonedDateTime} values: a {@link ZonedDateTimeGenerator} with this
     * generator's wall-clock bounds and precision, drawing its zones from {@code zones} (e.g. {@link
     * Generators#zoneIds()} for every zone both the engine and the JVM know, or {@code
     * just(ZoneId.of("Europe/London"))} to pin one). Skipped wall times are avoided and both
     * repeats of a repeated one are drawn; see {@link ZonedDateTimeGenerator} to change that.
     *
     * @param zones the zone generator
     * @return a generator of zone-aware datetimes
     */
    public ZonedDateTimeGenerator timezones(Generator<? extends ZoneId> zones) {
        return new ZonedDateTimeGenerator().zones(zones).min(min).max(max).precision(precision);
    }

    /**
     * Produce {@link OffsetDateTime} values by pairing each generated wall-clock datetime with a
     * fixed offset drawn from {@code offsets} (e.g. {@link Generators#zoneOffsets()}, or {@code
     * just(ZoneOffset.UTC)} to pin one).
     *
     * @param offsets the offset generator
     * @return a generator of offset-aware datetimes
     */
    public Generator<OffsetDateTime> offsets(Generator<ZoneOffset> offsets) {
        return Generators.tuples(this, offsets).map(parts -> OffsetDateTime.of(parts.value1(), parts.value2()));
    }
}
