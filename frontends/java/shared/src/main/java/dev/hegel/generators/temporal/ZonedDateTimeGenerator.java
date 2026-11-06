package dev.hegel.generators.temporal;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.Instant;
import java.time.LocalDateTime;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;
import java.time.temporal.ChronoUnit;

/**
 * Generates {@link ZonedDateTime} values: a zone, a wall-clock time in it and, when the clocks
 * repeated that time, which repeat is meant.
 *
 * <p>The zones come from {@link #zones(Generator)}, by default every region zone of the default
 * {@link ZoneSource} this JVM knows (see {@link ZoneGenerator}). The bounds are either on the wall clock of
 * whatever zone is drawn ({@link #min(LocalDateTime)} / {@link #max(LocalDateTime)}, the default:
 * the engine's whole range of years {@code [-999999, 999999]}) or on the instant ({@link
 * #min(Instant)} / {@link #max(Instant)}, or a {@link ZonedDateTime} standing for its instant);
 * setting a bound of one kind replaces the bounds of the other. Values shrink down a ladder of
 * zones to {@code UTC}, then toward 2000-01-01T00:00.
 *
 * <p>The engine knows every zone's daylight-saving and offset transitions, from the zone source:
 * one draw in twenty on average lands within a second, an hour or a day of one, or of a well-known
 * instant such as the Unix epoch, so the skipped and repeated wall times where date bugs live are
 * generated deliberately. A repeated wall time is drawn as either instant by default; {@link
 * #ambiguous(Ambiguous)} picks one. A skipped wall time cannot be a {@link ZonedDateTime}, so it is
 * never drawn; the real wall times either side of the gap are.
 *
 * <p>The value is built from the drawn wall-clock time with this JVM's own zone rules. Against the
 * default {@link ZoneSource#javaTime()} source the engine resolves it exactly as they do; against
 * another source (such as {@link ZoneSource#bundled()}) a value the two resolve to different
 * instants rejects the test case as if by {@code assume(false)}. Zones drawn by a generator other
 * than a {@link ZoneGenerator} are evaluated against the default source. A {@link ZoneOffset}, or a
 * region zone the default source lacks, has no transitions the engine can see: the drawn
 * wall-clock time is attached to it as is, with the earlier offset at an overlap unless {@link
 * Ambiguous#LATER} is chosen, and a skipped wall time rejects the test case.
 */
public final class ZonedDateTimeGenerator implements Generator<ZonedDateTime> {
    private final Generator<? extends ZoneId> zones;
    private final LocalDateTime civilMin;
    private final LocalDateTime civilMax;
    private final Instant instantMin;
    private final Instant instantMax;
    private final ChronoUnit precision;
    private final int precisionCode;
    private final Ambiguous ambiguous;
    private final SpecCache spec = new SpecCache();

    public ZonedDateTimeGenerator() {
        this(
                new ZoneGenerator(),
                DateTimeGenerator.MIN_CIVIL,
                DateTimeGenerator.MAX_CIVIL,
                null,
                null,
                ChronoUnit.NANOS,
                Ambiguous.BOTH);
    }

    private ZonedDateTimeGenerator(
            Generator<? extends ZoneId> zones,
            LocalDateTime civilMin,
            LocalDateTime civilMax,
            Instant instantMin,
            Instant instantMax,
            ChronoUnit precision,
            Ambiguous ambiguous) {
        if (civilMin != null) {
            DateTimeGenerator.validateCivil(civilMin, "zonedDateTimes");
            DateTimeGenerator.validateCivil(civilMax, "zonedDateTimes");
            if (civilMin.isAfter(civilMax)) {
                throw new IllegalArgumentException("zonedDateTimes: min (" + civilMin + ") > max (" + civilMax + ")");
            }
        } else {
            InstantGenerator.validate(instantMin, "zonedDateTimes");
            InstantGenerator.validate(instantMax, "zonedDateTimes");
            if (instantMin.isAfter(instantMax)) {
                throw new IllegalArgumentException(
                        "zonedDateTimes: min (" + instantMin + ") > max (" + instantMax + ")");
            }
        }
        this.zones = zones;
        this.civilMin = civilMin;
        this.civilMax = civilMax;
        this.instantMin = instantMin;
        this.instantMax = instantMax;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "zonedDateTimes");
        this.ambiguous = ambiguous;
    }

    private ZonedDateTimeGenerator civil(LocalDateTime min, LocalDateTime max) {
        return new ZonedDateTimeGenerator(zones, min, max, null, null, precision, ambiguous);
    }

    private ZonedDateTimeGenerator instant(Instant min, Instant max) {
        return new ZonedDateTimeGenerator(zones, null, null, min, max, precision, ambiguous);
    }

    /**
     * @param zones the generator of the zone each value is drawn in
     * @return a copy drawing its zones from {@code zones}
     */
    public ZonedDateTimeGenerator zones(Generator<? extends ZoneId> zones) {
        return new ZonedDateTimeGenerator(zones, civilMin, civilMax, instantMin, instantMax, precision, ambiguous);
    }

    /**
     * @param min the inclusive lower bound on the wall-clock time in whatever zone is drawn;
     *     replaces instant bounds
     * @return a copy with the bound set
     */
    public ZonedDateTimeGenerator min(LocalDateTime min) {
        return civil(min, civilMin == null ? DateTimeGenerator.MAX_CIVIL : civilMax);
    }

    /**
     * @param max the inclusive upper bound on the wall-clock time in whatever zone is drawn;
     *     replaces instant bounds
     * @return a copy with the bound set
     */
    public ZonedDateTimeGenerator max(LocalDateTime max) {
        return civil(civilMin == null ? DateTimeGenerator.MIN_CIVIL : civilMin, max);
    }

    /**
     * @param min the inclusive lower bound on the instant; replaces wall-clock bounds
     * @return a copy with the bound set
     */
    public ZonedDateTimeGenerator min(Instant min) {
        return instant(min, instantMin == null ? InstantGenerator.MAX_ENGINE : instantMax);
    }

    /**
     * @param max the inclusive upper bound on the instant; replaces wall-clock bounds
     * @return a copy with the bound set
     */
    public ZonedDateTimeGenerator max(Instant max) {
        return instant(instantMin == null ? InstantGenerator.MIN_ENGINE : instantMin, max);
    }

    /**
     * @param min the inclusive lower bound, as the instant it denotes; replaces wall-clock bounds
     * @return a copy with the bound set
     */
    public ZonedDateTimeGenerator min(ZonedDateTime min) {
        return min(min.toInstant());
    }

    /**
     * @param max the inclusive upper bound, as the instant it denotes; replaces wall-clock bounds
     * @return a copy with the bound set
     */
    public ZonedDateTimeGenerator max(ZonedDateTime max) {
        return max(max.toInstant());
    }

    /**
     * Draw only wall-clock times that are a whole number of {@code precision} units since
     * midnight: {@link ChronoUnit#NANOS} (the default), {@link ChronoUnit#MICROS}, {@link
     * ChronoUnit#MILLIS}, {@link ChronoUnit#SECONDS}, {@link ChronoUnit#MINUTES} or {@link
     * ChronoUnit#HOURS}. Bounds holding no such time are rejected with an {@link
     * IllegalArgumentException} at the first draw.
     *
     * @param precision the unit every drawn time of day is a whole number of
     * @return a copy with the precision set
     */
    public ZonedDateTimeGenerator precision(ChronoUnit precision) {
        return new ZonedDateTimeGenerator(zones, civilMin, civilMax, instantMin, instantMax, precision, ambiguous);
    }

    /**
     * @param ambiguous which instant a wall-clock time the clocks repeated means ({@link
     *     Ambiguous#BOTH} by default)
     * @return a copy with the policy set
     */
    public ZonedDateTimeGenerator ambiguous(Ambiguous ambiguous) {
        return new ZonedDateTimeGenerator(zones, civilMin, civilMax, instantMin, instantMax, precision, ambiguous);
    }

    private SpecHandle buildSpec(TestCase tc) {
        return tc.zonedSpec(
                civilMin, civilMax, instantMin, instantMax, precisionCode, Abi.IMAGINARY_AVOID, ambiguous.engineValue);
    }

    /** @hidden */
    @Override
    public ZonedDateTime doDraw(TestCase tc) {
        tc.startSpan(Label.ZONED_DATETIME);
        try {
            if (zones instanceof ZoneGenerator) {
                return fromEngine(tc, ((ZoneGenerator) zones).spec(tc));
            }
            ZoneId zone = zones.doDraw(tc);
            SpecHandle single = ZoneGenerator.specFor(tc, zone);
            return single != null ? fromEngine(tc, single) : attached(tc, zone);
        } finally {
            tc.stopSpan(false);
        }
    }

    private ZonedDateTime fromEngine(TestCase tc, SpecHandle tzSpec) {
        Libhegel.ZonedDatetime drawn = tc.generateZonedDatetime(tzSpec, spec.get(tc, this::buildSpec));
        ZoneId zone = ZoneId.of(drawn.zone());
        ZonedDateTime value =
                ZonedDateTime.ofLocal(drawn.civil(), zone, ZoneOffset.ofTotalSeconds(drawn.offsetSeconds()));
        tc.assume(value.toInstant().equals(drawn.timestamp()));
        return value;
    }

    private ZonedDateTime attached(TestCase tc, ZoneId zone) {
        LocalDateTime lo = civilMin;
        LocalDateTime hi = civilMax;
        if (lo == null) {
            lo = wallClock(instantMin, zone);
            hi = wallClock(instantMax, zone);
            tc.assume(!lo.isAfter(hi));
        }
        LocalDateTime civil = tc.generateDatetime(lo, hi, precisionCode);
        ZonedDateTime value = ZonedDateTime.ofLocal(civil, zone, null);
        if (ambiguous == Ambiguous.LATER) {
            value = value.withLaterOffsetAtOverlap();
        }
        tc.assume(value.toLocalDateTime().equals(civil));
        if (civilMin == null) {
            Instant instant = value.toInstant();
            tc.assume(!instant.isBefore(instantMin) && !instant.isAfter(instantMax));
        }
        return value;
    }

    private static LocalDateTime wallClock(Instant instant, ZoneId zone) {
        LocalDateTime wall = instant.atZone(zone).toLocalDateTime();
        if (wall.isBefore(DateTimeGenerator.MIN_CIVIL)) {
            return DateTimeGenerator.MIN_CIVIL;
        }
        return wall.isAfter(DateTimeGenerator.MAX_CIVIL) ? DateTimeGenerator.MAX_CIVIL : wall;
    }
}
