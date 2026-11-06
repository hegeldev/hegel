package dev.hegel;

import static dev.hegel.Generators.just;
import static dev.hegel.Utils.assertAllExamples;
import static dev.hegel.Utils.findAny;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.Generators.Temporal;
import dev.hegel.generators.temporal.ZoneSource;
import dev.hegel.generators.temporal.ZonedDateTimeGenerator;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.DayOfWeek;
import java.time.Instant;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.Month;
import java.time.ZoneId;
import java.time.ZoneOffset;
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

    @Test
    void everyJavaTimeZoneRoundTripsThroughTheEngine() {
        Libhegel lib = Engine.get();
        long source = ZoneSource.javaTime().handle(lib);
        for (String name : new TreeSet<>(ZoneRulesProvider.getAvailableZoneIds())) {
            assertRoundTrips(lib, source, name, ZoneRulesProvider.getRules(name, false), Integer.MAX_VALUE);
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
        Map<String, ZoneRules> loaded = new LinkedHashMap<>();
        loaded.put(
                "Test/NoHistory",
                ZoneRules.of(
                        STD,
                        STD,
                        List.of(),
                        List.of(),
                        List.of(start(Month.MARCH, 1, null), end(Month.OCTOBER, 1, null))));
        loaded.put("Test/OneRule", zone(start(Month.MARCH, 1, null)));
        loaded.put(
                "Test/NotAPair",
                zone(
                        start(Month.MARCH, 1, null),
                        rule(Month.OCTOBER, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, DST, BASE)));
        loaded.put(
                "Test/NotAPairEither",
                zone(
                        start(Month.MARCH, 1, null),
                        rule(Month.OCTOBER, 1, null, LocalTime.of(2, 0), false, TimeDefinition.WALL, HIGH, STD)));
        ZoneSource.Builder builder = ZoneSource.builder();
        for (Map<String, ZoneRules> zones : List.of(exact, expanded, loaded)) {
            zones.forEach(builder::rules);
        }
        Libhegel lib = Engine.get();
        long source = builder.build().handle(lib);
        exact.forEach((name, rules) -> assertRoundTrips(lib, source, name, rules, Integer.MAX_VALUE));
        expanded.forEach((name, rules) -> assertRoundTrips(lib, source, name, rules, 2399));
        Instant mid = Instant.parse("1970-01-15T00:00:00Z");
        loaded.forEach((name, rules) -> assertEquals(
                rules.getOffset(mid).getTotalSeconds(),
                BindingEdges.offsetAt(lib, source, name, mid).offsetSeconds(),
                name));
        assertEquals(
                3600,
                BindingEdges.offsetAt(lib, source, "Test/NoHistory", Instant.parse("2030-07-01T00:00:00Z"))
                        .offsetSeconds());
    }

    @Test
    void sourcesReportTheirVersions() {
        String first = new TreeSet<>(ZoneRulesProvider.getAvailableZoneIds()).first();
        assertEquals(
                ZoneRulesProvider.getVersions(first).lastKey(),
                ZoneSource.javaTime().version());
        assertEquals(Temporal.tzdbVersion(), ZoneSource.bundled().version());
        assertEquals("", ZoneSource.builder().build().version());
        assertEquals("lib-1.0", ZoneSource.builder().version("lib-1.0").build().version());
    }

    @Test
    void theDefaultIsJavaTimeUntilReplaced() {
        String host = HostOnlyZone.zone().getId();
        Libhegel lib = Engine.get();
        assertSame(ZoneSource.javaTime(), ZoneSource.getDefault());
        assertTrue(BindingEdges.info(lib, 0, host).known());
        ZoneSource.setDefault(ZoneSource.bundled());
        try {
            assertSame(ZoneSource.bundled(), ZoneSource.getDefault());
            assertFalse(BindingEdges.info(lib, 0, host).known());
            assertAllExamples(Temporal.zones(), z -> !z.getId().equals(host));
        } finally {
            ZoneSource.setDefault(ZoneSource.javaTime());
        }
        assertTrue(BindingEdges.info(lib, 0, host).known());
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
                Temporal.zones().source(custom),
                z -> z.getId().equals(host)
                        || z.getId().equals("UTC")
                        || z.getId().equals("Etc/UTC"));
        ZonedDateTimeGenerator fold = Temporal.zonedDateTimes()
                .zones(Temporal.zones().source(custom).names(host))
                .min(FOLD)
                .max(FOLD);
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-4))) != null);
        assertTrue(findAny(fold, z -> z.getOffset().equals(ZoneOffset.ofHours(-5))) != null);
        assertAllExamples(
                Temporal.zonedDateTimes()
                        .zones(Temporal.zones().source(custom).names("Etc/UTC"))
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
                Temporal.zonedDateTimes()
                        .zones(Temporal.zones().source(lazy).names(host))
                        .min(FOLD)
                        .max(FOLD),
                z -> z.getZone().getId().equals(host));
        assertTrue(loads.get() >= 1);
        assertFalse(BindingEdges.info(Engine.get(), lazy.handle(Engine.get()), "Mars/Olympus_Mons")
                .known());

        ZoneSource broken = ZoneSource.builder()
                .declare(host)
                .loader((name, zones) -> {
                    throw new IllegalStateException("cannot load " + name);
                })
                .build();
        IllegalArgumentException failure = assertThrows(
                IllegalArgumentException.class,
                () -> assertAllExamples(Temporal.zones().source(broken).names(host), z -> true));
        assertTrue(failure.getMessage().contains("cannot load " + host), failure.getMessage());
    }

    @Test
    void malformedDataFailsTheFirstDraw() {
        ZoneSource malformed = ZoneSource.builder().tzif("UTC", new byte[3]).build();
        assertThrows(
                IllegalArgumentException.class,
                () -> assertAllExamples(Temporal.zones().source(malformed), z -> true));
        assertThrows(IllegalArgumentException.class, () -> malformed.handle(Engine.get()));
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
                Temporal.zones().source(probed),
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
        assertEquals(1, fake.freedZoneSources);
    }

    @Test
    void zonesFromAnyGeneratorResolveAgainstTheDefault() {
        String host = HostOnlyZone.zone().getId();
        assertAllExamples(
                Temporal.zonedDateTimes().zones(just(ZoneId.of(host))).min(FOLD).max(FOLD),
                z -> z.getZone().getId().equals(host));
        FakeLibhegel fake = new FakeLibhegel();
        new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false, Reporter.silent()).draw(Temporal.zones());
        assertEquals(FakeLibhegel.ZONE_SOURCE, fake.timezoneSource);
    }
}
