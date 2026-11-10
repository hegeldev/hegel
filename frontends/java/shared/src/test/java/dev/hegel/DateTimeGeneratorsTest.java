package dev.hegel;

import static dev.hegel.Generators.forType;
import static dev.hegel.Generators.just;
import static dev.hegel.Utils.assertAllExamples;
import static dev.hegel.Utils.findAny;
import static dev.hegel.Utils.minimal;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.Generators.Temporals;
import dev.hegel.generators.temporal.Ambiguous;
import dev.hegel.generators.temporal.DateGenerator;
import dev.hegel.generators.temporal.DateTimeGenerator;
import dev.hegel.generators.temporal.InstantGenerator;
import dev.hegel.generators.temporal.OffsetGenerator;
import dev.hegel.generators.temporal.OffsetGranularity;
import dev.hegel.generators.temporal.ZoneGenerator;
import dev.hegel.generators.temporal.ZoneSource;
import dev.hegel.generators.temporal.ZonedDateTimeGenerator;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.DayOfWeek;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.Month;
import java.time.OffsetDateTime;
import java.time.Period;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;
import java.time.temporal.ChronoUnit;
import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Supplier;
import org.junit.jupiter.api.Test;

/** The date and time generators against the real engine, plus their fake-driven edge paths. */
class DateTimeGeneratorsTest {
    private static final Instant MIN_ENGINE = Instant.ofEpochSecond(-31_619_087_596_800L);
    private static final Instant MAX_ENGINE = Instant.ofEpochSecond(31_494_784_780_799L, 999_999_999);
    private static final ZoneId NEW_YORK = ZoneId.of("America/New_York");
    private static final LocalDateTime GAP = LocalDateTime.of(2024, 3, 10, 2, 30);
    private static final LocalDateTime FOLD = LocalDateTime.of(2024, 11, 3, 1, 30);

    private static <T> T drawWith(FakeLibhegel fake, Generator<T> gen) {
        return new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false, Reporter.silent()).draw(gen);
    }

    // --- dates ---

    @Test
    void datesRespectWeekdaysAndMonths() {
        assertAllExamples(
                Temporals.dates().weekdays(DayOfWeek.SATURDAY, DayOfWeek.SUNDAY),
                d -> d.getDayOfWeek() == DayOfWeek.SATURDAY || d.getDayOfWeek() == DayOfWeek.SUNDAY);
        assertAllExamples(Temporals.dates().months(Month.FEBRUARY), d -> d.getMonth() == Month.FEBRUARY);
        assertAllExamples(
                Temporals.dates().weekdays(Set.of(DayOfWeek.MONDAY)).months(Set.of(Month.JANUARY, Month.DECEMBER)),
                d -> d.getDayOfWeek() == DayOfWeek.MONDAY
                        && (d.getMonth() == Month.JANUARY || d.getMonth() == Month.DECEMBER));
        LocalDate lo = LocalDate.of(2024, 1, 1);
        LocalDate hi = LocalDate.of(2024, 1, 31);
        assertAllExamples(
                Temporals.dates().min(lo).max(hi).weekdays(DayOfWeek.FRIDAY),
                d -> !d.isBefore(lo) && !d.isAfter(hi) && d.getDayOfWeek() == DayOfWeek.FRIDAY);
        assertEquals(LocalDate.of(2000, 1, 1), minimal(Temporals.dates(), d -> true));
        assertEquals(LocalDate.of(2000, 1, 3), minimal(Temporals.dates().weekdays(DayOfWeek.MONDAY), d -> true));
    }

    @Test
    void datesPassTheirOptionsHandleToTheEngine() {
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporals.dates());
        assertEquals(0L, fake.dateOptionsPassed);
        assertNull(fake.weekdayMask);
        Generator<LocalDate> restricted =
                Temporals.dates().weekdays(DayOfWeek.MONDAY, DayOfWeek.SUNDAY).months(Month.JANUARY);
        drawWith(fake, restricted);
        assertEquals(FakeLibhegel.DATE_OPTIONS, fake.dateOptionsPassed);
        assertEquals(0b1000001, fake.weekdayMask);
        assertEquals(0b1, fake.monthMask);
        drawWith(fake, restricted);
        assertEquals(0, fake.freedDateOptions);
        FakeLibhegel other = new FakeLibhegel();
        drawWith(other, restricted);
        assertEquals(0b1000001, other.weekdayMask);
    }

    @Test
    void datesValidateEagerly() {
        assertThrows(IllegalArgumentException.class, () -> Temporals.dates().weekdays());
        assertThrows(IllegalArgumentException.class, () -> Temporals.dates().months(Set.of()));
        assertThrows(IllegalArgumentException.class, () -> Temporals.dates().min(LocalDate.of(1_000_000, 1, 1)));
        assertThrows(IllegalArgumentException.class, () -> Temporals.dates().max(LocalDate.of(-1_000_000, 1, 1)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.dates().min(LocalDate.of(2024, 1, 2)).max(LocalDate.of(2024, 1, 1)));
    }

    @Test
    void datesWithNoAllowedDayAreRejectedByTheCallThatLeavesNone() {
        DateGenerator week = Temporals.dates().min(LocalDate.of(2024, 1, 1)).max(LocalDate.of(2024, 1, 6));
        IllegalArgumentException noSunday =
                assertThrows(IllegalArgumentException.class, () -> week.weekdays(DayOfWeek.SUNDAY));
        assertTrue(noSunday.getMessage().contains("allowed weekday and month"), noSunday.getMessage());
        assertThrows(IllegalArgumentException.class, () -> week.months(Month.MARCH));
        DateGenerator sundays = Temporals.dates().weekdays(DayOfWeek.SUNDAY).min(LocalDate.of(2024, 1, 1));
        assertThrows(IllegalArgumentException.class, () -> sundays.max(LocalDate.of(2024, 1, 6)));
        assertAllExamples(sundays.max(LocalDate.of(2024, 1, 7)), d -> d.equals(LocalDate.of(2024, 1, 7)));
    }

    // --- default ranges ---

    private static boolean modern(int year) {
        return year >= 1 && year <= 9999;
    }

    @Test
    void everyDateBearingGeneratorDefaultsToYearsOneTo9999() {
        Instant first = Instant.parse("0001-01-01T00:00:00Z");
        Instant last = Instant.parse("9999-12-31T23:59:59.999999999Z");
        assertAllExamples(Temporals.dates(), d -> modern(d.getYear()));
        assertAllExamples(Temporals.datetimes(), dt -> modern(dt.getYear()));
        assertAllExamples(
                Temporals.zonedDatetimes(), z -> modern(z.toLocalDateTime().getYear()));
        assertAllExamples(Temporals.instants(), i -> !i.isBefore(first) && !i.isAfter(last));
        assertAllExamples(forType(LocalDate.class), d -> modern(d.getYear()));
        assertAllExamples(forType(LocalDateTime.class), dt -> modern(dt.getYear()));
        assertAllExamples(
                forType(OffsetDateTime.class), o -> modern(o.toLocalDateTime().getYear()));
        assertAllExamples(
                forType(ZonedDateTime.class), z -> modern(z.toLocalDateTime().getYear()));
        assertAllExamples(forType(Instant.class), i -> !i.isBefore(first) && !i.isAfter(last));
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporals.zonedDatetimes());
        assertArrayEquals(
                new LocalDateTime[] {
                    LocalDateTime.of(1, 1, 1, 0, 0), LocalDateTime.of(9999, 12, 31, 23, 59, 59, 999_999_999)
                },
                fake.civilBounds);
        fake = new FakeLibhegel();
        drawWith(fake, Temporals.zonedDatetimes().min(first.plusSeconds(1)));
        assertArrayEquals(new Instant[] {first.plusSeconds(1), last}, fake.instantBounds);
        fake = new FakeLibhegel();
        drawWith(fake, Temporals.zonedDatetimes().max(last.minusSeconds(1)));
        assertArrayEquals(new Instant[] {first, last.minusSeconds(1)}, fake.instantBounds);
    }

    @Test
    void explicitBoundsReachTheWholeEngineRange() {
        assertTrue(findAny(Temporals.dates().min(LocalDate.of(-999_999, 1, 1)), d -> d.getYear() < 1) != null);
        assertTrue(findAny(Temporals.dates().max(LocalDate.of(999_999, 12, 31)), d -> d.getYear() > 9999) != null);
        assertTrue(findAny(Temporals.datetimes().min(LocalDateTime.of(-999_999, 1, 1, 0, 0)), dt -> dt.getYear() < 1)
                != null);
        assertTrue(
                findAny(Temporals.datetimes().max(LocalDateTime.of(999_999, 12, 31, 0, 0)), dt -> dt.getYear() > 9999)
                        != null);
        assertTrue(findAny(Temporals.instants().min(MIN_ENGINE), i -> i.isBefore(Instant.parse("0001-01-01T00:00:00Z")))
                != null);
        assertTrue(findAny(Temporals.instants().max(MAX_ENGINE), i -> i.isAfter(Instant.parse("9999-12-31T23:59:59Z")))
                != null);
        assertTrue(findAny(
                        Temporals.zonedDatetimes().min(LocalDateTime.of(-999_999, 1, 1, 0, 0)),
                        z -> z.toLocalDateTime().getYear() < 1)
                != null);
        assertTrue(findAny(Temporals.zonedDatetimes().max(MAX_ENGINE), z -> z.getYear() > 9999) != null);
    }

    @Test
    void aBoundBeyondTheDefaultOfTheOtherIsRejectedUntilTheOtherIsSet() {
        LocalDate farDay = LocalDate.of(20_000, 1, 1);
        LocalDateTime far = farDay.atStartOfDay();
        Instant farInstant = far.toInstant(ZoneOffset.UTC);
        List<Supplier<Generator<?>>> beyondTheDefaultMax = List.of(
                () -> Temporals.dates().min(farDay),
                () -> Temporals.datetimes().min(far),
                () -> Temporals.instants().min(farInstant),
                () -> Temporals.zonedDatetimes().min(far),
                () -> Temporals.zonedDatetimes().min(farInstant));
        for (Supplier<Generator<?>> build : beyondTheDefaultMax) {
            IllegalArgumentException unordered = assertThrows(IllegalArgumentException.class, build::get);
            assertTrue(
                    unordered.getMessage().contains("max was not set, so it is the default: set it before min"),
                    unordered.getMessage());
        }
        IllegalArgumentException early = assertThrows(
                IllegalArgumentException.class, () -> Temporals.dates().max(farDay.withYear(-5)));
        assertTrue(
                early.getMessage().contains("min was not set, so it is the default: set it before max"),
                early.getMessage());
        IllegalArgumentException both = assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.dates().max(farDay.plusYears(1)).min(farDay.plusYears(2)));
        assertFalse(both.getMessage().contains("default"), both.getMessage());
        assertAllExamples(Temporals.dates().max(farDay.plusYears(1)).min(farDay), d -> d.getYear() >= 20_000);
        assertAllExamples(Temporals.dates().min(farDay.withYear(-6)).max(farDay.withYear(-5)), d -> d.getYear() <= -5);
        assertAllExamples(Temporals.datetimes().max(far.plusYears(1)).min(far), dt -> dt.getYear() >= 20_000);
        assertAllExamples(
                Temporals.instants().max(farInstant.plusSeconds(60)).min(farInstant), i -> !i.isBefore(farInstant));
        assertAllExamples(
                Temporals.zonedDatetimes().max(far.plusYears(1)).min(far),
                z -> z.toLocalDateTime().getYear() >= 20_000);
        assertAllExamples(
                Temporals.zonedDatetimes().max(farInstant.plusSeconds(60)).min(farInstant),
                z -> !z.toInstant().isBefore(farInstant));
    }

    // --- times and datetimes ---

    @Test
    void timesAndDatetimesHonourPrecision() {
        assertAllExamples(Temporals.times().precision(ChronoUnit.MINUTES), t -> t.getSecond() == 0 && t.getNano() == 0);
        assertAllExamples(Temporals.times().precision(ChronoUnit.HOURS), t -> t.getMinute() == 0 && t.getSecond() == 0);
        assertAllExamples(Temporals.times().precision(ChronoUnit.MILLIS), t -> t.getNano() % 1_000_000 == 0);
        assertAllExamples(Temporals.datetimes().precision(ChronoUnit.SECONDS), dt -> dt.getNano() == 0);
        assertAllExamples(Temporals.datetimes().precision(ChronoUnit.MICROS), dt -> dt.getNano() % 1000 == 0);
        assertTrue(findAny(Temporals.times(), t -> t.getNano() % 1000 != 0) != null);
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporals.times());
        assertEquals(Abi.PRECISION_NANOSECONDS, fake.timePrecision);
        drawWith(fake, Temporals.times().precision(ChronoUnit.MICROS));
        assertEquals(Abi.PRECISION_MICROSECONDS, fake.timePrecision);
    }

    @Test
    void timesAndDatetimesValidate() {
        assertThrows(IllegalArgumentException.class, () -> Temporals.times().precision(ChronoUnit.DAYS));
        assertThrows(IllegalArgumentException.class, () -> Temporals.datetimes().precision(ChronoUnit.HALF_DAYS));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.datetimes().min(LocalDateTime.of(1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.datetimes().max(LocalDateTime.of(-1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.times().min(LocalTime.NOON).max(LocalTime.MIDNIGHT));
    }

    @Test
    void boundsHoldingNoValueAtThePrecisionAreRejectedByTheCallThatLeavesNone() {
        LocalTime odd = LocalTime.of(12, 0, 30);
        LocalDateTime oddDay = LocalDate.of(2024, 1, 1).atTime(odd);
        Instant oddInstant = oddDay.toInstant(ZoneOffset.UTC);
        List<Supplier<Generator<?>>> none = List.of(
                () -> Temporals.times().min(odd).max(odd).precision(ChronoUnit.MINUTES),
                () -> Temporals.times().precision(ChronoUnit.MINUTES).min(odd).max(odd),
                () -> Temporals.datetimes().min(oddDay).max(oddDay).precision(ChronoUnit.MINUTES),
                () -> Temporals.instants().min(oddInstant).max(oddInstant).precision(ChronoUnit.MINUTES),
                () -> Temporals.durations()
                        .min(Duration.ofSeconds(30))
                        .max(Duration.ofSeconds(30))
                        .precision(ChronoUnit.MINUTES),
                () -> Temporals.offsets()
                        .min(ZoneOffset.ofTotalSeconds(30))
                        .max(ZoneOffset.ofTotalSeconds(30))
                        .granularity(OffsetGranularity.MINUTES),
                () -> Temporals.zonedDatetimes().min(oddDay).max(oddDay).precision(ChronoUnit.MINUTES));
        for (Supplier<Generator<?>> build : none) {
            IllegalArgumentException empty = assertThrows(IllegalArgumentException.class, build::get);
            assertTrue(empty.getMessage().contains("no "), empty.getMessage());
        }
    }

    // --- instants ---

    @Test
    void instantsStayInRange() {
        assertAllExamples(Temporals.instants(), i -> !i.isBefore(MIN_ENGINE) && !i.isAfter(MAX_ENGINE));
        Instant lo = Instant.parse("2020-01-01T00:00:00Z");
        Instant hi = Instant.parse("2020-12-31T23:59:59Z");
        assertAllExamples(Temporals.instants().min(lo).max(hi), i -> !i.isBefore(lo) && !i.isAfter(hi));
        assertAllExamples(Temporals.instants().precision(ChronoUnit.SECONDS), i -> i.getNano() == 0);
        assertAllExamples(Temporals.instants().precision(ChronoUnit.MINUTES), i -> i.getEpochSecond() % 60 == 0);
        assertEquals(Instant.parse("2000-01-01T00:00:00Z"), minimal(Temporals.instants(), i -> true));
        assertTrue(findAny(Temporals.instants(), i -> i.getEpochSecond() < 0) != null);
    }

    @Test
    void instantsValidate() {
        assertThrows(IllegalArgumentException.class, () -> Temporals.instants().min(MIN_ENGINE.minusNanos(1)));
        assertThrows(IllegalArgumentException.class, () -> Temporals.instants().max(MAX_ENGINE.plusNanos(1)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.instants().min(Instant.EPOCH).max(Instant.EPOCH.minusNanos(1)));
        assertThrows(IllegalArgumentException.class, () -> Temporals.instants().precision(ChronoUnit.WEEKS));
    }

    @Test
    void generatorsConstructWithBothBounds() {
        LocalDate first = LocalDate.of(2020, 1, 1);
        LocalDate second = LocalDate.of(2020, 1, 2);
        assertAllExamples(new DateGenerator(first, second), d -> d.equals(first) || d.equals(second));
        assertThrows(IllegalArgumentException.class, () -> new DateGenerator(second, first));
        assertThrows(NullPointerException.class, () -> new DateGenerator(first, null));
        assertAllExamples(
                new DateTimeGenerator(first.atStartOfDay(), second.atStartOfDay()),
                dt -> !dt.toLocalDate().isBefore(first) && !dt.toLocalDate().isAfter(second));
        assertThrows(
                IllegalArgumentException.class,
                () -> new DateTimeGenerator(second.atStartOfDay(), first.atStartOfDay()));
        Instant lo = Instant.parse("2020-01-01T00:00:00Z");
        Instant hi = Instant.parse("2020-01-02T00:00:00Z");
        assertAllExamples(new InstantGenerator(lo, hi), i -> !i.isBefore(lo) && !i.isAfter(hi));
        assertThrows(IllegalArgumentException.class, () -> new InstantGenerator(hi, lo));
        assertThrows(NullPointerException.class, () -> new InstantGenerator(null, hi));
        ZoneOffset one = ZoneOffset.ofHours(1);
        ZoneOffset two = ZoneOffset.ofHours(2);
        assertAllExamples(
                new OffsetGenerator(one, two), z -> z.getTotalSeconds() >= 3600 && z.getTotalSeconds() <= 7200);
        assertThrows(IllegalArgumentException.class, () -> new OffsetGenerator(two, one));
    }

    // --- offsets ---

    @Test
    void offsetsHonourGranularityAndBounds() {
        assertAllExamples(
                Temporals.offsets().granularity(OffsetGranularity.HOURS), z -> z.getTotalSeconds() % 3600 == 0);
        assertAllExamples(
                Temporals.offsets().granularity(OffsetGranularity.QUARTER_HOURS), z -> z.getTotalSeconds() % 900 == 0);
        assertAllExamples(
                Temporals.offsets().granularity(OffsetGranularity.MINUTES), z -> z.getTotalSeconds() % 60 == 0);
        assertTrue(findAny(Temporals.offsets(), z -> z.getTotalSeconds() % 60 != 0) != null);
        ZoneOffset lo = ZoneOffset.ofHours(-2);
        ZoneOffset hi = ZoneOffset.ofHoursMinutes(5, 30);
        assertAllExamples(
                Temporals.offsets().min(lo).max(hi),
                z -> z.getTotalSeconds() >= lo.getTotalSeconds() && z.getTotalSeconds() <= hi.getTotalSeconds());
        assertEquals(ZoneOffset.UTC, minimal(Temporals.offsets(), z -> true));
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporals.offsets());
        assertEquals(Abi.OFFSET_GRANULARITY_SECONDS, fake.offsetGranularity);
        drawWith(fake, Temporals.offsets().granularity(OffsetGranularity.HOURS));
        assertEquals(Abi.OFFSET_GRANULARITY_HOURS, fake.offsetGranularity);
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.offsets().min(hi).max(lo));
    }

    // --- zones ---

    @Test
    void zonesAreRegionsBothSidesKnow() {
        Set<String> available = ZoneId.getAvailableZoneIds();
        assertAllExamples(Temporals.zones(), z -> available.contains(z.getId()) && !(z instanceof ZoneOffset));
        assertAllExamples(Temporals.zones().allowLegacy(true), z -> available.contains(z.getId()));
        Hegel.test(
                tc -> {
                    ZoneId z = tc.draw(Temporals.zones().allowLinks(false));
                    assertFalse(tc.timezoneInfo(0, z.getId()).link(), z.getId());
                },
                new Settings().database(Database.disabled()));
        assertAllExamples(
                Temporals.zones().names("Europe/London", "Asia/Calcutta"),
                z -> z.getId().equals("Europe/London") || z.getId().equals("Asia/Calcutta"));
        assertAllExamples(
                Temporals.zones().names(List.of("Asia/Kolkata")), z -> z.getId().equals("Asia/Kolkata"));
        assertEquals(ZoneId.of("UTC"), minimal(Temporals.zones(), z -> true));
        AtomicBoolean drewLink = new AtomicBoolean(false);
        Hegel.test(
                tc -> {
                    ZoneId z = tc.draw(Temporals.zones());
                    if (tc.timezoneInfo(0, z.getId()).link()) {
                        drewLink.set(true);
                    }
                },
                new Settings().database(Database.disabled()).testCases(500));
        assertTrue(drewLink.get());
        assertTrue(findAny(Temporals.zones(), z -> z.getId().startsWith("Europe/")) != null);
    }

    @Test
    void zonesLeaveJavaTimeNamesToTheEngineAndFilterAnotherSourceThroughIt() {
        FakeLibhegel fake = new FakeLibhegel();
        assertEquals(ZoneId.of("UTC"), drawWith(fake, Temporals.zones()));
        assertNull(fake.timezoneNames);
        assertEquals(FakeLibhegel.ZONE_SOURCE, fake.timezoneSource);
        assertEquals(true, fake.timezoneLinks);
        assertEquals(false, fake.timezoneLegacy);
        ZoneGenerator bundled = Temporals.zones().source(ZoneSource.bundled());
        drawWith(fake, bundled);
        assertEquals(List.of("America/New_York", "Asia/Calcutta", "Europe/London", "UTC"), fake.timezoneNames);
        drawWith(fake, bundled.allowLinks(false).allowLegacy(true));
        assertEquals(List.of("America/New_York", "EST5EDT", "Europe/London", "UTC"), fake.timezoneNames);
        assertEquals(false, fake.timezoneLinks);
        assertEquals(true, fake.timezoneLegacy);
        drawWith(fake, Temporals.zones().names("Asia/Calcutta", "UTC"));
        assertEquals(List.of("Asia/Calcutta", "UTC"), fake.timezoneNames);
        FakeLibhegel empty = new FakeLibhegel();
        empty.knownZones = List.of();
        IllegalArgumentException none = assertThrows(IllegalArgumentException.class, () -> drawWith(empty, bundled));
        assertTrue(none.getMessage().contains("no zone this JVM knows"), none.getMessage());
    }

    @Test
    void zonesValidate() {
        assertThrows(IllegalArgumentException.class, () -> Temporals.zones().names());
        assertThrows(IllegalArgumentException.class, () -> Temporals.zones().names("Mars/Olympus_Mons"));
        ZoneGenerator hostOnly = Temporals.zones().names(HostOnlyZone.zone().getId());
        assertAllExamples(hostOnly, z -> z.equals(HostOnlyZone.zone()));
        IllegalArgumentException unknown =
                assertThrows(IllegalArgumentException.class, () -> hostOnly.source(ZoneSource.bundled()));
        assertTrue(
                unknown.getMessage().contains("unknown time zone \"" + HostOnlyZone.ID + "\""), unknown.getMessage());
    }

    @Test
    void tzdbVersionIsAnIanaRelease() {
        assertTrue(Generators.Temporals.tzdbVersion().matches("\\d{4}[a-z]"), Generators.Temporals.tzdbVersion());
        try {
            Engine.setForTesting(new FakeLibhegel());
            assertEquals("2026a", Generators.Temporals.tzdbVersion());
        } finally {
            Engine.reset();
        }
    }

    // --- zoned datetimes ---

    /** What the engine draws in New York over 2024, gaps included, before any {@code java.time} resolution. */
    private static Generator<Libhegel.ZonedDatetime> rawNewYork() {
        return new Generator<>() {
            private SpecHandle tz;
            private SpecHandle zoned;

            @Override
            public Libhegel.ZonedDatetime doDraw(TestCase tc) {
                if (tz == null) {
                    tz = tc.timezoneSpec(List.of("America/New_York"), true, false, 0);
                    zoned = tc.zonedSpec(
                            LocalDateTime.of(2024, 1, 1, 0, 0),
                            LocalDateTime.of(2024, 12, 31, 0, 0),
                            null,
                            null,
                            Abi.PRECISION_NANOSECONDS,
                            Abi.IMAGINARY_ALLOW,
                            Abi.AMBIGUOUS_BOTH);
                }
                return tc.generateZonedDatetime(tz, zoned);
            }
        };
    }

    @Test
    void engineAndHostAgreeOnEveryZonedInstant() {
        Generator<ZonedDateTime> everyZone =
                Temporals.zonedDatetimes().zones(Temporals.zones().allowLegacy(true));
        Hegel.test(
                tc -> {
                    ZonedDateTime value =
                            tc.draw(tc.draw(Generators.booleans()) ? Temporals.zonedDatetimes() : everyZone);
                    assertTrue(value.getZone().getRules().isValidOffset(value.toLocalDateTime(), value.getOffset()));
                    assertTrue(modern(value.toLocalDateTime().getYear()), value.toString());
                },
                new Settings().database(Database.disabled()).testCases(1000));
        Hegel.test(
                tc -> {
                    Libhegel.ZonedDatetime drawn = tc.draw(rawNewYork());
                    ZoneId zone = ZoneId.of(drawn.zone());
                    ZonedDateTime host = drawn.kind() == Abi.TIMEZONE_KIND_GAP
                            ? ZonedDateTime.of(drawn.civil(), zone)
                            : ZonedDateTime.ofLocal(
                                    drawn.civil(), zone, ZoneOffset.ofTotalSeconds(drawn.offsetSeconds()));
                    assertEquals(drawn.timestamp(), host.toInstant());
                    assertTrue(drawn.offsetKnown());
                    if (drawn.kind() == Abi.TIMEZONE_KIND_UNIQUE) {
                        assertEquals(drawn.offsetBefore(), drawn.offsetAfter());
                    } else {
                        assertTrue(drawn.offsetBefore() != drawn.offsetAfter());
                    }
                    if (drawn.kind() == Abi.TIMEZONE_KIND_FOLD) {
                        assertEquals(drawn.fold(), host.getOffset().getTotalSeconds() == drawn.offsetAfter());
                    }
                },
                new Settings().database(Database.disabled()).testCases(300));
    }

    @Test
    void zonedDatetimesReachGapsAndBothFolds() {
        assertTrue(findAny(rawNewYork(), d -> d.kind() == Abi.TIMEZONE_KIND_GAP) != null);
        assertTrue(findAny(rawNewYork(), d -> d.kind() == Abi.TIMEZONE_KIND_FOLD && d.fold()) != null);
        assertTrue(findAny(rawNewYork(), d -> d.kind() == Abi.TIMEZONE_KIND_FOLD && !d.fold()) != null);
        assertTrue(findAny(
                        Temporals.zonedDatetimes(),
                        z -> z.getZone()
                                        .getRules()
                                        .getValidOffsets(z.toLocalDateTime())
                                        .size()
                                == 2)
                != null);

        for (ZonedDateTimeGenerator fold : List.of(
                Temporals.zonedDatetimes().zones(just(NEW_YORK)).min(FOLD).max(FOLD),
                Temporals.zonedDatetimes()
                        .zones(Temporals.zones().names(NEW_YORK.getId()))
                        .min(FOLD)
                        .max(FOLD))) {
            assertAllExamples(fold, z -> z.toLocalDateTime().equals(FOLD));
            assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4))) != null);
            assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-5))) != null);
            assertAllExamples(
                    fold.ambiguous(Ambiguous.EARLIER), z -> z.getOffset().equals(ZoneOffset.ofHours(-4)));
            assertAllExamples(
                    fold.ambiguous(Ambiguous.LATER), z -> z.getOffset().equals(ZoneOffset.ofHours(-5)));
        }

        LocalDateTime before = GAP.minusHours(1);
        LocalDateTime after = GAP.plusHours(1);
        assertAllExamples(
                Temporals.zonedDatetimes().zones(just(NEW_YORK)).min(before).max(after),
                z -> !z.toLocalDateTime().isBefore(before)
                        && !z.toLocalDateTime().isAfter(after));
    }

    @Test
    void zonedDatetimesHonourBoundsAndPrecision() {
        LocalDateTime lo = LocalDateTime.of(2021, 6, 1, 0, 0);
        LocalDateTime hi = LocalDateTime.of(2021, 6, 30, 23, 59);
        assertAllExamples(
                Temporals.zonedDatetimes().min(lo).max(hi),
                z -> !z.toLocalDateTime().isBefore(lo) && !z.toLocalDateTime().isAfter(hi));
        assertAllExamples(
                Temporals.datetimes().min(lo).max(hi).zones(Temporals.zones()),
                z -> !z.toLocalDateTime().isBefore(lo) && !z.toLocalDateTime().isAfter(hi));
        Instant from = Instant.parse("2021-06-01T00:00:00Z");
        Instant to = Instant.parse("2021-06-02T00:00:00Z");
        assertAllExamples(
                Temporals.zonedDatetimes().min(from).max(to),
                z -> !z.toInstant().isBefore(from) && !z.toInstant().isAfter(to));
        assertAllExamples(
                Temporals.zonedDatetimes().min(from.atZone(NEW_YORK)).max(to.atZone(ZoneOffset.UTC)),
                z -> !z.toInstant().isBefore(from) && !z.toInstant().isAfter(to));
        assertAllExamples(
                Temporals.zonedDatetimes().precision(ChronoUnit.MINUTES), z -> z.getSecond() == 0 && z.getNano() == 0);
        assertAllExamples(
                Temporals.zonedDatetimes()
                        .zones(just(NEW_YORK))
                        .min(from)
                        .max(to)
                        .precision(ChronoUnit.HOURS),
                z -> z.getZone().equals(NEW_YORK)
                        && z.getMinute() == 0
                        && !z.toInstant().isBefore(from));
        ZonedDateTime smallest = minimal(Temporals.zonedDatetimes(), z -> true);
        assertEquals(ZoneId.of("UTC"), smallest.getZone());
        assertEquals(LocalDateTime.of(2000, 1, 1, 0, 0), smallest.toLocalDateTime());
    }

    @Test
    void wallClockAndInstantBoundsDoNotMix() {
        LocalDateTime civil = LocalDateTime.of(2020, 1, 1, 0, 0);
        ZonedDateTime zoned = civil.atZone(NEW_YORK);
        List<Runnable> mixed = List.of(
                () -> Temporals.zonedDatetimes().min(Instant.EPOCH).max(civil),
                () -> Temporals.zonedDatetimes().min(civil).max(Instant.EPOCH),
                () -> Temporals.zonedDatetimes().max(Instant.EPOCH).min(civil),
                () -> Temporals.zonedDatetimes().max(civil).min(Instant.EPOCH),
                () -> Temporals.zonedDatetimes().max(civil).min(zoned),
                () -> Temporals.zonedDatetimes().min(civil).max(zoned),
                () -> Temporals.datetimes().min(civil).zones(Temporals.zones()).max(Instant.EPOCH));
        for (Runnable build : mixed) {
            IllegalArgumentException both = assertThrows(IllegalArgumentException.class, build::run);
            assertTrue(both.getMessage().contains("not both"), both.getMessage());
        }
    }

    @Test
    void zonedDatetimesPassTheirSettingsToTheEngine() {
        LocalDateTime civil = LocalDateTime.of(2020, 1, 1, 0, 0);
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporals.zonedDatetimes().max(civil));
        assertNull(fake.instantBounds);
        assertArrayEquals(new LocalDateTime[] {LocalDateTime.of(1, 1, 1, 0, 0), civil}, fake.civilBounds);
        fake = new FakeLibhegel();
        drawWith(
                fake,
                Temporals.zonedDatetimes().min(civil).max(civil.plusDays(1)).min(civil.plusHours(1)));
        assertArrayEquals(new LocalDateTime[] {civil.plusHours(1), civil.plusDays(1)}, fake.civilBounds);
        fake = new FakeLibhegel();
        drawWith(
                fake,
                Temporals.zonedDatetimes()
                        .max(Instant.EPOCH)
                        .min(Instant.EPOCH.minusSeconds(5))
                        .max(Instant.EPOCH.plusSeconds(5)));
        assertNull(fake.civilBounds);
        assertArrayEquals(
                new Instant[] {Instant.EPOCH.minusSeconds(5), Instant.EPOCH.plusSeconds(5)}, fake.instantBounds);
        fake = new FakeLibhegel();
        drawWith(
                fake,
                Temporals.datetimes()
                        .min(civil)
                        .max(civil.plusDays(1))
                        .precision(ChronoUnit.SECONDS)
                        .zones(Temporals.zones())
                        .ambiguous(Ambiguous.LATER));
        assertArrayEquals(new LocalDateTime[] {civil, civil.plusDays(1)}, fake.civilBounds);
        assertEquals(Abi.PRECISION_SECONDS, fake.zonedPrecision);
        assertEquals(Abi.IMAGINARY_AVOID, fake.zonedImaginary);
        assertEquals(Abi.AMBIGUOUS_LATER, fake.zonedAmbiguous);
        fake = new FakeLibhegel();
        drawWith(fake, Temporals.zonedDatetimes());
        assertEquals(Abi.IMAGINARY_AVOID, fake.zonedImaginary);
        assertEquals(Abi.AMBIGUOUS_BOTH, fake.zonedAmbiguous);
    }

    @Test
    void zonedDatetimesValidate() {
        LocalDateTime civil = LocalDateTime.of(2020, 1, 1, 0, 0);
        IllegalArgumentException twice = assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.zonedDatetimes().zones(Temporals.zones()).source(ZoneSource.bundled()));
        assertTrue(twice.getMessage().contains("zones().source(...)"), twice.getMessage());
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.zonedDatetimes().source(ZoneSource.bundled()).zones(Temporals.zones()));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.zonedDatetimes().min(civil).max(civil.minusNanos(1)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.zonedDatetimes().min(LocalDateTime.of(1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.zonedDatetimes().max(LocalDateTime.of(-1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.zonedDatetimes().min(Instant.EPOCH).max(Instant.EPOCH.minusNanos(1)));
        assertThrows(
                IllegalArgumentException.class, () -> Temporals.zonedDatetimes().min(MIN_ENGINE.minusNanos(1)));
        assertThrows(
                IllegalArgumentException.class, () -> Temporals.zonedDatetimes().max(MAX_ENGINE.plusNanos(1)));
        assertThrows(
                IllegalArgumentException.class, () -> Temporals.zonedDatetimes().precision(ChronoUnit.DAYS));
    }

    private static Libhegel.ZonedDatetime drawnAt(String zone, LocalDateTime civil, boolean fold, Instant timestamp) {
        return new Libhegel.ZonedDatetime(zone, civil, 0, true, fold, Abi.TIMEZONE_KIND_UNIQUE, 0, 0, timestamp);
    }

    @Test
    void aDisagreementWithJavaTimeIsReportedAsABugInHegel() {
        LocalDateTime y2k = LocalDateTime.of(2000, 1, 1, 0, 0);
        FakeLibhegel agreeing = new FakeLibhegel();
        assertEquals(ZonedDateTime.of(y2k, ZoneId.of("UTC")), drawWith(agreeing, Temporals.zonedDatetimes()));
        FakeLibhegel otherInstant = new FakeLibhegel();
        otherInstant.zonedValue = drawnAt("UTC", y2k, false, Instant.ofEpochSecond(946684801));
        HegelException instant =
                assertThrows(HegelException.class, () -> drawWith(otherInstant, Temporals.zonedDatetimes()));
        assertTrue(instant.getMessage().contains("bug in Hegel"), instant.getMessage());
        FakeLibhegel skipped = new FakeLibhegel();
        skipped.zonedValue = drawnAt("America/New_York", GAP, false, GAP.toInstant(ZoneOffset.ofHours(-5)));
        HegelException wallClock =
                assertThrows(HegelException.class, () -> drawWith(skipped, Temporals.zonedDatetimes()));
        assertTrue(wallClock.getMessage().contains("America/New_York"), wallClock.getMessage());
        assertThrows(
                HegelException.class,
                () -> drawWith(skipped, Temporals.zonedDatetimes().zones(just(NEW_YORK))));
        FakeLibhegel far = new FakeLibhegel();
        LocalDateTime year10001 = LocalDateTime.of(10_001, 1, 1, 0, 0);
        far.zonedValue = drawnAt("UTC", year10001, false, Instant.EPOCH);
        assertEquals(
                year10001,
                drawWith(far, Temporals.zonedDatetimes().max(MAX_ENGINE)).toLocalDateTime());
        Instant bound = year10001.toInstant(ZoneOffset.UTC).plusSeconds(1);
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(far, Temporals.zonedDatetimes().max(MAX_ENGINE).min(bound)));
    }

    @Test
    void aValueFromAnotherSourceMeansWhatJavaTimeSays() {
        LocalDateTime y2k = LocalDateTime.of(2000, 1, 1, 0, 0);
        Instant instant = y2k.toInstant(ZoneOffset.UTC);
        ZonedDateTimeGenerator bundled = Temporals.zonedDatetimes().source(ZoneSource.bundled());
        FakeLibhegel otherInstant = new FakeLibhegel();
        otherInstant.zonedValue = drawnAt("UTC", y2k, false, instant.plusSeconds(1));
        assertEquals(instant, drawWith(otherInstant, bundled).toInstant());
        assertEquals(
                instant,
                drawWith(otherInstant, bundled.min(instant.minusSeconds(10)).max(instant.plusSeconds(10)))
                        .toInstant());
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(otherInstant, bundled.min(instant.plusSeconds(1)).max(instant.plusSeconds(10))));
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        otherInstant, bundled.min(instant.minusSeconds(10)).max(instant.minusSeconds(1))));

        FakeLibhegel skipped = new FakeLibhegel();
        skipped.zonedValue = drawnAt("America/New_York", GAP, false, GAP.toInstant(ZoneOffset.ofHours(-5)));
        assertThrows(AssumeRejected.class, () -> drawWith(skipped, bundled));

        FakeLibhegel repeated = new FakeLibhegel();
        repeated.zonedValue = drawnAt("America/New_York", FOLD, false, FOLD.toInstant(ZoneOffset.ofHours(-4)));
        assertEquals(ZoneOffset.ofHours(-4), drawWith(repeated, bundled).getOffset());
        assertEquals(
                ZoneOffset.ofHours(-5),
                drawWith(repeated, bundled.ambiguous(Ambiguous.LATER)).getOffset());
        repeated.zonedValue = drawnAt("America/New_York", FOLD, true, FOLD.toInstant(ZoneOffset.ofHours(-5)));
        assertEquals(ZoneOffset.ofHours(-5), drawWith(repeated, bundled).getOffset());
    }

    @Test
    void zonesOfAnotherGeneratorAreDrawnInByName() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.zonedValue = drawnAt("UTC", FOLD, true, FOLD.toInstant(ZoneOffset.ofHours(-5)));
        ZonedDateTime drawn = drawWith(fake, Temporals.zonedDatetimes().zones(just(NEW_YORK)));
        assertEquals(FOLD.atZone(NEW_YORK).withLaterOffsetAtOverlap(), drawn);
        assertEquals(FakeLibhegel.ZONE_SOURCE, fake.zonedSource);
        assertEquals("America/New_York", fake.zonedName);
        IllegalArgumentException unknown = assertThrows(
                IllegalArgumentException.class,
                () -> drawWith(fake, Temporals.zonedDatetimes().zones(just(HostOnlyZone.zone()))));
        assertTrue(unknown.getMessage().contains("\"" + HostOnlyZone.ID + "\""), unknown.getMessage());
        assertTrue(unknown.getMessage().contains("source(...)"), unknown.getMessage());
    }

    @Test
    void aRegionTheSourceLacksIsAnArgumentError() {
        ZoneId host = HostOnlyZone.zone();
        ZonedDateTimeGenerator bundled =
                Temporals.zonedDatetimes().zones(just(host)).source(ZoneSource.bundled());
        IllegalArgumentException unknown =
                assertThrows(IllegalArgumentException.class, () -> assertAllExamples(bundled, z -> true));
        assertTrue(unknown.getMessage().contains("\"" + HostOnlyZone.ID + "\""), unknown.getMessage());
        assertTrue(unknown.getMessage().contains("source(...)"), unknown.getMessage());
    }

    @Test
    void zonesOnlyThisJvmKnowsDrawThroughItsZoneRules() {
        ZoneId host = HostOnlyZone.zone();
        ZonedDateTimeGenerator fold =
                Temporals.zonedDatetimes().zones(just(host)).min(FOLD).max(FOLD);
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4))) != null);
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-5))) != null);
        assertAllExamples(fold.ambiguous(Ambiguous.EARLIER), z -> z.getOffset().equals(ZoneOffset.ofHours(-4)));
        assertAllExamples(fold.ambiguous(Ambiguous.LATER), z -> z.getOffset().equals(ZoneOffset.ofHours(-5)));
        assertAllExamples(
                Temporals.zonedDatetimes()
                        .zones(Temporals.zones().names(host.getId()))
                        .min(GAP.minusHours(1))
                        .max(GAP),
                z -> z.getZone().equals(host) && z.toLocalDateTime().getHour() != 2);
        LocalDateTime before = GAP.minusHours(2);
        LocalDateTime after = GAP.plusHours(2);
        assertAllExamples(
                Temporals.zonedDatetimes().zones(just(host)).min(before).max(after),
                z -> z.getZone().equals(host)
                        && !z.toLocalDateTime().isBefore(before)
                        && !z.toLocalDateTime().isAfter(after)
                        && NEW_YORK.getRules().isValidOffset(z.toLocalDateTime(), z.getOffset()));
        Instant foldStart = Instant.parse("2024-11-03T05:30:00Z");
        Instant foldEnd = Instant.parse("2024-11-03T06:30:00Z");
        ZonedDateTimeGenerator acrossTheFold =
                Temporals.zonedDatetimes().zones(just(host)).min(foldStart).max(foldEnd);
        assertAllExamples(
                acrossTheFold,
                z -> !z.toInstant().isBefore(foldStart) && !z.toInstant().isAfter(foldEnd));
        assertTrue(findAny(acrossTheFold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4))) != null);
        assertTrue(findAny(acrossTheFold, z -> z.getOffset().equals(ZoneOffset.ofHours(-5))) != null);
    }

    @Test
    void fixedOffsetZonesNeedNoZoneData() {
        ZoneOffset five = ZoneOffset.ofHours(5);
        ZoneId utcPlusFive = ZoneId.of("UTC+05:00");
        LocalDateTime lo = LocalDateTime.of(2022, 1, 1, 0, 0);
        LocalDateTime hi = LocalDateTime.of(2022, 1, 2, 0, 0);
        Instant from = Instant.parse("2022-01-01T00:00:30Z");
        Instant to = Instant.parse("2022-01-01T06:00:00Z");
        for (ZoneId zone : List.of(five, utcPlusFive)) {
            for (ZonedDateTimeGenerator gen : List.of(
                    Temporals.zonedDatetimes().zones(just(zone)),
                    Temporals.zonedDatetimes().zones(just(zone)).source(ZoneSource.bundled()))) {
                assertAllExamples(
                        gen.min(lo).max(hi),
                        z -> z.getZone().equals(zone)
                                && z.getOffset().equals(five)
                                && !z.toLocalDateTime().isBefore(lo)
                                && !z.toLocalDateTime().isAfter(hi));
                assertAllExamples(
                        gen.min(from).max(to).precision(ChronoUnit.MINUTES),
                        z -> z.getZone().equals(zone)
                                && z.getSecond() == 0
                                && !z.toInstant().isBefore(from)
                                && !z.toInstant().isAfter(to));
            }
        }
        assertAllExamples(
                Temporals.zonedDatetimes().zones(just(five)).min(from).max(from),
                z -> z.toInstant().equals(from));
        assertAllExamples(
                Temporals.zonedDatetimes().zones(just(five)).min(from).max(to).precision(ChronoUnit.HOURS),
                z -> z.getMinute() == 0
                        && z.toInstant().isAfter(from)
                        && !z.toInstant().isAfter(to));
    }

    @Test
    void fixedOffsetZonesKeepInstantBoundsAtTheEdgesOfTheRange() {
        FakeLibhegel fake = new FakeLibhegel();
        assertEquals(
                MIN_ENGINE.plusSeconds(5 * 3600),
                drawWith(
                                fake,
                                Temporals.zonedDatetimes()
                                        .zones(just(ZoneOffset.ofHours(-5)))
                                        .min(MIN_ENGINE)
                                        .max(Instant.EPOCH))
                        .toInstant());
        assertEquals(
                Instant.EPOCH,
                drawWith(
                                fake,
                                Temporals.zonedDatetimes()
                                        .zones(just(ZoneOffset.ofHours(5)))
                                        .min(Instant.EPOCH)
                                        .max(MAX_ENGINE))
                        .toInstant());
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporals.zonedDatetimes()
                                .zones(just(ZoneOffset.ofHours(5)))
                                .max(MAX_ENGINE)
                                .min(MAX_ENGINE.minusSeconds(3600))));
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporals.zonedDatetimes()
                                .zones(just(ZoneOffset.ofHours(-5)))
                                .min(MIN_ENGINE)
                                .max(MIN_ENGINE.plusSeconds(3600))));
        Instant odd = Instant.parse("2022-01-01T00:00:30Z");
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporals.zonedDatetimes()
                                .zones(just(ZoneOffset.UTC))
                                .min(odd)
                                .max(odd.plusSeconds(10))
                                .precision(ChronoUnit.MINUTES)));
        assertEquals(
                odd.plusSeconds(30),
                drawWith(
                                fake,
                                Temporals.zonedDatetimes()
                                        .zones(just(ZoneId.of("UTC+05:00")))
                                        .min(odd)
                                        .max(odd.plusSeconds(60))
                                        .precision(ChronoUnit.MINUTES))
                        .toInstant());
    }

    // --- durations ---

    @Test
    void durationsSpanBothSigns() {
        assertTrue(findAny(Temporals.durations(), Duration::isNegative) != null);
        assertTrue(findAny(Temporals.durations(), d -> d.compareTo(Duration.ofDays(400)) > 0) != null);
        Duration lo = Duration.ofSeconds(-1, 500_000_000);
        Duration hi = Duration.ofMillis(250);
        assertAllExamples(Temporals.durations().min(lo).max(hi), d -> d.compareTo(lo) >= 0 && d.compareTo(hi) <= 0);
        assertAllExamples(Temporals.durations().min(lo).max(lo), d -> d.equals(lo));
        assertAllExamples(Temporals.durations().precision(ChronoUnit.SECONDS), d -> d.getNano() == 0);
        assertAllExamples(Temporals.durations().precision(ChronoUnit.MILLIS), d -> d.getNano() % 1_000_000 == 0);
        assertAllExamples(
                Temporals.durations()
                        .min(Duration.ofHours(-3))
                        .max(Duration.ofHours(3))
                        .precision(ChronoUnit.HOURS),
                d -> d.toSeconds() % 3600 == 0 && d.getNano() == 0);
        assertEquals(Duration.ZERO, minimal(Temporals.durations(), d -> true));
        assertEquals(Duration.ofSeconds(1), minimal(Temporals.durations().min(Duration.ofSeconds(1)), d -> true));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.durations().min(hi).max(lo));
        assertThrows(IllegalArgumentException.class, () -> Temporals.durations().precision(ChronoUnit.DAYS));
    }

    // --- spans ---

    @Test
    void spansHonourTheirUnits() {
        assertAllExamples(Temporals.spans(), p -> p != null);
        assertTrue(findAny(Temporals.spans(), p -> p.getYears() < 0) != null);
        assertTrue(findAny(Temporals.spans(), p -> p.getDays() > 0 && p.getMonths() != 0) != null);
        assertAllExamples(
                Temporals.spans().years(0, 5),
                p -> p.getYears() >= 0 && p.getYears() <= 5 && p.getMonths() == 0 && p.getDays() == 0);
        assertAllExamples(
                Temporals.spans().days(1, 2).months(-1, 1),
                p -> p.getYears() == 0 && p.getDays() >= 1 && p.getDays() <= 2 && Math.abs(p.getMonths()) <= 1);
        assertAllExamples(
                Temporals.spans().nonNegative(true), p -> p.getYears() >= 0 && p.getMonths() >= 0 && p.getDays() >= 0);
        assertAllExamples(Temporals.spans().nonNegative(true).days(0, 10), p -> p.getDays() >= 0 && p.getDays() <= 10);
        assertEquals(Period.ZERO, minimal(Temporals.spans(), p -> true));
        assertEquals(Period.ofDays(3), minimal(Temporals.spans().days(3, 9), p -> true));
        assertThrows(IllegalArgumentException.class, () -> Temporals.spans().years(1, 0));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.spans().nonNegative(true).months(-1, 1));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.spans().months(-1, 1).nonNegative(true));
        FakeLibhegel fake = new FakeLibhegel();
        fake.spanValue = new long[] {1, 2, 0, 3, 0, 0, 0, 0, 0, 0};
        assertEquals(Period.of(1, 2, 3), drawWith(fake, Temporals.spans()));
        assertEquals(Abi.SPAN_SIGN_MIXED, fake.spanSign);
        assertEquals(3, fake.spanUnitBounds.size());
        assertArrayEquals(
                new long[] {Abi.SPAN_UNIT_YEARS, Integer.MIN_VALUE, Integer.MAX_VALUE}, fake.spanUnitBounds.get(0));
        fake = new FakeLibhegel();
        drawWith(fake, Temporals.spans().nonNegative(true));
        assertEquals(Abi.SPAN_SIGN_NONNEGATIVE, fake.spanSign);
        assertArrayEquals(new long[] {Abi.SPAN_UNIT_DAYS, 0, Integer.MAX_VALUE}, fake.spanUnitBounds.get(2));
        fake = new FakeLibhegel();
        drawWith(fake, Temporals.spans().months(2, 4));
        assertEquals(1, fake.spanUnitBounds.size());
        assertArrayEquals(new long[] {Abi.SPAN_UNIT_MONTHS, 2, 4}, fake.spanUnitBounds.get(0));
    }

    // --- released names ---

    @Test
    @SuppressWarnings("deprecation")
    void releasedNamesAreDeprecatedAliasesOfTheTemporalFactories() throws NoSuchMethodException {
        for (String name : List.of("dates", "times", "datetimes", "zoneOffsets", "zoneIds", "durations")) {
            assertTrue(Generators.class.getMethod(name).isAnnotationPresent(Deprecated.class), name);
        }
        assertTrue(
                DateTimeGenerator.class.getMethod("timezones", Generator.class).isAnnotationPresent(Deprecated.class));
        LocalDate day = LocalDate.of(2024, 2, 29);
        assertAllExamples(Generators.dates().min(day).max(day), d -> d.equals(day));
        assertAllExamples(Generators.times().precision(ChronoUnit.HOURS), t -> t.getMinute() == 0);
        assertAllExamples(Generators.datetimes().min(FOLD).max(FOLD), dt -> dt.equals(FOLD));
        assertAllExamples(
                Generators.zoneOffsets().granularity(OffsetGranularity.HOURS), o -> o.getTotalSeconds() % 3600 == 0);
        assertAllExamples(Generators.zoneIds().names("Europe/London"), z -> z.equals(ZoneId.of("Europe/London")));
        Duration longest = Duration.ofNanos(Long.MAX_VALUE);
        assertAllExamples(Generators.durations(), d -> !d.isNegative() && d.compareTo(longest) <= 0);
        assertTrue(findAny(Generators.durations(), d -> d.compareTo(Duration.ofDays(36_500)) > 0) != null);
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Generators.durations());
        assertEquals(Duration.ZERO, fake.durationMin);
        assertEquals(longest, fake.durationMax);
        assertAllExamples(
                Temporals.datetimes().min(FOLD).max(FOLD).timezones(just(NEW_YORK)),
                zdt -> zdt.getZone().equals(NEW_YORK) && zdt.toLocalDateTime().equals(FOLD));
    }
}
