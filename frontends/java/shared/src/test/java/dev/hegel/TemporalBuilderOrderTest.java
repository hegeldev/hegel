package dev.hegel;

import static dev.hegel.Generators.just;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.Generators.Temporals;
import dev.hegel.generators.temporal.Ambiguous;
import dev.hegel.generators.temporal.OffsetGranularity;
import dev.hegel.generators.temporal.ZoneSource;
import java.time.DayOfWeek;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.Month;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.temporal.ChronoUnit;
import java.time.zone.ZoneRules;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Supplier;
import java.util.function.UnaryOperator;
import org.junit.jupiter.api.Test;

/**
 * The fluent methods of the date and time generators can be called in any order: one set of calls
 * makes one generator, which works or fails the same way however the calls were ordered. Settings
 * that were given and cannot work together are rejected by the call that gives the last of them; a
 * setting that only conflicts with a default, which a later call can still replace, is reported by
 * the first draw.
 */
class TemporalBuilderOrderTest {
    private static final ZoneId NEW_YORK = ZoneId.of("America/New_York");
    private static final ZoneRules NEW_YORK_RULES = NEW_YORK.getRules();
    private static final String HOST = HostOnlyZone.zone().getId();
    private static final String BUILD = "build";
    private static final String DRAW = "draw";

    /** Where a chain of calls ended up: a generator that draws, or the exception and the stage it came from. */
    private record Outcome(String stage, Class<?> type, String message) {
        static final Outcome WORKS = new Outcome("works", null, null);

        void failsAtTheFirstDraw(String fragment) {
            fails(DRAW, fragment);
        }

        void failsAtACall(String fragment) {
            fails(BUILD, fragment);
        }

        private void fails(String at, String fragment) {
            assertEquals(at, stage, toString());
            assertEquals(IllegalArgumentException.class, type, toString());
            assertTrue(message.contains(fragment), message);
        }
    }

    private static <G extends Generator<?>> Outcome outcome(Supplier<G> start, List<UnaryOperator<G>> calls) {
        G built;
        try {
            built = start.get();
            for (UnaryOperator<G> call : calls) {
                built = call.apply(built);
            }
        } catch (RuntimeException e) {
            return new Outcome(BUILD, e.getClass(), e.getMessage());
        }
        Generator<?> gen = built;
        AtomicInteger drawn = new AtomicInteger();
        try {
            Hegel.run(
                            tc -> {
                                tc.draw(gen);
                                drawn.incrementAndGet();
                            },
                            new Settings().database(Database.disabled()).testCases(20),
                            Reporter.silent())
                    .throwIfFailed();
        } catch (RuntimeException e) {
            assertEquals(0, drawn.get(), e.toString());
            return new Outcome(DRAW, e.getClass(), e.getMessage());
        }
        assertEquals(20, drawn.get());
        return Outcome.WORKS;
    }

    private static <T> List<List<T>> orders(List<T> calls) {
        if (calls.size() <= 1) {
            return List.of(calls);
        }
        List<List<T>> all = new ArrayList<>();
        for (int i = 0; i < calls.size(); i++) {
            List<T> rest = new ArrayList<>(calls);
            T first = rest.remove(i);
            for (List<T> tail : orders(rest)) {
                List<T> order = new ArrayList<>(List.of(first));
                order.addAll(tail);
                all.add(order);
            }
        }
        return all;
    }

    private static <G extends Generator<?>> Outcome same(Supplier<G> start, List<List<UnaryOperator<G>>> orders) {
        Outcome first = outcome(start, orders.get(0));
        for (List<UnaryOperator<G>> order : orders.subList(1, orders.size())) {
            assertEquals(first, outcome(start, order));
        }
        return first;
    }

    /** The one outcome of {@code calls} made on {@code start} in every order. */
    @SafeVarargs
    private static <G extends Generator<?>> Outcome inAnyOrder(Supplier<G> start, UnaryOperator<G>... calls) {
        return same(start, orders(List.of(calls)));
    }

    // --- zones ---

    @Test
    void zoneNamesMayComeBeforeTheSourceThatHasThem() {
        ZoneSource custom = ZoneSource.builder().rules(HOST, NEW_YORK_RULES).build();
        ZoneSource previous = ZoneSource.setDefault(ZoneSource.bundled());
        try {
            assertEquals(Outcome.WORKS, inAnyOrder(Temporals::zones, g -> g.names(HOST), g -> g.source(custom)));
            assertEquals(
                    Outcome.WORKS,
                    inAnyOrder(
                            Temporals::zones,
                            g -> g.names(List.of(HOST)),
                            g -> g.source(custom),
                            g -> g.allowLinks(false),
                            g -> g.allowLegacy(true)));
            inAnyOrder(Temporals::zones, g -> g.names(HOST)).failsAtTheFirstDraw("unknown time zone \"" + HOST + "\"");
            inAnyOrder(Temporals::zones, g -> g.names(HOST), g -> g.allowLinks(false))
                    .failsAtTheFirstDraw("unknown time zone \"" + HOST + "\"");
        } finally {
            ZoneSource.setDefault(previous);
        }
    }

    @Test
    void aZoneNameTheGivenSourceLacksFailsWhicheverCallComesSecond() {
        inAnyOrder(Temporals::zones, g -> g.names(HOST), g -> g.source(ZoneSource.bundled()))
                .failsAtACall("unknown time zone \"" + HOST + "\"");
        inAnyOrder(Temporals::zones, g -> g.names(HOST), g -> g.source(ZoneSource.bundled()), g -> g.allowLegacy(true))
                .failsAtACall("unknown time zone \"" + HOST + "\"");
    }

    @Test
    void zoneFlagsAndNamesMayComeAfterTheSourceTheyLeaveSomethingOf() {
        ZoneSource legacyOnly =
                ZoneSource.builder().rules("EST5EDT", NEW_YORK_RULES).build();
        assertEquals(Outcome.WORKS, inAnyOrder(Temporals::zones, g -> g.source(legacyOnly), g -> g.allowLegacy(true)));
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::zones,
                        g -> g.source(legacyOnly),
                        g -> g.allowLegacy(false),
                        g -> g.names("EST5EDT")));
        inAnyOrder(Temporals::zones, g -> g.source(legacyOnly)).failsAtTheFirstDraw("no zone this JVM knows");
        inAnyOrder(Temporals::zones, g -> g.source(legacyOnly), g -> g.allowLinks(false))
                .failsAtTheFirstDraw("no zone this JVM knows");
    }

    // --- dates ---

    @Test
    void dateBoundsBeyondTheDefaultRangeMayBeSetInEitherOrder() {
        LocalDate far = LocalDate.of(12_000, 1, 1);
        LocalDate early = LocalDate.of(-5, 1, 1);
        assertEquals(Outcome.WORKS, inAnyOrder(Temporals::dates, g -> g.min(far), g -> g.max(far.plusYears(1))));
        assertEquals(Outcome.WORKS, inAnyOrder(Temporals::dates, g -> g.max(early), g -> g.min(early.minusYears(1))));
        inAnyOrder(Temporals::dates, g -> g.min(far))
                .failsAtTheFirstDraw(
                        "dates: min (+12000-01-01) > max (9999-12-31); max was not set, so it is the default");
        inAnyOrder(Temporals::dates, g -> g.max(early))
                .failsAtTheFirstDraw(
                        "dates: min (0001-01-01) > max (-0005-01-01); min was not set, so it is the default");
    }

    @Test
    void dateBoundsInTheWrongOrderFailWhicheverCallComesSecond() {
        LocalDate far = LocalDate.of(12_000, 1, 1);
        LocalDate near = LocalDate.of(2024, 1, 1);
        inAnyOrder(Temporals::dates, g -> g.min(far.plusYears(1)), g -> g.max(far))
                .failsAtACall("dates: min (+12001-01-01) > max (+12000-01-01)");
        inAnyOrder(Temporals::dates, g -> g.min(near.plusDays(1)), g -> g.max(near), g -> g.months(Month.JANUARY))
                .failsAtACall("dates: min (2024-01-02) > max (2024-01-01)");
    }

    @Test
    void dateOptionsAndBoundsMayBeSetInAnyOrder() {
        LocalDate monday = LocalDate.of(2024, 1, 1);
        LocalDate saturday = LocalDate.of(2024, 1, 6);
        LocalDate lastDefault = LocalDate.of(9999, 12, 31);
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::dates,
                        g -> g.min(monday),
                        g -> g.max(saturday.plusDays(60)),
                        g -> g.weekdays(DayOfWeek.SUNDAY),
                        g -> g.months(Month.FEBRUARY)));
        inAnyOrder(Temporals::dates, g -> g.min(monday), g -> g.max(saturday), g -> g.weekdays(DayOfWeek.SUNDAY))
                .failsAtACall("allowed weekday and month");
        inAnyOrder(Temporals::dates, g -> g.min(monday), g -> g.max(saturday), g -> g.months(Month.MARCH))
                .failsAtACall("allowed weekday and month");
        inAnyOrder(Temporals::dates, g -> g.min(lastDefault), g -> g.weekdays(DayOfWeek.SUNDAY))
                .failsAtTheFirstDraw("allowed weekday and month");
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::dates,
                        g -> g.min(lastDefault),
                        g -> g.weekdays(DayOfWeek.SUNDAY),
                        g -> g.max(lastDefault.plusDays(7))));
    }

    // --- times, datetimes, instants, durations, offsets ---

    @Test
    void timeBoundsAndPrecisionMayBeSetInAnyOrder() {
        LocalTime late = LocalTime.of(23, 30);
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::times,
                        g -> g.min(late.minusHours(1)),
                        g -> g.max(late),
                        g -> g.precision(ChronoUnit.HOURS)));
        inAnyOrder(Temporals::times, g -> g.min(late), g -> g.precision(ChronoUnit.HOURS))
                .failsAtACall("no ");
        inAnyOrder(Temporals::times, g -> g.min(late), g -> g.max(late.minusMinutes(10)))
                .failsAtACall("times: min (23:30) > max (23:20)");
    }

    @Test
    void datetimeBoundsAndPrecisionMayBeSetInAnyOrder() {
        LocalDateTime far = LocalDateTime.of(12_000, 1, 1, 0, 0);
        LocalDateTime early = LocalDateTime.of(-5, 1, 1, 0, 0);
        LocalDateTime odd = LocalDateTime.of(2024, 1, 1, 12, 0, 30);
        LocalDateTime lastHalfHour = LocalDateTime.of(9999, 12, 31, 23, 30);
        assertEquals(Outcome.WORKS, inAnyOrder(Temporals::datetimes, g -> g.min(far), g -> g.max(far.plusYears(1))));
        assertEquals(
                Outcome.WORKS, inAnyOrder(Temporals::datetimes, g -> g.max(early), g -> g.min(early.minusYears(1))));
        inAnyOrder(Temporals::datetimes, g -> g.min(far)).failsAtTheFirstDraw("max was not set, so it is the default");
        inAnyOrder(Temporals::datetimes, g -> g.max(early))
                .failsAtTheFirstDraw("min was not set, so it is the default");
        inAnyOrder(Temporals::datetimes, g -> g.min(far.plusYears(1)), g -> g.max(far))
                .failsAtACall("datetimes: min (+12001-01-01T00:00) > max (+12000-01-01T00:00)");
        inAnyOrder(Temporals::datetimes, g -> g.min(odd), g -> g.max(odd), g -> g.precision(ChronoUnit.MINUTES))
                .failsAtACall("no ");
        inAnyOrder(Temporals::datetimes, g -> g.min(lastHalfHour), g -> g.precision(ChronoUnit.HOURS))
                .failsAtTheFirstDraw("no ");
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::datetimes,
                        g -> g.min(lastHalfHour),
                        g -> g.precision(ChronoUnit.HOURS),
                        g -> g.max(lastHalfHour.plusHours(1))));
    }

    @Test
    void instantBoundsAndPrecisionMayBeSetInAnyOrder() {
        Instant far = LocalDateTime.of(12_000, 1, 1, 0, 0).toInstant(ZoneOffset.UTC);
        Instant early = LocalDateTime.of(-5, 1, 1, 0, 0).toInstant(ZoneOffset.UTC);
        Instant odd = Instant.parse("2024-01-01T12:00:30Z");
        Instant lastHalfHour = Instant.parse("9999-12-31T23:30:00Z");
        assertEquals(Outcome.WORKS, inAnyOrder(Temporals::instants, g -> g.min(far), g -> g.max(far.plusSeconds(60))));
        assertEquals(
                Outcome.WORKS, inAnyOrder(Temporals::instants, g -> g.max(early), g -> g.min(early.minusSeconds(60))));
        inAnyOrder(Temporals::instants, g -> g.min(far)).failsAtTheFirstDraw("max was not set, so it is the default");
        inAnyOrder(Temporals::instants, g -> g.max(early)).failsAtTheFirstDraw("min was not set, so it is the default");
        inAnyOrder(Temporals::instants, g -> g.min(far.plusSeconds(1)), g -> g.max(far))
                .failsAtACall("instants: min (+12000-01-01T00:00:01Z) > max (+12000-01-01T00:00:00Z)");
        inAnyOrder(Temporals::instants, g -> g.min(odd), g -> g.max(odd), g -> g.precision(ChronoUnit.MINUTES))
                .failsAtACall("no ");
        inAnyOrder(Temporals::instants, g -> g.min(lastHalfHour), g -> g.precision(ChronoUnit.HOURS))
                .failsAtTheFirstDraw("no ");
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::instants,
                        g -> g.min(lastHalfHour),
                        g -> g.precision(ChronoUnit.HOURS),
                        g -> g.max(lastHalfHour.plusSeconds(3600))));
    }

    @Test
    void durationBoundsAndPrecisionMayBeSetInAnyOrder() {
        Duration odd = Duration.ofSeconds(30);
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::durations,
                        g -> g.min(odd),
                        g -> g.max(odd.plusSeconds(30)),
                        g -> g.precision(ChronoUnit.MINUTES)));
        inAnyOrder(Temporals::durations, g -> g.min(odd), g -> g.max(odd), g -> g.precision(ChronoUnit.MINUTES))
                .failsAtACall("no ");
        inAnyOrder(Temporals::durations, g -> g.min(odd), g -> g.max(odd.minusSeconds(1)))
                .failsAtACall("durations: min (PT30S) > max (PT29S)");
    }

    @Test
    void offsetBoundsAndGranularityMayBeSetInAnyOrder() {
        ZoneOffset odd = ZoneOffset.ofTotalSeconds(30);
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::offsets,
                        g -> g.min(odd),
                        g -> g.max(ZoneOffset.ofTotalSeconds(60)),
                        g -> g.granularity(OffsetGranularity.MINUTES)));
        inAnyOrder(Temporals::offsets, g -> g.min(odd), g -> g.max(odd), g -> g.granularity(OffsetGranularity.MINUTES))
                .failsAtACall("no ");
        inAnyOrder(Temporals::offsets, g -> g.min(odd), g -> g.max(ZoneOffset.ofTotalSeconds(29)))
                .failsAtACall("offsets: min (30s) > max (29s)");
    }

    // --- spans ---

    @Test
    void spanUnitsAndSignMayBeSetInAnyOrder() {
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(Temporals::spans, g -> g.days(1, 30), g -> g.months(0, 11), g -> g.nonNegative(true)));
        inAnyOrder(Temporals::spans, g -> g.months(-3, -1), g -> g.nonNegative(true))
                .failsAtACall("requires min >= 0 for months, got -3");
        inAnyOrder(Temporals::spans, g -> g.days(1, 30), g -> g.months(-3, -1), g -> g.nonNegative(true))
                .failsAtACall("requires min >= 0 for months, got -3");
    }

    // --- zoned datetimes ---

    @Test
    void zonedBoundsBeyondTheDefaultRangeMayBeSetInEitherOrder() {
        LocalDateTime far = LocalDateTime.of(12_000, 1, 1, 0, 0);
        Instant farInstant = far.toInstant(ZoneOffset.UTC);
        assertEquals(
                Outcome.WORKS, inAnyOrder(Temporals::zonedDatetimes, g -> g.min(far), g -> g.max(far.plusYears(1))));
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(Temporals::zonedDatetimes, g -> g.min(farInstant), g -> g.max(farInstant.plusSeconds(60))));
        inAnyOrder(Temporals::zonedDatetimes, g -> g.min(far))
                .failsAtTheFirstDraw("max was not set, so it is the default");
        inAnyOrder(Temporals::zonedDatetimes, g -> g.min(farInstant))
                .failsAtTheFirstDraw("max was not set, so it is the default");
        inAnyOrder(Temporals::zonedDatetimes, g -> g.min(far.plusYears(1)), g -> g.max(far))
                .failsAtACall("zonedDatetimes: min (+12001-01-01T00:00) > max (+12000-01-01T00:00)");
        inAnyOrder(Temporals::zonedDatetimes, g -> g.min(farInstant.plusSeconds(1)), g -> g.max(farInstant))
                .failsAtACall("zonedDatetimes: min (+12000-01-01T00:00:01Z) > max (+12000-01-01T00:00:00Z)");
    }

    @Test
    void zonedZonesAndSourceMayBeSetInAnyOrder() {
        ZoneSource custom = ZoneSource.builder().rules(HOST, NEW_YORK_RULES).build();
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::zonedDatetimes,
                        g -> g.zones(just(ZoneId.of(HOST))),
                        g -> g.source(custom),
                        g -> g.precision(ChronoUnit.SECONDS)));
        inAnyOrder(Temporals::zonedDatetimes, g -> g.zones(Temporals.zones()), g -> g.source(ZoneSource.bundled()))
                .failsAtACall("zones().source(...)");
        ZoneSource previous = ZoneSource.setDefault(ZoneSource.bundled());
        try {
            inAnyOrder(
                            Temporals::zonedDatetimes,
                            g -> g.zones(Temporals.zones().names(HOST)),
                            g -> g.ambiguous(Ambiguous.LATER))
                    .failsAtTheFirstDraw("unknown time zone \"" + HOST + "\"");
        } finally {
            ZoneSource.setDefault(previous);
        }
    }

    @Test
    void zonedBoundsOfBothKindsFailWhicheverCallComesSecond() {
        LocalDateTime civil = LocalDateTime.of(2020, 1, 1, 0, 0);
        inAnyOrder(Temporals::zonedDatetimes, g -> g.min(civil), g -> g.max(Instant.EPOCH))
                .failsAtACall("not both");
        inAnyOrder(
                        Temporals::zonedDatetimes,
                        g -> g.zones(just(ZoneOffset.UTC)),
                        g -> g.max(civil),
                        g -> g.min(civil.atZone(NEW_YORK)))
                .failsAtACall("not both");
        inAnyOrder(() -> Temporals.datetimes().min(civil).zones(Temporals.zones()), g -> g.max(Instant.EPOCH))
                .failsAtACall("not both");
    }

    @Test
    void zonedBoundsAndPrecisionMayBeSetInAnyOrder() {
        LocalDateTime odd = LocalDateTime.of(2024, 1, 1, 12, 0, 30);
        LocalDateTime lastHalfHour = LocalDateTime.of(9999, 12, 31, 23, 30);
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::zonedDatetimes,
                        g -> g.min(odd),
                        g -> g.max(odd.plusMinutes(1)),
                        g -> g.precision(ChronoUnit.MINUTES)));
        inAnyOrder(Temporals::zonedDatetimes, g -> g.min(odd), g -> g.max(odd), g -> g.precision(ChronoUnit.MINUTES))
                .failsAtACall("no ");
        inAnyOrder(Temporals::zonedDatetimes, g -> g.min(lastHalfHour), g -> g.precision(ChronoUnit.HOURS))
                .failsAtTheFirstDraw("no ");
        assertEquals(
                Outcome.WORKS,
                inAnyOrder(
                        Temporals::zonedDatetimes,
                        g -> g.min(lastHalfHour),
                        g -> g.precision(ChronoUnit.HOURS),
                        g -> g.max(lastHalfHour.plusHours(1))));
    }
}
