package dev.hegel;

import static dev.hegel.Generators.just;
import static dev.hegel.Utils.assertAllExamples;
import static dev.hegel.Utils.findAny;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.Generators.Temporals;
import dev.hegel.generators.temporal.Ambiguous;
import dev.hegel.generators.temporal.ZoneGenerator;
import dev.hegel.generators.temporal.ZoneSource;
import dev.hegel.generators.temporal.ZonedDateTimeGenerator;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.ref.WeakReference;
import java.time.DayOfWeek;
import java.time.Instant;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.Month;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;
import java.time.temporal.ChronoUnit;
import java.time.zone.ZoneOffsetTransition;
import java.time.zone.ZoneOffsetTransitionRule;
import java.time.zone.ZoneOffsetTransitionRule.TimeDefinition;
import java.time.zone.ZoneRules;
import java.time.zone.ZoneRulesProvider;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.TreeSet;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.Test;

/** Zone sources: java.time's data through the engine, the builder's shapes, and the default. */
class ZoneSourceTest {
    private static final int[] FAR_YEARS = {2026, 2028, 2037, 2038, 2100, 2500, 10_000, 100_000, 999_000};
    private static final ZoneRules NEW_YORK = ZoneId.of("America/New_York").getRules();
    private static final LocalDateTime FOLD = LocalDateTime.of(2024, 11, 3, 1, 30);

    /** A zone's explicit transitions, and two of its rule's transitions in each of the far years. */
    private static List<ZoneOffsetTransition> samples(ZoneRules rules) {
        List<ZoneOffsetTransition> transitions = new ArrayList<>(rules.getTransitions());
        for (int year : FAR_YEARS) {
            Instant from = LocalDateTime.of(year, 1, 1, 0, 0).toInstant(ZoneOffset.UTC);
            ZoneOffsetTransition next = rules.nextTransition(from);
            for (int i = 0; i < 2 && next != null; i++) {
                transitions.add(next);
                next = rules.nextTransition(next.getInstant());
            }
        }
        return transitions;
    }

    /**
     * The engine's offsets and resolutions agree with {@code rules} either side of each sampled
     * transition before {@code untilYear}: the instants a second before, at and after it, and the
     * wall-clock times at its edges.
     */
    static void assertRoundTrips(Libhegel lib, long source, String name, ZoneRules rules, int untilYear) {
        for (ZoneOffsetTransition t : samples(rules)) {
            if (t.getInstant().atOffset(ZoneOffset.UTC).getYear() >= untilYear) {
                continue;
            }
            for (long delta : new long[] {-1, 0, 1}) {
                Instant at = t.getInstant().plusSeconds(delta);
                assertEquals(
                        rules.getOffset(at).getTotalSeconds(),
                        BindingEdges.offsetAt(lib, source, name, at).offsetSeconds(),
                        name + " at " + at);
            }
            for (LocalDateTime civil : List.of(
                    t.getDateTimeBefore().minusSeconds(1),
                    t.getDateTimeBefore(),
                    t.getDateTimeAfter().minusSeconds(1),
                    t.getDateTimeAfter())) {
                List<ZoneOffset> valid = rules.getValidOffsets(civil);
                Libhegel.TimezoneResolution resolved = BindingEdges.resolve(lib, source, name, civil);
                int kind = valid.isEmpty()
                        ? Abi.TIMEZONE_KIND_GAP
                        : valid.size() == 2 ? Abi.TIMEZONE_KIND_FOLD : Abi.TIMEZONE_KIND_UNIQUE;
                assertEquals(kind, resolved.kind(), name + " at " + civil);
                ZoneOffset earlier =
                        valid.isEmpty() ? rules.getTransition(civil).getOffsetBefore() : valid.get(0);
                assertEquals(civil.toInstant(earlier), resolved.timestamp(), name + " at " + civil);
            }
        }
    }

    /**
     * The engine's offsets agree with {@code rules} every {@code stride} half days through each of
     * {@code years}, and every {@code stride} half hours (and the seconds either side) of the day
     * and a half around each one's start.
     */
    private static void assertOffsetsAgree(
            Libhegel lib, long source, String name, ZoneRules rules, int stride, int... years) {
        for (int year : years) {
            Instant start = LocalDateTime.of(year, 1, 1, 0, 0).toInstant(ZoneOffset.UTC);
            List<Instant> instants = new ArrayList<>();
            for (int halfDay = 0; halfDay < 732; halfDay += stride) {
                instants.add(start.plusSeconds(halfDay * 43_200L));
            }
            for (int halfHour = -36; halfHour <= 36; halfHour += stride) {
                for (int delta = -1; delta <= 1; delta++) {
                    instants.add(start.plusSeconds(halfHour * 1800L + delta));
                }
            }
            for (Instant at : instants) {
                assertEquals(
                        rules.getOffset(at).getTotalSeconds(),
                        BindingEdges.offsetAt(lib, source, name, at).offsetSeconds(),
                        name + " at " + at);
            }
        }
    }

    @Test
    void everyJavaTimeZoneRoundTripsThroughTheEngine() {
        Libhegel lib = Engine.get();
        long source = ZoneSource.javaTime().handle(lib);
        for (String name : new TreeSet<>(ZoneRulesProvider.getAvailableZoneIds())) {
            ZoneRules rules = ZoneRulesProvider.getRules(name, false);
            if (!name.equals(HostOnlyZone.ONE_RULE_ID)) {
                assertRoundTrips(lib, source, name, rules, Integer.MAX_VALUE);
            }
            assertOffsetsAgree(lib, source, name, rules, name.startsWith("Hegel/") ? 1 : 6, 9999);
        }
    }

    private static final ZoneOffset BASE = ZoneOffset.ofHoursMinutes(0, 30);
    private static final ZoneOffset STD = ZoneOffset.ofHours(1);
    private static final ZoneOffset DST = ZoneOffset.ofHours(2);
    private static final ZoneOffset HIGH = ZoneOffset.ofHours(3);

    private static ZoneOffsetTransitionRule rule(
            Month month,
            int dom,
            DayOfWeek dow,
            LocalTime time,
            boolean endOfDay,
            TimeDefinition definition,
            ZoneOffset before,
            ZoneOffset after) {
        return ZoneOffsetTransitionRule.of(month, dom, dow, time, endOfDay, definition, STD, before, after);
    }

    private static ZoneOffsetTransitionRule start(Month month, int dom, DayOfWeek dow) {
        return rule(month, dom, dow, LocalTime.of(2, 0), false, TimeDefinition.WALL, STD, DST);
    }

    private static ZoneOffsetTransitionRule end(Month month, int dom, DayOfWeek dow) {
        return rule(month, dom, dow, LocalTime.of(3, 0), false, TimeDefinition.WALL, DST, STD);
    }

    /** Rules with one transition, in 1970 from {@link #BASE}, after which {@code yearly} apply. */
    private static ZoneRules zone(ZoneOffsetTransitionRule... yearly) {
        return zone(STD, yearly);
    }

    private static ZoneRules zone(ZoneOffset january, ZoneOffsetTransitionRule... yearly) {
        return ZoneRules.of(
                STD,
                BASE,
                List.of(),
                List.of(ZoneOffsetTransition.of(LocalDateTime.of(1970, 1, 1, 0, 0), BASE, january)),
                List.of(yearly));
    }

    @Test
    void everyFormOfJavaTimeRuleRoundTrips() {
        Map<String, ZoneRules> exact = new LinkedHashMap<>();
        exact.put("Test/Fixed", zone(start(Month.MARCH, 21, null), end(Month.SEPTEMBER, 22, null)));
        exact.put("Test/LastDays", zone(start(Month.APRIL, -1, null), end(Month.OCTOBER, -2, null)));
        exact.put("Test/February", zone(start(Month.FEBRUARY, -1, null), end(Month.OCTOBER, 15, null)));
        exact.put(
                "Test/FebruaryWeeks",
                zone(start(Month.FEBRUARY, 8, DayOfWeek.SUNDAY), end(Month.OCTOBER, 1, DayOfWeek.SUNDAY)));
        exact.put(
                "Test/LastSundays",
                zone(
                        rule(
                                Month.MARCH,
                                -1,
                                DayOfWeek.SUNDAY,
                                LocalTime.of(1, 0),
                                false,
                                TimeDefinition.UTC,
                                STD,
                                DST),
                        rule(
                                Month.OCTOBER,
                                -1,
                                DayOfWeek.SUNDAY,
                                LocalTime.of(1, 0),
                                false,
                                TimeDefinition.UTC,
                                DST,
                                STD)));
        exact.put(
                "Test/Southern",
                zone(DST, end(Month.FEBRUARY, -3, DayOfWeek.SATURDAY), start(Month.SEPTEMBER, 1, DayOfWeek.SUNDAY)));
        exact.put(
                "Test/EndOfDay",
                zone(
                        rule(
                                Month.MARCH,
                                8,
                                DayOfWeek.SUNDAY,
                                LocalTime.MIDNIGHT,
                                true,
                                TimeDefinition.STANDARD,
                                STD,
                                DST),
                        rule(
                                Month.NOVEMBER,
                                1,
                                DayOfWeek.SUNDAY,
                                LocalTime.MIDNIGHT,
                                true,
                                TimeDefinition.STANDARD,
                                DST,
                                STD)));
        exact.put(
                "Test/Weeks", zone(start(Month.APRIL, 22, DayOfWeek.FRIDAY), end(Month.OCTOBER, 24, DayOfWeek.SUNDAY)));
        exact.put(
                "Test/BothDaylight",
                zone(
                        DST,
                        rule(Month.MARCH, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, DST, HIGH),
                        rule(Month.OCTOBER, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, HIGH, DST)));
        Map<String, ZoneRules> expanded = new LinkedHashMap<>();
        expanded.put(
                "Test/LateStart",
                zone(start(Month.MARCH, 29, DayOfWeek.SUNDAY), end(Month.OCTOBER, 1, DayOfWeek.SUNDAY)));
        expanded.put(
                "Test/LateEnd",
                zone(start(Month.MARCH, 1, DayOfWeek.SUNDAY), end(Month.OCTOBER, 29, DayOfWeek.SUNDAY)));
        expanded.put("Test/FarFromMarch", zone(start(Month.FEBRUARY, -10, null), end(Month.OCTOBER, 1, null)));
        expanded.put(
                "Test/ThreeRules",
                zone(
                        start(Month.MARCH, 1, DayOfWeek.SUNDAY),
                        rule(Month.JUNE, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, DST, HIGH),
                        rule(Month.OCTOBER, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, HIGH, STD)));
        Map<String, ZoneRules> restarted = new LinkedHashMap<>();
        restarted.put("Test/Restart", zone(DST, start(Month.MARCH, 21, null), end(Month.SEPTEMBER, 22, null)));
        restarted.put(
                "Test/RestartElsewhere", zone(HIGH, start(Month.MARCH, 21, null), end(Month.SEPTEMBER, 22, null)));
        expanded.put("Test/OneRule", zone(start(Month.MARCH, 1, null)));
        expanded.put("Test/OneRuleFromElsewhere", zone(HIGH, start(Month.MARCH, 1, null)));
        expanded.put(
                "Test/NotAPair",
                zone(
                        start(Month.MARCH, 1, null),
                        rule(Month.OCTOBER, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, DST, BASE)));
        expanded.put(
                "Test/NotAPairEither",
                zone(
                        start(Month.MARCH, 1, null),
                        rule(Month.OCTOBER, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, HIGH, STD)));
        ZoneRules noHistory = ZoneRules.of(
                STD, STD, List.of(), List.of(), List.of(start(Month.MARCH, 1, null), end(Month.OCTOBER, 1, null)));
        ZoneSource.Builder builder = ZoneSource.builder().rules("Test/NoHistory", noHistory);
        for (Map<String, ZoneRules> zones : List.of(exact, restarted, expanded)) {
            zones.forEach(builder::rules);
        }
        Libhegel lib = Engine.get();
        ZoneSource built = builder.build();
        long source = built.handle(lib);
        int[] years = {1970, 1971, 2024, 2399, 2400, 9998, 9999};
        exact.forEach((name, rules) -> {
            assertRoundTrips(lib, source, name, rules, Integer.MAX_VALUE);
            assertOffsetsAgree(lib, source, name, rules, 1, years);
            assertOffsetsAgree(lib, source, name, rules, 1, 10_000, 250_000);
        });
        restarted.forEach((name, rules) -> {
            assertOffsetsAgree(lib, source, name, rules, 1, years);
            assertOffsetsAgree(lib, source, name, rules, 1, 10_000, 250_000);
        });
        expanded.forEach((name, rules) -> assertOffsetsAgree(lib, source, name, rules, 1, years));
        for (String name : List.of("Test/LateStart", "Test/LateEnd", "Test/FarFromMarch", "Test/ThreeRules")) {
            assertRoundTrips(lib, source, name, expanded.get(name), 10_000);
        }
        assertOffsetsAgree(lib, source, "Test/NoHistory", noHistory, 1, 1970, 2030);
        assertEquals("", built.version());
    }

    @Test
    void wallClockTimesJavaTimeResolvesInconsistentlyAreRejected() {
        ZoneId zone = HostOnlyZone.oneRule();
        ZoneRules rules = HostOnlyZone.ONE_RULE;
        LocalDateTime newYear = LocalDateTime.of(2500, 1, 1, 0, 0);
        assertEquals(ZoneOffset.ofHours(2), rules.getOffset(newYear.minusHours(1)));
        assertEquals(
                ZoneOffset.ofHours(1), rules.getOffset(newYear.minusHours(1).toInstant(ZoneOffset.ofHours(2))));
        ZonedDateTimeGenerator around = Temporals.zonedDatetimes()
                .zones(just(zone))
                .min(newYear.minusDays(2))
                .max(newYear.plusDays(2));
        assertAllExamples(
                around,
                z -> z.toInstant().atZone(zone).equals(z)
                        && !z.toLocalDateTime().isBefore(newYear.minusDays(2))
                        && !z.toLocalDateTime().isAfter(newYear.plusDays(2)));
        assertTrue(findAny(around, z -> z.getOffset().equals(ZoneOffset.ofHours(1))) != null);
        assertTrue(findAny(around, z -> z.getOffset().equals(ZoneOffset.ofHours(2))) != null);
        FakeLibhegel fake = new FakeLibhegel();
        fake.knownZones = List.of(zone.getId());
        LocalDateTime lastHour = newYear.minusMinutes(30);
        fake.zonedValue = new Libhegel.ZonedDatetime(
                zone.getId(),
                lastHour,
                3600,
                true,
                false,
                Abi.TIMEZONE_KIND_UNIQUE,
                3600,
                3600,
                lastHour.toInstant(ZoneOffset.ofHours(1)));
        assertThrows(
                AssumeRejected.class,
                () -> new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false, Reporter.silent()).draw(around));
    }

    @Test
    void sourcesReportTheirVersions() {
        String first = new TreeSet<>(ZoneRulesProvider.getAvailableZoneIds()).first();
        assertEquals(
                ZoneRulesProvider.getVersions(first).lastKey(),
                ZoneSource.javaTime().version());
        assertEquals(Temporals.tzdbVersion(), ZoneSource.bundled().version());
        assertEquals("", ZoneSource.builder().build().version());
        assertEquals("lib-1.0", ZoneSource.builder().version("lib-1.0").build().version());
    }

    @Test
    void theDefaultIsJavaTimeUntilReplaced() {
        String host = HostOnlyZone.zone().getId();
        Libhegel lib = Engine.get();
        assertSame(ZoneSource.javaTime(), ZoneSource.getDefault());
        assertTrue(BindingEdges.info(lib, 0, host).known());
        ZoneSource previous = ZoneSource.setDefault(ZoneSource.bundled());
        try {
            assertSame(ZoneSource.javaTime(), previous);
            assertSame(ZoneSource.bundled(), ZoneSource.getDefault());
            assertFalse(BindingEdges.info(lib, 0, host).known());
            assertAllExamples(Temporals.zones(), z -> !z.getId().equals(host));
        } finally {
            assertSame(ZoneSource.bundled(), ZoneSource.setDefault(previous));
        }
        assertSame(ZoneSource.javaTime(), ZoneSource.getDefault());
        assertTrue(BindingEdges.info(lib, 0, host).known());
    }

    @Test
    void aGeneratorKeepsTheDefaultItWasCreatedUnder() {
        ZoneId host = HostOnlyZone.zone();
        ZoneGenerator zones = Temporals.zones();
        ZonedDateTimeGenerator zoned = Temporals.zonedDatetimes();
        ZonedDateTimeGenerator wrapped = Temporals.zonedDatetimes().zones(just(host));
        ZonedDateTimeGenerator fromDatetimes = Temporals.datetimes().zones(just(host));
        ZoneSource previous = ZoneSource.setDefault(ZoneSource.bundled());
        try {
            assertAllExamples(zones.names(host.getId()), z -> z.equals(host));
            FakeLibhegel fake = new FakeLibhegel();
            TestCase tc = new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false, Reporter.silent());
            tc.draw(Temporals.zonedDatetimes());
            assertEquals(List.of("America/New_York", "Asia/Calcutta", "Europe/London", "UTC"), fake.timezoneNames);
            fake.timezoneNames = null;
            tc.draw(zoned.precision(ChronoUnit.SECONDS));
            assertNull(fake.timezoneNames);
            assertAllExamples(wrapped.min(FOLD).max(FOLD), z -> z.getZone().equals(host));
            assertAllExamples(
                    fromDatetimes.min(FOLD).max(FOLD), z -> z.getZone().equals(host));
            for (Generator<ZonedDateTime> now : List.<Generator<ZonedDateTime>>of(
                    Temporals.zonedDatetimes().zones(just(host)),
                    Temporals.datetimes().zones(just(host)))) {
                assertThrows(IllegalArgumentException.class, () -> assertAllExamples(now, z -> true));
            }
            assertAllExamples(Temporals.zonedDatetimes(), z -> !z.getZone().equals(host));
        } finally {
            ZoneSource.setDefault(previous);
        }
    }

    @Test
    void aGeneratorDrawsFromTheSourceItIsGiven() {
        String host = HostOnlyZone.zone().getId();
        ZoneSource custom = ZoneSource.builder()
                .rules(host, NEW_YORK)
                .tzif("UTC", BindingEdges.utcTzif())
                .link("Etc/UTC", "UTC")
                .version("custom-1")
                .build();
        assertAllExamples(
                Temporals.zones().source(custom),
                z -> z.getId().equals(host)
                        || z.getId().equals("UTC")
                        || z.getId().equals("Etc/UTC"));
        assertAllExamples(
                Temporals.zonedDatetimes().source(custom),
                z -> z.getZone().getId().equals(host)
                        || z.getZone().getId().equals("UTC")
                        || z.getZone().getId().equals("Etc/UTC"));
        for (ZonedDateTimeGenerator fold : List.of(
                Temporals.zonedDatetimes()
                        .zones(Temporals.zones().source(custom).names(host))
                        .min(FOLD)
                        .max(FOLD),
                Temporals.zonedDatetimes()
                        .zones(Temporals.zones()
                                .source(custom)
                                .filter(z -> z.getId().equals(host)))
                        .source(custom)
                        .min(FOLD)
                        .max(FOLD))) {
            assertAllExamples(fold, z -> z.getZone().getId().equals(host));
            assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4))) != null);
            assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-5))) != null);
        }
        assertAllExamples(
                Temporals.zonedDatetimes()
                        .zones(Temporals.zones().source(custom).names("Etc/UTC"))
                        .min(FOLD)
                        .max(FOLD),
                z -> z.getOffset().equals(ZoneOffset.UTC));
    }

    @Test
    void aLoaderSuppliesZonesAsTheyAreDrawn() {
        String host = HostOnlyZone.zone().getId();
        AtomicInteger loads = new AtomicInteger();
        ZoneSource lazy = ZoneSource.builder()
                .declare(host)
                .declare(List.of("Europe/London"))
                .loader((name, zones) -> {
                    loads.incrementAndGet();
                    if (name.equals(host)) {
                        zones.rules(name, NEW_YORK);
                    }
                })
                .build();
        assertAllExamples(
                Temporals.zonedDatetimes()
                        .zones(Temporals.zones().source(lazy).names(host))
                        .min(FOLD)
                        .max(FOLD),
                z -> z.getZone().getId().equals(host));
        assertTrue(loads.get() >= 1);
        assertFalse(BindingEdges.info(Engine.get(), lazy.handle(Engine.get()), "Mars/Olympus_Mons")
                .known());
        IllegalArgumentException undeclared = assertThrows(
                IllegalArgumentException.class,
                () -> assertAllExamples(
                        Temporals.zonedDatetimes()
                                .zones(just(ZoneId.of("Europe/Paris")))
                                .source(lazy),
                        z -> true));
        assertTrue(undeclared.getMessage().contains("\"Europe/Paris\""), undeclared.getMessage());
        assertTrue(undeclared.getMessage().contains("source(...)"), undeclared.getMessage());

        ZoneSource broken = ZoneSource.builder()
                .declare(host)
                .loader((name, zones) -> {
                    throw new IllegalStateException("cannot load " + name);
                })
                .build();
        IllegalArgumentException failure = assertThrows(
                IllegalArgumentException.class,
                () -> Temporals.zones().names(host).source(broken));
        assertTrue(failure.getMessage().contains("cannot load " + host), failure.getMessage());
        assertEquals(IllegalStateException.class, failure.getCause().getClass());
        IllegalArgumentException byName = assertThrows(
                IllegalArgumentException.class,
                () -> assertAllExamples(
                        Temporals.zonedDatetimes().zones(just(ZoneId.of(host))).source(broken), z -> true));
        assertEquals("cannot load " + host, byName.getCause().getMessage());
    }

    @Test
    void theSourceOfAWrappedZonesGeneratorIsGivenToTheZonedGenerator() {
        ZoneId host = HostOnlyZone.zone();
        AtomicInteger loads = new AtomicInteger();
        ZoneSource.Loader loader = (name, zones) -> {
            if (name.equals(host.getId())) {
                loads.incrementAndGet();
                zones.rules(name, NEW_YORK);
            }
        };
        ZoneSource counting =
                ZoneSource.builder().declare(host.getId()).loader(loader).build();
        Generator<ZoneId> wrapped = Temporals.zones().source(counting).map(z -> z);
        assertAllExamples(
                Temporals.zonedDatetimes().zones(wrapped).min(FOLD).max(FOLD),
                z -> z.getZone().equals(host));
        int byTheZonesGenerator = loads.get();
        assertAllExamples(
                Temporals.zonedDatetimes().zones(just(host)).min(FOLD).max(FOLD),
                z -> z.getZone().equals(host));
        assertEquals(byTheZonesGenerator, loads.get());
        ZoneSource asked =
                ZoneSource.builder().declare(host.getId()).loader(loader).build();
        assertAllExamples(
                Temporals.zonedDatetimes()
                        .zones(just(host))
                        .source(asked)
                        .min(FOLD)
                        .max(FOLD),
                z -> z.getZone().equals(host));
        assertTrue(loads.get() > byTheZonesGenerator);
    }

    @Test
    void aSourceThatDiffersFromThisJvmAimsTheDrawAndJavaTimeResolvesIt() {
        ZoneId host = HostOnlyZone.zone();
        ZoneRules london = ZoneId.of("Europe/London").getRules();
        ZoneSource differing = ZoneSource.builder().rules(host.getId(), london).build();
        for (ZonedDateTimeGenerator fold : List.of(
                Temporals.zonedDatetimes()
                        .zones(Temporals.zones().source(differing))
                        .min(FOLD)
                        .max(FOLD),
                Temporals.zonedDatetimes()
                        .zones(just(host))
                        .source(differing)
                        .min(FOLD)
                        .max(FOLD))) {
            assertAllExamples(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4)));
            assertAllExamples(
                    fold.ambiguous(Ambiguous.EARLIER), z -> z.getOffset().equals(ZoneOffset.ofHours(-4)));
            assertAllExamples(
                    fold.ambiguous(Ambiguous.LATER), z -> z.getOffset().equals(ZoneOffset.ofHours(-5)));
        }
        Instant from = Instant.parse("2024-07-01T00:00:00Z");
        Instant to = Instant.parse("2024-07-08T00:00:00Z");
        ZonedDateTimeGenerator week =
                Temporals.zonedDatetimes().source(differing).min(from).max(to);
        assertAllExamples(
                week, z -> !z.toInstant().isBefore(from) && !z.toInstant().isAfter(to));
        assertTrue(findAny(week, z -> z.getZone().equals(host)) != null);
    }

    @Test
    void malformedDataFailsTheBuild() {
        ZoneSource.Builder malformed = ZoneSource.builder().tzif("UTC", new byte[3]);
        IllegalArgumentException failure = assertThrows(IllegalArgumentException.class, malformed::build);
        assertTrue(failure.getMessage().contains("malformed TZif"), failure.getMessage());
    }

    private static WeakReference<ZoneSource.Loader> loaderOfADroppedSource() {
        AtomicInteger loads = new AtomicInteger();
        ZoneSource.Loader loader = (name, zones) -> loads.incrementAndGet();
        ZoneSource source = ZoneSource.builder().loader(loader).build();
        assertFalse(BindingEdges.info(Engine.get(), source.handle(Engine.get()), "Test/Lazy")
                .known());
        assertEquals(1, loads.get());
        return new WeakReference<>(loader);
    }

    @Test
    void aSourceNothingRefersToReleasesItsLoader() throws InterruptedException {
        WeakReference<ZoneSource.Loader> loader = loaderOfADroppedSource();
        for (int i = 0; i < 500 && loader.get() != null; i++) {
            System.gc();
            Thread.sleep(10);
        }
        assertNull(loader.get());
    }

    @Test
    void oraclesAreProbedIntoZones() {
        ZoneRules london = ZoneId.of("Europe/London").getRules();
        ZoneSource.Oracle sampled = ZoneSource.Oracle.of(
                instant -> new ZoneSource.LocalType(ZoneOffset.UTC, false, "UTC"), 2000, 2001, ZoneSource.Beyond.HOLD);
        ZoneSource.Oracle stepped = ZoneSource.Oracle.of(
                        instant -> new ZoneSource.LocalType(
                                london.getOffset(instant), london.isDaylightSavings(instant), "LON"),
                        2020,
                        2022,
                        ZoneSource.Beyond.INFER)
                .nextTransition(after ->
                        Optional.ofNullable(london.nextTransition(after)).map(ZoneOffsetTransition::getInstant));
        ZoneSource probed = ZoneSource.builder()
                .oracle("UTC", sampled)
                .oracle("Europe/London", stepped)
                .build();
        Libhegel lib = Engine.get();
        long source = probed.handle(lib);
        assertEquals(
                0,
                BindingEdges.offsetAt(lib, source, "UTC", Instant.parse("2050-07-01T00:00:00Z"))
                        .offsetSeconds());
        for (int year : new int[] {2021, 2030, 2100}) {
            Instant summer = LocalDateTime.of(year, 7, 1, 0, 0).toInstant(ZoneOffset.UTC);
            Libhegel.TimezoneOffset offset = BindingEdges.offsetAt(lib, source, "Europe/London", summer);
            assertEquals(3600, offset.offsetSeconds());
            assertEquals("LON", offset.abbreviation());
        }
        assertAllExamples(
                Temporals.zones().source(probed),
                z -> z.getId().equals("UTC") || z.getId().equals("Europe/London"));
    }

    @Test
    void aSourceIsRebuiltForEachBinding() {
        ZoneSource source = ZoneSource.builder().version("rebuilt").build();
        FakeLibhegel fake = new FakeLibhegel();
        long real = source.handle(Engine.get());
        assertEquals(real, source.handle(Engine.get()));
        assertEquals(FakeLibhegel.ZONE_SOURCE, source.handle(fake));
        assertEquals(FakeLibhegel.ZONE_SOURCE, source.handle(fake));
        assertTrue(source.handle(Engine.get()) != 0);
    }

    @Test
    void zonesFromAnyGeneratorResolveAgainstTheDefault() {
        String host = HostOnlyZone.zone().getId();
        assertAllExamples(
                Temporals.zonedDatetimes()
                        .zones(just(ZoneId.of(host)))
                        .min(FOLD)
                        .max(FOLD),
                z -> z.getZone().getId().equals(host));
        FakeLibhegel fake = new FakeLibhegel();
        new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false, Reporter.silent()).draw(Temporals.zones());
        assertEquals(FakeLibhegel.ZONE_SOURCE, fake.timezoneSource);
    }
}
