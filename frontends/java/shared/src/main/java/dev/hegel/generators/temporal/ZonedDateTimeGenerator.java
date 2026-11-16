package dev.hegel.generators.temporal;

import dev.hegel.EngineAccess;
import dev.hegel.Generator;
import dev.hegel.HegelException;
import dev.hegel.Label;
import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.ref.Reference;
import java.time.Instant;
import java.time.LocalDateTime;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;
import java.time.temporal.ChronoUnit;
import java.util.Objects;

/**
 * Generates {@link ZonedDateTime} values: a zone, a wall-clock time in it and, when the clocks
 * repeated that time, which repeat is meant.
 *
 * <p>The bounds are either on the wall clock of whatever zone is drawn ({@link
 * #min(LocalDateTime)} / {@link #max(LocalDateTime)}; by default years 1 to 9999, as for {@link
 * DateTimeGenerator}) or on the instant ({@link #min(Instant)} / {@link #max(Instant)}, or a {@link
 * ZonedDateTime} standing for its instant), never both. A bound that was not set counts as its
 * default, so a {@code min} beyond the year 9999 needs a {@code max} as well, and a {@code max}
 * before the year 1 a {@code min}. Values shrink down a ladder of zones to {@code UTC}, then toward
 * 2000-01-01T00:00.
 *
 * <p>The fluent methods can be called in any order. Each rejects, with an {@link
 * IllegalArgumentException}, what cannot work whatever is set afterwards: a bound outside the years
 * {@code -999999} to {@code 999999}, a precision that is not one of the six units, and the
 * settings given so far when they do not fit together (bounds of both kinds, {@code min} after
 * {@code max}, bounds with no value at the precision, a {@link ZoneGenerator} together with {@link
 * #source(ZoneSource)}). A setting that was not given is not held against the others until the
 * first draw, where it counts as its default: {@code min} alone may be beyond the year 9999, since
 * a {@code max} can follow in either order, and without one the first draw fails with an {@link
 * IllegalArgumentException}, as it does for whatever a {@link ZoneGenerator} given as the zones
 * leaves to its own first draw.
 *
 * <p>The engine knows every zone's daylight-saving and offset transitions, from a {@link
 * ZoneSource}: one draw in twenty on average lands within a second, an hour or a day of one, or of
 * a well-known instant such as the Unix epoch, so the skipped and repeated wall times where date
 * bugs live are generated deliberately. A repeated wall time is drawn as either instant by default;
 * {@link #ambiguous(Ambiguous)} picks one. A skipped wall time cannot be a {@link ZonedDateTime},
 * so it is never drawn; the real wall times either side of the gap are.
 *
 * <p>Which zones are drawn, and from which zone source, is decided by {@link #zones(Generator)}
 * and {@link #source(ZoneSource)}:
 *
 * <ul>
 *   <li>With neither, the zones are every region zone of the default zone source this JVM knows
 *       (see {@link ZoneGenerator}), and with only {@link #source(ZoneSource)} those of that
 *       source. The default is read when the generator is created.
 *   <li>With a {@link ZoneGenerator} as the zones, that generator's source is used. Give the
 *       source to it, with {@link ZoneGenerator#source(ZoneSource)}: also calling {@link
 *       #source(ZoneSource)} here, before or after, is an {@link IllegalArgumentException}.
 *   <li>With any other generator of zones (a mapped or filtered {@link ZoneGenerator}, {@code
 *       just(...)}, {@code sampledFrom(...)}), each drawn zone is looked up by its id in the
 *       source given to {@link #source(ZoneSource)}, or else in the default as it was when this
 *       generator was created. A region zone that source does not know is an {@link
 *       IllegalArgumentException} naming it. A {@link ZoneOffset}, and a fixed-offset zone such
 *       as {@code ZoneId.of("UTC+05:00")} that the source does not list, needs no zone data: its
 *       wall-clock time is drawn directly, within the bounds as they read at that offset.
 * </ul>
 *
 * <p>The value is built from the drawn zone, wall-clock time and repeat with this JVM's own zone
 * rules. With {@link ZoneSource#javaTime()}, the default, the engine drew from those same rules,
 * so the value is exactly what was drawn. With another source (such as {@link
 * ZoneSource#bundled()}) the value means what this JVM's rules say, which is the drawn instant
 * wherever the two databases agree; a drawn wall-clock time this JVM skips, or resolves to an
 * instant outside instant bounds, rejects the test case as if by {@code assume(false)}. Two cases
 * under {@link ZoneSource#javaTime()} are rejected likewise, neither of which a zone of the JDK's
 * own database reaches: after the year 10000, which takes explicit bounds, the engine has an
 * approximation of zone rules that are not one yearly pair of transitions; and such rules can make
 * {@code java.time} resolve a wall-clock time to one instant and read that instant back as another
 * wall-clock time, which no value can honour.
 */
public final class ZonedDateTimeGenerator implements Generator<ZonedDateTime> {
    private static final String WHAT = "zonedDatetimes";

    private final Generator<? extends ZoneId> zones;
    private final ZoneSource source;
    private final ZoneSource defaultSource;
    private final LocalDateTime civilMin;
    private final LocalDateTime civilMax;
    private final Instant instantMin;
    private final Instant instantMax;
    private final ChronoUnit precision;
    private final int precisionCode;
    private final Ambiguous ambiguous;

    private final ZoneGenerator builtIn;
    private final ZoneSource resolved;
    private final boolean civilBounded;
    private final boolean instantBounded;
    private final LocalDateTime civilLo;
    private final LocalDateTime civilHi;
    private final Instant instantLo;
    private final Instant instantHi;
    private final SpecCache spec = new SpecCache();

    public ZonedDateTimeGenerator() {
        this(null, null, ZoneSource.getDefault(), null, null, null, null, ChronoUnit.NANOS, Ambiguous.BOTH);
    }

    private ZonedDateTimeGenerator(
            Generator<? extends ZoneId> zones,
            ZoneSource source,
            ZoneSource defaultSource,
            LocalDateTime civilMin,
            LocalDateTime civilMax,
            Instant instantMin,
            Instant instantMax,
            ChronoUnit precision,
            Ambiguous ambiguous) {
        this.civilBounded = civilMin != null || civilMax != null;
        this.instantBounded = instantMin != null || instantMax != null;
        this.civilLo = Bounds.or(civilMin, DateTimeGenerator.DEFAULT_MIN);
        this.civilHi = Bounds.or(civilMax, DateTimeGenerator.DEFAULT_MAX);
        this.instantLo = Bounds.or(instantMin, InstantGenerator.DEFAULT_MIN);
        this.instantHi = Bounds.or(instantMax, InstantGenerator.DEFAULT_MAX);
        DateTimeGenerator.validateCivil(civilLo, WHAT);
        DateTimeGenerator.validateCivil(civilHi, WHAT);
        InstantGenerator.validate(instantLo, WHAT);
        InstantGenerator.validate(instantHi, WHAT);
        if (zones instanceof ZoneGenerator) {
            if (source != null) {
                throw new IllegalArgumentException(WHAT + ": the zones are a zones() generator, which has a zone"
                        + " source of its own; give the source to it, as zones().source(...), not to source(...)");
            }
            this.builtIn = (ZoneGenerator) zones;
            this.resolved = builtIn.source();
        } else {
            this.resolved = source != null ? source : defaultSource;
            this.builtIn = zones == null ? ZoneGenerator.of(resolved) : null;
        }
        this.zones = zones;
        this.source = source;
        this.defaultSource = defaultSource;
        this.civilMin = civilMin;
        this.civilMax = civilMax;
        this.instantMin = instantMin;
        this.instantMax = instantMax;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, WHAT);
        this.ambiguous = Objects.requireNonNull(ambiguous);
        validateGiven();
    }

    /**
     * Reject the settings given so far if no later call could make them work: each bound that was
     * not set is the engine's extreme here, not its default, and a generator with no bounds has
     * the engine's whole range. Bounds of both kinds both go to the engine, which rejects the pair.
     */
    private void validateGiven() {
        LocalDateTime civilWidestLo = Bounds.or(civilMin, DateTimeGenerator.MIN_CIVIL);
        LocalDateTime civilWidestHi = Bounds.or(civilMax, DateTimeGenerator.MAX_CIVIL);
        Instant instantWidestLo = Bounds.or(instantMin, InstantGenerator.MIN_ENGINE);
        Instant instantWidestHi = Bounds.or(instantMax, InstantGenerator.MAX_ENGINE);
        Bounds.ordered(WHAT, civilMin, civilMax, civilWidestLo, civilWidestHi);
        Bounds.ordered(WHAT, instantMin, instantMax, instantWidestLo, instantWidestHi);
        EngineAccess.setup()
                .zonedSpec(
                        civilBounded ? civilWidestLo : null,
                        civilBounded ? civilWidestHi : null,
                        instantBounded ? instantWidestLo : null,
                        instantBounded ? instantWidestHi : null,
                        precisionCode,
                        Abi.IMAGINARY_AVOID,
                        ambiguous.engineValue);
    }

    /** What {@link DateTimeGenerator#zones(Generator)} makes: wall-clock bounds, each null when not set. */
    static ZonedDateTimeGenerator of(
            Generator<? extends ZoneId> zones, LocalDateTime min, LocalDateTime max, ChronoUnit precision) {
        return new ZonedDateTimeGenerator(
                zones, null, ZoneSource.getDefault(), min, max, null, null, precision, Ambiguous.BOTH);
    }

    private ZonedDateTimeGenerator civil(LocalDateTime min, LocalDateTime max) {
        return new ZonedDateTimeGenerator(
                zones, source, defaultSource, min, max, instantMin, instantMax, precision, ambiguous);
    }

    private ZonedDateTimeGenerator instant(Instant min, Instant max) {
        return new ZonedDateTimeGenerator(
                zones, source, defaultSource, civilMin, civilMax, min, max, precision, ambiguous);
    }

    /**
     * Draw each value's zone from {@code zones}. A {@link ZoneGenerator} brings its own zone
     * source; the zones of any other generator are looked up in the source given to {@link
     * #source(ZoneSource)}, or else in the default zone source as it was when this generator was
     * created, and a region zone that source does not know fails the draw that drew it with an
     * {@link IllegalArgumentException}.
     *
     * @param zones the generator of the zone each value is drawn in
     * @return a copy drawing its zones from {@code zones}
     * @throws IllegalArgumentException if {@code zones} is a {@link ZoneGenerator} and {@link
     *     #source(ZoneSource)} was called
     */
    public ZonedDateTimeGenerator zones(Generator<? extends ZoneId> zones) {
        return new ZonedDateTimeGenerator(
                zones, source, defaultSource, civilMin, civilMax, instantMin, instantMax, precision, ambiguous);
    }

    /**
     * Draw from {@code source} rather than the default zone source: its zones when {@link
     * #zones(Generator)} was not called, and its transitions for the zones of a generator other
     * than a {@link ZoneGenerator}.
     *
     * @param source the time zone data to draw from
     * @return a copy with the source set
     * @throws IllegalArgumentException if the zones are a {@link ZoneGenerator}, which takes the
     *     source itself, as {@code zones().source(source)}
     */
    public ZonedDateTimeGenerator source(ZoneSource source) {
        return new ZonedDateTimeGenerator(
                zones, source, defaultSource, civilMin, civilMax, instantMin, instantMax, precision, ambiguous);
    }

    /**
     * @param min the inclusive lower bound on the wall-clock time in whatever zone is drawn
     * @return a copy with the bound set
     * @throws IllegalArgumentException if an instant bound was set
     */
    public ZonedDateTimeGenerator min(LocalDateTime min) {
        return civil(Objects.requireNonNull(min), civilMax);
    }

    /**
     * @param max the inclusive upper bound on the wall-clock time in whatever zone is drawn
     * @return a copy with the bound set
     * @throws IllegalArgumentException if an instant bound was set
     */
    public ZonedDateTimeGenerator max(LocalDateTime max) {
        return civil(civilMin, Objects.requireNonNull(max));
    }

    /**
     * @param min the inclusive lower bound on the instant
     * @return a copy with the bound set
     * @throws IllegalArgumentException if a wall-clock bound was set
     */
    public ZonedDateTimeGenerator min(Instant min) {
        return instant(Objects.requireNonNull(min), instantMax);
    }

    /**
     * @param max the inclusive upper bound on the instant
     * @return a copy with the bound set
     * @throws IllegalArgumentException if a wall-clock bound was set
     */
    public ZonedDateTimeGenerator max(Instant max) {
        return instant(instantMin, Objects.requireNonNull(max));
    }

    /**
     * @param min the inclusive lower bound, as the instant it denotes
     * @return a copy with the bound set
     * @throws IllegalArgumentException if a wall-clock bound was set
     */
    public ZonedDateTimeGenerator min(ZonedDateTime min) {
        return min(min.toInstant());
    }

    /**
     * @param max the inclusive upper bound, as the instant it denotes
     * @return a copy with the bound set
     * @throws IllegalArgumentException if a wall-clock bound was set
     */
    public ZonedDateTimeGenerator max(ZonedDateTime max) {
        return max(max.toInstant());
    }

    /**
     * Draw only wall-clock times that are a whole number of {@code precision} units since
     * midnight: {@link ChronoUnit#NANOS} (the default), {@link ChronoUnit#MICROS}, {@link
     * ChronoUnit#MILLIS}, {@link ChronoUnit#SECONDS}, {@link ChronoUnit#MINUTES} or {@link
     * ChronoUnit#HOURS}. Wall-clock bounds holding no such time are rejected with an {@link
     * IllegalArgumentException}.
     *
     * @param precision the unit every drawn time of day is a whole number of
     * @return a copy with the precision set
     */
    public ZonedDateTimeGenerator precision(ChronoUnit precision) {
        return new ZonedDateTimeGenerator(
                zones, source, defaultSource, civilMin, civilMax, instantMin, instantMax, precision, ambiguous);
    }

    /**
     * @param ambiguous which instant a wall-clock time the clocks repeated means ({@link
     *     Ambiguous#BOTH} by default)
     * @return a copy with the policy set
     */
    public ZonedDateTimeGenerator ambiguous(Ambiguous ambiguous) {
        return new ZonedDateTimeGenerator(
                zones, source, defaultSource, civilMin, civilMax, instantMin, instantMax, precision, ambiguous);
    }

    /** The zoned spec a draw uses, with the default for each bound that was not set. */
    private SpecHandle buildSpec(TestCase tc) {
        Bounds.ordered(WHAT, civilMin, civilMax, civilLo, civilHi);
        Bounds.ordered(WHAT, instantMin, instantMax, instantLo, instantHi);
        return tc.zonedSpec(
                instantBounded ? null : civilLo,
                instantBounded ? null : civilHi,
                instantBounded ? instantLo : null,
                instantBounded ? instantHi : null,
                precisionCode,
                Abi.IMAGINARY_AVOID,
                ambiguous.engineValue);
    }

    /** @hidden */
    @Override
    public ZonedDateTime doDraw(TestCase tc) {
        SpecHandle zoned = spec.get(tc, this::buildSpec);
        tc.startSpan(Label.ZONED_DATETIME);
        try {
            if (builtIn != null) {
                Libhegel.ZonedDatetime drawn = tc.generateZonedDatetime(builtIn.spec(tc), zoned);
                return resolve(tc, drawn, ZoneId.of(drawn.zone()));
            }
            return in(tc, zones.doDraw(tc), zoned);
        } finally {
            tc.stopSpan(false);
        }
    }

    private ZonedDateTime in(TestCase tc, ZoneId zone, SpecHandle zoned) {
        if (zone instanceof ZoneOffset) {
            return fixed(tc, zone, (ZoneOffset) zone);
        }
        String id = zone.getId();
        try {
            long handle = resolved.handle(tc.binding());
            if (tc.timezoneInfo(handle, id).known()) {
                return resolve(tc, tc.generateZonedDatetimeForZone(handle, id, zoned), zone);
            }
        } finally {
            Reference.reachabilityFence(resolved);
        }
        if (!zone.getRules().isFixedOffset()) {
            throw new IllegalArgumentException(WHAT + ": the zone source does not know the zone \"" + id
                    + "\" that the zones generator drew; pass the source that has it to source(...)");
        }
        return fixed(tc, zone, zone.getRules().getOffset(Instant.EPOCH));
    }

    /** The drawn value under this JVM's rules, which are the engine's when it drew from {@code java.time}. */
    private ZonedDateTime resolve(TestCase tc, Libhegel.ZonedDatetime drawn, ZoneId zone) {
        ZonedDateTime value = ZonedDateTime.ofLocal(drawn.civil(), zone, null);
        if (drawn.fold() || ambiguous == Ambiguous.LATER) {
            value = value.withLaterOffsetAtOverlap();
        }
        boolean sameWallClock = value.toLocalDateTime().equals(drawn.civil());
        Instant instant = value.toInstant();
        if (resolved != ZoneSource.javaTime() || drawn.civil().getYear() > JavaTimeZones.LAST_EXPANDED_YEAR) {
            tc.assume(sameWallClock && (!instantBounded || within(instant)));
            return value;
        }
        if (sameWallClock && instant.equals(drawn.timestamp())) {
            return value;
        }
        // When java.time reads the drawn instant as the drawn wall clock, the engine's data is
        // right and it is java.time that resolves that wall clock to a different instant.
        tc.assume(!LocalDateTime.ofInstant(drawn.timestamp(), zone).equals(drawn.civil()));
        throw new HegelException(WHAT + ": the engine drew " + drawn.civil() + " in " + zone.getId()
                + " as the instant " + drawn.timestamp() + ", but java.time resolves it to " + value
                + ". The engine drew from java.time's own zone rules, so the two must agree: this is a bug"
                + " in Hegel.");
    }

    private boolean within(Instant instant) {
        return !instant.isBefore(instantLo) && !instant.isAfter(instantHi);
    }

    /** A wall-clock time in a zone with one offset for ever, which needs no zone data. */
    private ZonedDateTime fixed(TestCase tc, ZoneId zone, ZoneOffset offset) {
        LocalDateTime lo = civilLo;
        LocalDateTime hi = civilHi;
        if (instantBounded) {
            LocalDateTime from = LocalDateTime.ofInstant(instantLo, offset);
            LocalDateTime to = LocalDateTime.ofInstant(instantHi, offset);
            lo = ceiling(from.isBefore(DateTimeGenerator.MIN_CIVIL) ? DateTimeGenerator.MIN_CIVIL : from);
            hi = to.isAfter(DateTimeGenerator.MAX_CIVIL) ? DateTimeGenerator.MAX_CIVIL : to;
            tc.assume(!lo.isAfter(hi));
        }
        return ZonedDateTime.ofLocal(tc.generateDatetime(lo, hi, precisionCode), zone, offset);
    }

    private LocalDateTime ceiling(LocalDateTime wall) {
        LocalDateTime truncated = wall.truncatedTo(precision);
        return truncated.isBefore(wall) ? truncated.plus(1, precision) : truncated;
    }
}
