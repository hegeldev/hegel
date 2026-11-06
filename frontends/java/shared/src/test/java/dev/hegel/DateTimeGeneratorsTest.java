package dev.hegel;

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

import dev.hegel.Generators.Temporal;
import dev.hegel.generators.temporal.Ambiguous;
import dev.hegel.generators.temporal.DateTimeGenerator;
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
import java.time.Period;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;
import java.time.temporal.ChronoUnit;
import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
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
                Temporal.dates().weekdays(DayOfWeek.SATURDAY, DayOfWeek.SUNDAY),
                d -> d.getDayOfWeek() == DayOfWeek.SATURDAY || d.getDayOfWeek() == DayOfWeek.SUNDAY);
        assertAllExamples(Temporal.dates().months(Month.FEBRUARY), d -> d.getMonth() == Month.FEBRUARY);
        assertAllExamples(
                Temporal.dates().weekdays(Set.of(DayOfWeek.MONDAY)).months(Set.of(Month.JANUARY, Month.DECEMBER)),
                d -> d.getDayOfWeek() == DayOfWeek.MONDAY
                        && (d.getMonth() == Month.JANUARY || d.getMonth() == Month.DECEMBER));
        LocalDate lo = LocalDate.of(2024, 1, 1);
        LocalDate hi = LocalDate.of(2024, 1, 31);
        assertAllExamples(
                Temporal.dates().min(lo).max(hi).weekdays(DayOfWeek.FRIDAY),
                d -> !d.isBefore(lo) && !d.isAfter(hi) && d.getDayOfWeek() == DayOfWeek.FRIDAY);
        assertEquals(LocalDate.of(2000, 1, 1), minimal(Temporal.dates(), d -> true));
        assertEquals(LocalDate.of(2000, 1, 3), minimal(Temporal.dates().weekdays(DayOfWeek.MONDAY), d -> true));
    }

    @Test
    void datesPassTheirOptionsHandleToTheEngine() {
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporal.dates());
        assertEquals(0L, fake.dateOptionsPassed);
        assertNull(fake.weekdayMask);
        Generator<LocalDate> restricted =
                Temporal.dates().weekdays(DayOfWeek.MONDAY, DayOfWeek.SUNDAY).months(Month.JANUARY);
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
        assertThrows(IllegalArgumentException.class, () -> Temporal.dates().weekdays());
        assertThrows(IllegalArgumentException.class, () -> Temporal.dates().months(Set.of()));
        assertThrows(IllegalArgumentException.class, () -> Temporal.dates().min(LocalDate.of(1_000_000, 1, 1)));
        assertThrows(IllegalArgumentException.class, () -> Temporal.dates().max(LocalDate.of(-1_000_000, 1, 1)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.dates()
                        .min(LocalDate.of(2024, 1, 1))
                        .max(LocalDate.of(2024, 1, 6))
                        .weekdays(DayOfWeek.SUNDAY));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.dates()
                        .min(LocalDate.of(2024, 1, 1))
                        .max(LocalDate.of(2024, 1, 31))
                        .months(Month.MARCH));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.dates().min(LocalDate.of(2024, 1, 2)).max(LocalDate.of(2024, 1, 1)));
    }

    // --- times and datetimes ---

    @Test
    void timesAndDatetimesHonourPrecision() {
        assertAllExamples(Temporal.times().precision(ChronoUnit.MINUTES), t -> t.getSecond() == 0 && t.getNano() == 0);
        assertAllExamples(Temporal.times().precision(ChronoUnit.HOURS), t -> t.getMinute() == 0 && t.getSecond() == 0);
        assertAllExamples(Temporal.times().precision(ChronoUnit.MILLIS), t -> t.getNano() % 1_000_000 == 0);
        assertAllExamples(Temporal.datetimes().precision(ChronoUnit.SECONDS), dt -> dt.getNano() == 0);
        assertAllExamples(Temporal.datetimes().precision(ChronoUnit.MICROS), dt -> dt.getNano() % 1000 == 0);
        assertTrue(findAny(Temporal.times(), t -> t.getNano() % 1000 != 0) != null);
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporal.times());
        assertEquals(Abi.PRECISION_NANOSECONDS, fake.timePrecision);
        drawWith(fake, Temporal.times().precision(ChronoUnit.MICROS));
        assertEquals(Abi.PRECISION_MICROSECONDS, fake.timePrecision);
    }

    @Test
    void timesAndDatetimesValidate() {
        assertThrows(IllegalArgumentException.class, () -> Temporal.times().precision(ChronoUnit.DAYS));
        assertThrows(IllegalArgumentException.class, () -> Temporal.datetimes().precision(ChronoUnit.HALF_DAYS));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.datetimes().min(LocalDateTime.of(1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.datetimes().max(LocalDateTime.of(-1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.times().min(LocalTime.NOON).max(LocalTime.MIDNIGHT));
    }

    @Test
    void precisionTheEngineCannotMeetFailsAtTheFirstDraw() {
        LocalTime odd = LocalTime.of(12, 0, 30);
        Generator<LocalTime> gen = Temporal.times().min(odd).max(odd).precision(ChronoUnit.MINUTES);
        assertThrows(IllegalArgumentException.class, () -> assertAllExamples(gen, t -> true));
    }

    // --- instants ---

    @Test
    void instantsStayInRange() {
        assertAllExamples(Temporal.instants(), i -> !i.isBefore(MIN_ENGINE) && !i.isAfter(MAX_ENGINE));
        Instant lo = Instant.parse("2020-01-01T00:00:00Z");
        Instant hi = Instant.parse("2020-12-31T23:59:59Z");
        assertAllExamples(Temporal.instants().min(lo).max(hi), i -> !i.isBefore(lo) && !i.isAfter(hi));
        assertAllExamples(Temporal.instants().precision(ChronoUnit.SECONDS), i -> i.getNano() == 0);
        assertAllExamples(Temporal.instants().precision(ChronoUnit.MINUTES), i -> i.getEpochSecond() % 60 == 0);
        assertEquals(Instant.parse("2000-01-01T00:00:00Z"), minimal(Temporal.instants(), i -> true));
        assertTrue(findAny(Temporal.instants(), i -> i.getEpochSecond() < 0) != null);
    }

    @Test
    void instantsValidate() {
        assertThrows(IllegalArgumentException.class, () -> Temporal.instants().min(MIN_ENGINE.minusNanos(1)));
        assertThrows(IllegalArgumentException.class, () -> Temporal.instants().max(MAX_ENGINE.plusNanos(1)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.instants().min(Instant.EPOCH).max(Instant.EPOCH.minusNanos(1)));
        assertThrows(IllegalArgumentException.class, () -> Temporal.instants().precision(ChronoUnit.WEEKS));
    }

    // --- offsets ---

    @Test
    void offsetsHonourGranularityAndBounds() {
        assertAllExamples(
                Temporal.offsets().granularity(OffsetGranularity.HOURS), z -> z.getTotalSeconds() % 3600 == 0);
        assertAllExamples(
                Temporal.offsets().granularity(OffsetGranularity.QUARTER_HOURS), z -> z.getTotalSeconds() % 900 == 0);
        assertAllExamples(
                Temporal.offsets().granularity(OffsetGranularity.MINUTES), z -> z.getTotalSeconds() % 60 == 0);
        assertTrue(findAny(Temporal.offsets(), z -> z.getTotalSeconds() % 60 != 0) != null);
        ZoneOffset lo = ZoneOffset.ofHours(-2);
        ZoneOffset hi = ZoneOffset.ofHoursMinutes(5, 30);
        assertAllExamples(
                Temporal.offsets().min(lo).max(hi),
                z -> z.getTotalSeconds() >= lo.getTotalSeconds() && z.getTotalSeconds() <= hi.getTotalSeconds());
        assertEquals(ZoneOffset.UTC, minimal(Temporal.offsets(), z -> true));
        FakeLibhegel fake = new FakeLibhegel();
        drawWith(fake, Temporal.offsets());
        assertEquals(Abi.OFFSET_GRANULARITY_SECONDS, fake.offsetGranularity);
        drawWith(fake, Temporal.offsets().granularity(OffsetGranularity.HOURS));
        assertEquals(Abi.OFFSET_GRANULARITY_HOURS, fake.offsetGranularity);
        assertThrows(
                IllegalArgumentException.class, () -> Temporal.offsets().min(hi).max(lo));
    }

    // --- zones ---

    @Test
    void zonesAreRegionsBothSidesKnow() {
        Set<String> available = ZoneId.getAvailableZoneIds();
        assertAllExamples(Temporal.zones(), z -> available.contains(z.getId()) && !(z instanceof ZoneOffset));
        assertAllExamples(Temporal.zones().legacy(true), z -> available.contains(z.getId()));
        Hegel.test(
                tc -> {
                    ZoneId z = tc.draw(Temporal.zones().links(false));
                    assertFalse(tc.timezoneInfo(0, z.getId()).link(), z.getId());
                },
                new Settings().database(Database.disabled()));
        assertAllExamples(
                Temporal.zones().names("Europe/London", "Asia/Calcutta"),
                z -> z.getId().equals("Europe/London") || z.getId().equals("Asia/Calcutta"));
        assertAllExamples(
                Temporal.zones().names(List.of("Asia/Kolkata")), z -> z.getId().equals("Asia/Kolkata"));
        assertEquals(ZoneId.of("UTC"), minimal(Temporal.zones(), z -> true));
        AtomicBoolean drewLink = new AtomicBoolean(false);
        Hegel.test(
                tc -> {
                    ZoneId z = tc.draw(Temporal.zones());
                    if (tc.timezoneInfo(0, z.getId()).link()) {
                        drewLink.set(true);
                    }
                },
                new Settings().database(Database.disabled()).testCases(500));
        assertTrue(drewLink.get());
        assertTrue(findAny(Temporal.zones(), z -> z.getId().startsWith("Europe/")) != null);
    }

    @Test
    void zonesFilterTheHostListThroughTheEngine() {
        FakeLibhegel fake = new FakeLibhegel();
        assertEquals(ZoneId.of("UTC"), drawWith(fake, Temporal.zones()));
        assertEquals(List.of("America/New_York", "Asia/Calcutta", "Europe/London", "UTC"), fake.timezoneNames);
        assertEquals(true, fake.timezoneLinks);
        assertEquals(false, fake.timezoneLegacy);
        drawWith(fake, Temporal.zones().links(false).legacy(true));
        assertEquals(List.of("America/New_York", "EST5EDT", "Europe/London", "UTC"), fake.timezoneNames);
        drawWith(fake, Temporal.zones().names("Asia/Calcutta", "UTC"));
        assertEquals(List.of("Asia/Calcutta", "UTC"), fake.timezoneNames);
    }

    @Test
    void zonesValidate() {
        assertThrows(IllegalArgumentException.class, () -> Temporal.zones().names());
        assertThrows(IllegalArgumentException.class, () -> Temporal.zones().names("Mars/Olympus_Mons"));
        ZoneGenerator hostOnly = Temporal.zones().names(HostOnlyZone.zone().getId());
        assertAllExamples(hostOnly, z -> z.equals(HostOnlyZone.zone()));
        Generator<ZoneId> bundledHostOnly = hostOnly.source(ZoneSource.bundled());
        IllegalArgumentException unknown =
                assertThrows(IllegalArgumentException.class, () -> assertAllExamples(bundledHostOnly, z -> true));
        assertTrue(unknown.getMessage().contains("not a zone the source knows"), unknown.getMessage());
    }

    @Test
    void tzdbVersionIsAnIanaRelease() {
        assertTrue(Generators.Temporal.tzdbVersion().matches("\\d{4}[a-z]"), Generators.Temporal.tzdbVersion());
        try {
            Engine.setForTesting(new FakeLibhegel());
            assertEquals("2026a", Generators.Temporal.tzdbVersion());
        } finally {
            Engine.reset();
        }
    }

    // --- zoned datetimes ---

    private static Generator<Libhegel.ZonedDatetime> raw(ZonedDateTimeGenerator gen) {
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
        Hegel.test(
                tc -> {
                    ZonedDateTime value = tc.draw(Temporal.zonedDateTimes());
                    assertEquals(
                            value.toInstant(),
                            ZonedDateTime.ofLocal(value.toLocalDateTime(), value.getZone(), value.getOffset())
                                    .toInstant());
                    assertTrue(value.getZone().getRules().isValidOffset(value.toLocalDateTime(), value.getOffset()));
                },
                new Settings().database(Database.disabled()).testCases(300));
        Hegel.test(
                tc -> {
                    Libhegel.ZonedDatetime drawn = tc.draw(raw(Temporal.zonedDateTimes()));
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
        assertTrue(findAny(raw(Temporal.zonedDateTimes()), d -> d.kind() == Abi.TIMEZONE_KIND_GAP) != null);
        assertTrue(
                findAny(raw(Temporal.zonedDateTimes()), d -> d.kind() == Abi.TIMEZONE_KIND_FOLD && d.fold()) != null);
        assertTrue(
                findAny(raw(Temporal.zonedDateTimes()), d -> d.kind() == Abi.TIMEZONE_KIND_FOLD && !d.fold()) != null);

        ZonedDateTimeGenerator fold =
                Temporal.zonedDateTimes().zones(just(NEW_YORK)).min(FOLD).max(FOLD);
        assertAllExamples(fold, z -> z.toLocalDateTime().equals(FOLD));
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4))) != null);
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-5))) != null);
        assertAllExamples(fold.ambiguous(Ambiguous.EARLIER), z -> z.getOffset().equals(ZoneOffset.ofHours(-4)));
        assertAllExamples(fold.ambiguous(Ambiguous.LATER), z -> z.getOffset().equals(ZoneOffset.ofHours(-5)));

        LocalDateTime before = GAP.minusHours(1);
        LocalDateTime after = GAP.plusHours(1);
        assertAllExamples(
                Temporal.zonedDateTimes().zones(just(NEW_YORK)).min(before).max(after),
                z -> !z.toLocalDateTime().isBefore(before)
                        && !z.toLocalDateTime().isAfter(after));
    }

    @Test
    void zonedDatetimesHonourBoundsAndPrecision() {
        LocalDateTime lo = LocalDateTime.of(2021, 6, 1, 0, 0);
        LocalDateTime hi = LocalDateTime.of(2021, 6, 30, 23, 59);
        assertAllExamples(
                Temporal.zonedDateTimes().min(lo).max(hi),
                z -> !z.toLocalDateTime().isBefore(lo) && !z.toLocalDateTime().isAfter(hi));
        assertAllExamples(
                Temporal.datetimes().min(lo).max(hi).zones(Temporal.zones()),
                z -> !z.toLocalDateTime().isBefore(lo) && !z.toLocalDateTime().isAfter(hi));
        Instant from = Instant.parse("2021-06-01T00:00:00Z");
        Instant to = Instant.parse("2021-06-02T00:00:00Z");
        assertAllExamples(
                Temporal.zonedDateTimes().min(from).max(to),
                z -> !z.toInstant().isBefore(from) && !z.toInstant().isAfter(to));
        assertAllExamples(
                Temporal.zonedDateTimes().min(from.atZone(NEW_YORK)).max(to.atZone(ZoneOffset.UTC)),
                z -> !z.toInstant().isBefore(from) && !z.toInstant().isAfter(to));
        assertAllExamples(
                Temporal.zonedDateTimes().precision(ChronoUnit.MINUTES), z -> z.getSecond() == 0 && z.getNano() == 0);
        assertAllExamples(
                Temporal.zonedDateTimes()
                        .zones(just(NEW_YORK))
                        .min(from)
                        .max(to)
                        .precision(ChronoUnit.HOURS),
                z -> z.getZone().equals(NEW_YORK)
                        && z.getMinute() == 0
                        && !z.toInstant().isBefore(from));
        ZonedDateTime smallest = minimal(Temporal.zonedDateTimes(), z -> true);
        assertEquals(ZoneId.of("UTC"), smallest.getZone());
        assertEquals(LocalDateTime.of(2000, 1, 1, 0, 0), smallest.toLocalDateTime());
    }

    @Test
    void theLastKindOfBoundSetWins() {
        FakeLibhegel fake = new FakeLibhegel();
        LocalDateTime civil = LocalDateTime.of(2020, 1, 1, 0, 0);
        drawWith(fake, Temporal.zonedDateTimes().min(Instant.EPOCH).max(civil));
        assertNull(fake.instantBounds);
        assertEquals(LocalDateTime.of(-999_999, 1, 1, 0, 0), fake.civilBounds[0]);
        assertEquals(civil, fake.civilBounds[1]);
        fake = new FakeLibhegel();
        drawWith(fake, Temporal.zonedDateTimes().min(civil).max(Instant.EPOCH));
        assertNull(fake.civilBounds);
        assertEquals(MIN_ENGINE, fake.instantBounds[0]);
        assertEquals(Instant.EPOCH, fake.instantBounds[1]);
        fake = new FakeLibhegel();
        drawWith(fake, Temporal.zonedDateTimes().max(Instant.EPOCH).min(civil));
        assertEquals(civil, fake.civilBounds[0]);
        assertEquals(LocalDateTime.of(999_999, 12, 31, 23, 59, 59, 999_999_999), fake.civilBounds[1]);
        fake = new FakeLibhegel();
        drawWith(fake, Temporal.zonedDateTimes().max(civil).min(Instant.EPOCH));
        assertEquals(Instant.EPOCH, fake.instantBounds[0]);
        assertEquals(MAX_ENGINE, fake.instantBounds[1]);
        fake = new FakeLibhegel();
        drawWith(
                fake,
                Temporal.zonedDateTimes().min(civil).max(civil.plusDays(1)).min(civil.plusHours(1)));
        assertArrayEquals(new LocalDateTime[] {civil.plusHours(1), civil.plusDays(1)}, fake.civilBounds);
        fake = new FakeLibhegel();
        drawWith(
                fake,
                Temporal.zonedDateTimes()
                        .max(Instant.EPOCH)
                        .min(Instant.EPOCH.minusSeconds(5))
                        .max(Instant.EPOCH.plusSeconds(5)));
        assertArrayEquals(
                new Instant[] {Instant.EPOCH.minusSeconds(5), Instant.EPOCH.plusSeconds(5)}, fake.instantBounds);
        fake = new FakeLibhegel();
        drawWith(
                fake,
                Temporal.datetimes()
                        .min(civil)
                        .max(civil.plusDays(1))
                        .precision(ChronoUnit.SECONDS)
                        .zones(Temporal.zones())
                        .ambiguous(Ambiguous.LATER));
        assertArrayEquals(new LocalDateTime[] {civil, civil.plusDays(1)}, fake.civilBounds);
        assertEquals(Abi.PRECISION_SECONDS, fake.zonedPrecision);
        assertEquals(Abi.IMAGINARY_AVOID, fake.zonedImaginary);
        assertEquals(Abi.AMBIGUOUS_LATER, fake.zonedAmbiguous);
        fake = new FakeLibhegel();
        drawWith(fake, Temporal.zonedDateTimes());
        assertEquals(Abi.IMAGINARY_AVOID, fake.zonedImaginary);
        assertEquals(Abi.AMBIGUOUS_BOTH, fake.zonedAmbiguous);
    }

    @Test
    void zonedDatetimesValidate() {
        LocalDateTime civil = LocalDateTime.of(2020, 1, 1, 0, 0);
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.zonedDateTimes().min(civil).max(civil.minusNanos(1)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.zonedDateTimes().min(LocalDateTime.of(1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.zonedDateTimes().max(LocalDateTime.of(-1_000_000, 1, 1, 0, 0)));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.zonedDateTimes().min(Instant.EPOCH).max(Instant.EPOCH.minusNanos(1)));
        assertThrows(
                IllegalArgumentException.class, () -> Temporal.zonedDateTimes().min(MIN_ENGINE.minusNanos(1)));
        assertThrows(
                IllegalArgumentException.class, () -> Temporal.zonedDateTimes().max(MAX_ENGINE.plusNanos(1)));
        assertThrows(
                IllegalArgumentException.class, () -> Temporal.zonedDateTimes().precision(ChronoUnit.DAYS));
    }

    @Test
    void aDisagreementBetweenEngineAndHostRejectsTheCase() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.zonedValue = new Libhegel.ZonedDatetime(
                "UTC",
                LocalDateTime.of(2000, 1, 1, 0, 0),
                0,
                true,
                false,
                Abi.TIMEZONE_KIND_UNIQUE,
                0,
                0,
                Instant.ofEpochSecond(946684801));
        assertThrows(AssumeRejected.class, () -> drawWith(fake, Temporal.zonedDateTimes()));
        FakeLibhegel agreeing = new FakeLibhegel();
        assertEquals(
                ZonedDateTime.of(LocalDateTime.of(2000, 1, 1, 0, 0), ZoneId.of("UTC")),
                drawWith(agreeing, Temporal.zonedDateTimes()));
    }

    @Test
    void hostOnlyZonesAttachTheWallClockDirectly() {
        ZoneOffset five = ZoneOffset.ofHours(5);
        LocalDateTime lo = LocalDateTime.of(2022, 1, 1, 0, 0);
        LocalDateTime hi = LocalDateTime.of(2022, 1, 2, 0, 0);
        assertAllExamples(
                Temporal.zonedDateTimes().zones(just(five)).min(lo).max(hi),
                z -> z.getOffset().equals(five)
                        && !z.toLocalDateTime().isBefore(lo)
                        && !z.toLocalDateTime().isAfter(hi));
        Instant from = Instant.parse("2022-01-01T00:00:00Z");
        Instant to = Instant.parse("2022-01-01T06:00:00Z");
        assertAllExamples(
                Temporal.zonedDateTimes().zones(just(five)).min(from).max(to),
                z -> z.getZone().equals(five)
                        && !z.toInstant().isBefore(from)
                        && !z.toInstant().isAfter(to));
        ZoneId host = HostOnlyZone.zone();
        ZoneSource.setDefault(ZoneSource.bundled());
        try {
            LocalDateTime before = GAP.minusHours(2);
            LocalDateTime after = GAP.plusHours(2);
            assertAllExamples(
                    Temporal.zonedDateTimes().zones(just(host)).min(before).max(after),
                    z -> z.getZone().equals(host)
                            && !z.toLocalDateTime().isBefore(before)
                            && !z.toLocalDateTime().isAfter(after)
                            && NEW_YORK.getRules().isValidOffset(z.toLocalDateTime(), z.getOffset()));
            ZonedDateTimeGenerator fold =
                    Temporal.zonedDateTimes().zones(just(host)).min(FOLD).max(FOLD);
            assertAllExamples(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4)));
            assertAllExamples(
                    fold.ambiguous(Ambiguous.EARLIER), z -> z.getOffset().equals(ZoneOffset.ofHours(-4)));
            assertAllExamples(
                    fold.ambiguous(Ambiguous.LATER), z -> z.getOffset().equals(ZoneOffset.ofHours(-5)));
            Instant foldStart = Instant.parse("2024-11-03T06:30:00Z");
            Instant foldEnd = Instant.parse("2024-11-03T09:30:00Z");
            Hegel.test(
                    tc -> {
                        ZonedDateTime z = tc.draw(Temporal.zonedDateTimes()
                                .zones(just(host))
                                .min(foldStart)
                                .max(foldEnd));
                        assertTrue(
                                !z.toInstant().isBefore(foldStart)
                                        && !z.toInstant().isAfter(foldEnd),
                                z.toString());
                    },
                    new Settings().database(Database.disabled()).suppressHealthCheck(HealthCheck.FILTER_TOO_MUCH));
        } finally {
            ZoneSource.setDefault(ZoneSource.javaTime());
        }
    }

    @Test
    void zonesOnlyThisJvmKnowsDrawThroughItsZoneRules() {
        ZoneId host = HostOnlyZone.zone();
        ZonedDateTimeGenerator fold =
                Temporal.zonedDateTimes().zones(just(host)).min(FOLD).max(FOLD);
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4))) != null);
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-5))) != null);
        assertAllExamples(
                Temporal.zonedDateTimes()
                        .zones(Temporal.zones().names(host.getId()))
                        .min(GAP.minusHours(1))
                        .max(GAP),
                z -> z.getZone().equals(host) && z.toLocalDateTime().getHour() != 2);
    }

    @Test
    void hostOnlyEdgesRejectRatherThanError() {
        ZoneId host = HostOnlyZone.zone();
        Instant foldStart = Instant.parse("2024-11-03T06:30:00Z");
        FakeLibhegel fake = new FakeLibhegel();
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporal.zonedDateTimes()
                                .zones(just(host))
                                .min(foldStart)
                                .max(foldStart.plusSeconds(3600))));
        assertEquals(
                foldStart,
                drawWith(
                                fake,
                                Temporal.zonedDateTimes()
                                        .zones(just(host))
                                        .min(foldStart)
                                        .max(foldStart.plusSeconds(3600))
                                        .ambiguous(Ambiguous.LATER))
                        .toInstant());
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporal.zonedDateTimes()
                                .zones(just(host))
                                .min(Instant.parse("2024-11-03T05:30:00Z"))
                                .max(Instant.parse("2024-11-03T06:00:00Z"))));
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporal.zonedDateTimes().zones(just(host)).min(GAP).max(GAP)));
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporal.zonedDateTimes()
                                .zones(just(host))
                                .min(Instant.parse("2024-11-03T05:30:00Z"))
                                .max(Instant.parse("2024-11-03T05:45:00Z"))
                                .ambiguous(Ambiguous.LATER)));
        assertEquals(
                MIN_ENGINE.plusSeconds(5 * 3600),
                drawWith(
                                fake,
                                Temporal.zonedDateTimes()
                                        .zones(just(ZoneOffset.ofHours(-5)))
                                        .min(MIN_ENGINE)
                                        .max(Instant.EPOCH))
                        .toInstant());
        assertEquals(
                Instant.EPOCH,
                drawWith(
                                fake,
                                Temporal.zonedDateTimes()
                                        .zones(just(ZoneOffset.ofHours(5)))
                                        .min(Instant.EPOCH)
                                        .max(MAX_ENGINE))
                        .toInstant());
        assertThrows(
                AssumeRejected.class,
                () -> drawWith(
                        fake,
                        Temporal.zonedDateTimes()
                                .zones(just(ZoneOffset.ofHours(5)))
                                .min(MAX_ENGINE.minusSeconds(3600))
                                .max(MAX_ENGINE)));
    }

    // --- durations ---

    @Test
    void durationsSpanBothSigns() {
        assertTrue(findAny(Temporal.durations(), Duration::isNegative) != null);
        assertTrue(findAny(Temporal.durations(), d -> d.compareTo(Duration.ofDays(400)) > 0) != null);
        Duration lo = Duration.ofSeconds(-1, 500_000_000);
        Duration hi = Duration.ofMillis(250);
        assertAllExamples(Temporal.durations().min(lo).max(hi), d -> d.compareTo(lo) >= 0 && d.compareTo(hi) <= 0);
        assertAllExamples(Temporal.durations().min(lo).max(lo), d -> d.equals(lo));
        assertAllExamples(Temporal.durations().precision(ChronoUnit.SECONDS), d -> d.getNano() == 0);
        assertAllExamples(Temporal.durations().precision(ChronoUnit.MILLIS), d -> d.getNano() % 1_000_000 == 0);
        assertAllExamples(
                Temporal.durations()
                        .min(Duration.ofHours(-3))
                        .max(Duration.ofHours(3))
                        .precision(ChronoUnit.HOURS),
                d -> d.toSeconds() % 3600 == 0 && d.getNano() == 0);
        assertEquals(Duration.ZERO, minimal(Temporal.durations(), d -> true));
        assertEquals(Duration.ofSeconds(1), minimal(Temporal.durations().min(Duration.ofSeconds(1)), d -> true));
        assertThrows(
                IllegalArgumentException.class,
                () -> Temporal.durations().min(hi).max(lo));
        assertThrows(IllegalArgumentException.class, () -> Temporal.durations().precision(ChronoUnit.DAYS));
    }

    // --- spans ---

    @Test
    void spansHonourTheirUnits() {
        assertAllExamples(Temporal.spans(), p -> p != null);
        assertTrue(findAny(Temporal.spans(), p -> p.getYears() < 0) != null);
        assertTrue(findAny(Temporal.spans(), p -> p.getDays() > 0 && p.getMonths() != 0) != null);
        assertAllExamples(
                Temporal.spans().years(0, 5),
                p -> p.getYears() >= 0 && p.getYears() <= 5 && p.getMonths() == 0 && p.getDays() == 0);
        assertAllExamples(
                Temporal.spans().days(1, 2).months(-1, 1),
                p -> p.getYears() == 0 && p.getDays() >= 1 && p.getDays() <= 2 && Math.abs(p.getMonths()) <= 1);
        assertAllExamples(
                Temporal.spans().nonNegative(true), p -> p.getYears() >= 0 && p.getMonths() >= 0 && p.getDays() >= 0);
        assertAllExamples(Temporal.spans().nonNegative(true).days(0, 10), p -> p.getDays() >= 0 && p.getDays() <= 10);
        assertEquals(Period.ZERO, minimal(Temporal.spans(), p -> true));
        assertEquals(Period.ofDays(3), minimal(Temporal.spans().days(3, 9), p -> true));
        assertThrows(IllegalArgumentException.class, () -> Temporal.spans().years(1, 0));
        Generator<Period> negative = Temporal.spans().nonNegative(true).months(-1, 1);
        assertThrows(IllegalArgumentException.class, () -> assertAllExamples(negative, p -> true));
        FakeLibhegel fake = new FakeLibhegel();
        fake.spanValue = new long[] {1, 2, 0, 3, 0, 0, 0, 0, 0, 0};
        assertEquals(Period.of(1, 2, 3), drawWith(fake, Temporal.spans()));
        assertEquals(Abi.SPAN_SIGN_MIXED, fake.spanSign);
        assertEquals(3, fake.spanUnitBounds.size());
        assertArrayEquals(
                new long[] {Abi.SPAN_UNIT_YEARS, Integer.MIN_VALUE, Integer.MAX_VALUE}, fake.spanUnitBounds.get(0));
        fake = new FakeLibhegel();
        drawWith(fake, Temporal.spans().nonNegative(true));
        assertEquals(Abi.SPAN_SIGN_NONNEGATIVE, fake.spanSign);
        assertArrayEquals(new long[] {Abi.SPAN_UNIT_DAYS, 0, Integer.MAX_VALUE}, fake.spanUnitBounds.get(2));
        fake = new FakeLibhegel();
        drawWith(fake, Temporal.spans().months(2, 4));
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
        assertAllExamples(Generators.durations().min(Duration.ZERO), d -> !d.isNegative());
        assertAllExamples(
                Temporal.datetimes().min(FOLD).max(FOLD).timezones(just(NEW_YORK)),
                zdt -> zdt.getZone().equals(NEW_YORK) && zdt.toLocalDateTime().equals(FOLD));
    }
}
