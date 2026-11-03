package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.generators.temporal.ZoneSource;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.List;
import java.util.OptionalLong;

/** Binding-level checks shared by the FFM and JNA backends' test suites. */
final class BindingEdges {
    private BindingEdges() {}

    /** Every structured draw rejects a NULL test case before writing anything out. */
    static void structuredDrawsReportNullHandles(Libhegel lib) {
        LocalDate d = LocalDate.of(2000, 1, 1);
        LocalDate[] date = new LocalDate[1];
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateDate(0, d, d, 0, date));
        assertNull(date[0]);
        LocalTime t = LocalTime.NOON;
        LocalTime[] time = new LocalTime[1];
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateTime(0, t, t, Abi.PRECISION_NANOSECONDS, time));
        assertNull(time[0]);
        LocalDateTime dt = LocalDateTime.of(d, t);
        LocalDateTime[] datetime = new LocalDateTime[1];
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateDatetime(0, dt, dt, Abi.PRECISION_NANOSECONDS, datetime));
        assertNull(datetime[0]);
        Instant[] instant = new Instant[1];
        assertEquals(
                Abi.E_INVALID_HANDLE,
                lib.generateTimestamp(0, Instant.EPOCH, Instant.EPOCH, Abi.PRECISION_NANOSECONDS, instant));
        assertNull(instant[0]);
        int[] seconds = {7};
        boolean[] known = {true};
        assertEquals(
                Abi.E_INVALID_HANDLE,
                lib.generateOffset(0, 0, 0, Abi.OFFSET_GRANULARITY_SECONDS, false, seconds, known));
        assertEquals(7, seconds[0]);
        assertTrue(known[0]);
        Duration[] duration = new Duration[1];
        assertEquals(
                Abi.E_INVALID_HANDLE,
                lib.generateDuration(0, Duration.ZERO, Duration.ZERO, Abi.PRECISION_NANOSECONDS, duration));
        assertNull(duration[0]);

        long[] tz = new long[1];
        assertEquals(Abi.OK, lib.timezoneSpecNew(tz));
        assertEquals(Abi.OK, lib.timezoneSpecSetNames(tz[0], List.of("UTC")));
        String[] zone = new String[1];
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateTimezone(0, tz[0], zone));
        assertNull(zone[0]);
        long[] zoned = new long[1];
        assertEquals(Abi.OK, lib.zonedSpecNew(zoned));
        Libhegel.ZonedDatetime[] value = new Libhegel.ZonedDatetime[1];
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateZonedDatetime(0, tz[0], zoned[0], value));
        assertNull(value[0]);
        lib.zonedSpecFree(zoned[0]);
        lib.timezoneSpecFree(tz[0]);

        long[] span = new long[1];
        assertEquals(Abi.OK, lib.spanSpecNew(span));
        long[] units = new long[10];
        units[0] = 7;
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateSpan(0, span[0], units));
        assertEquals(7, units[0]);
        lib.spanSpecFree(span[0]);

        long[] options = new long[1];
        assertEquals(Abi.OK, lib.dateOptionsNew(options));
        assertEquals(Abi.E_INVALID_ARG, lib.dateOptionsSetWeekdays(options[0], 0));
        assertEquals(Abi.E_INVALID_ARG, lib.dateOptionsSetMonths(options[0], 0));
        lib.dateOptionsFree(options[0]);

        long[] bundled = new long[1];
        assertEquals(Abi.OK, lib.zoneSourceBundled(bundled));
        Libhegel.TimezoneInfo utc = info(lib, bundled[0], "UTC");
        assertTrue(utc.known());
        assertEquals("UTC", utc.primaryName());
        assertFalse(utc.link());
        assertEquals(0, utc.rank());
        Libhegel.TimezoneInfo calcutta = info(lib, bundled[0], "Asia/Calcutta");
        assertTrue(calcutta.known());
        assertTrue(calcutta.link());
        assertEquals("Asia/Kolkata", calcutta.primaryName());
        assertTrue((calcutta.tags() & Abi.TIMEZONE_TAG_LINK) != 0);
        Libhegel.TimezoneInfo unknown = info(lib, bundled[0], "Mars/Olympus_Mons");
        assertFalse(unknown.known());
        assertEquals("", unknown.primaryName());
        assertTrue(lib.tzdbVersion().matches("\\d{4}[a-z]"), lib.tzdbVersion());
        assertEquals(lib.tzdbVersion(), lib.zoneSourceVersion(bundled[0]));
        lib.zoneSourceFree(bundled[0]);
    }

    static Libhegel.TimezoneInfo info(Libhegel lib, long source, String name) {
        Libhegel.TimezoneInfo[] out = new Libhegel.TimezoneInfo[1];
        assertEquals(Abi.OK, lib.timezoneInfo(source, name, out), name);
        return out[0];
    }

    static Libhegel.TimezoneOffset offsetAt(Libhegel lib, long source, String name, Instant instant) {
        Libhegel.TimezoneOffset[] out = new Libhegel.TimezoneOffset[1];
        assertEquals(Abi.OK, lib.timezoneOffsetAt(source, name, instant, out), name + " at " + instant);
        return out[0];
    }

    static Libhegel.TimezoneResolution resolve(Libhegel lib, long source, String name, LocalDateTime civil) {
        Libhegel.TimezoneResolution[] out = new Libhegel.TimezoneResolution[1];
        assertEquals(Abi.OK, lib.timezoneResolve(source, name, civil, out), name + " at " + civil);
        return out[0];
    }

    /** A TZif file (version 2) of a zone fixed at UTC. */
    static byte[] utcTzif() {
        ByteBuffer block = ByteBuffer.allocate(44 + 6 + 4);
        block.put("TZif2".getBytes(StandardCharsets.US_ASCII));
        block.put(new byte[15]);
        block.putInt(0).putInt(0).putInt(0).putInt(0).putInt(1).putInt(4);
        block.putInt(0).put((byte) 0).put((byte) 0);
        block.put("UTC\0".getBytes(StandardCharsets.US_ASCII));
        byte[] once = block.array();
        byte[] footer = "\nUTC0\n".getBytes(StandardCharsets.US_ASCII);
        byte[] file = new byte[once.length * 2 + footer.length];
        System.arraycopy(once, 0, file, 0, once.length);
        System.arraycopy(once, 0, file, once.length, once.length);
        System.arraycopy(footer, 0, file, once.length * 2, footer.length);
        return file;
    }

    private static final Libhegel.ZoneType PLUS_ONE = new Libhegel.ZoneType(3600, false, "+01");
    private static final Libhegel.ZoneType PLUS_TWO = new Libhegel.ZoneType(7200, true, "+02");
    private static final Instant Y2K = Instant.parse("2000-01-01T00:00:00Z");

    /**
     * Every zone source call through the binding: each shape of data, links, declared names and a
     * loader, versions, the default, a spec's source, the queries, and callbacks that fail.
     */
    static void zoneSources(Libhegel lib) {
        long[] out = new long[1];
        assertEquals(Abi.OK, lib.zoneSourceNew(out));
        long source = out[0];

        assertEquals(Abi.OK, lib.zoneSourceAddTzif(source, "Test/Tzif", utcTzif()));
        assertEquals("UTC", offsetAt(lib, source, "Test/Tzif", Y2K).abbreviation());
        assertEquals(Abi.E_INVALID_ARG, lib.zoneSourceAddTzif(source, "Test/Bad", new byte[0]));

        Libhegel.ZoneRuleDate lastSundayOfMarch =
                new Libhegel.ZoneRuleDate(Abi.ZONE_RULE_DAY_MONTH_WEEK_DAY, 0, 3, 5, 0, 7200);
        Libhegel.ZoneRuleDate lastSundayOfOctober =
                new Libhegel.ZoneRuleDate(Abi.ZONE_RULE_DAY_MONTH_WEEK_DAY, 0, 10, 5, 0, 10800);
        assertEquals(
                Abi.OK,
                lib.zoneSourceAddZone(
                        source,
                        "Test/Rule",
                        new Libhegel.ZoneData(
                                PLUS_TWO,
                                List.of(new Libhegel.ZoneTransition(Y2K.getEpochSecond(), PLUS_ONE)),
                                Abi.ZONE_AFTER_RULE,
                                PLUS_ONE,
                                true,
                                PLUS_TWO,
                                lastSundayOfMarch,
                                lastSundayOfOctober,
                                0)));
        assertEquals(
                7200, offsetAt(lib, source, "Test/Rule", Y2K.minusSeconds(1)).offsetSeconds());
        Libhegel.TimezoneOffset summer = offsetAt(lib, source, "Test/Rule", Instant.parse("2050-07-01T00:00:00Z"));
        assertEquals(new Libhegel.TimezoneOffset(7200, true, true, "+02"), summer);
        assertEquals(
                3600,
                offsetAt(lib, source, "Test/Rule", Instant.parse("2050-12-01T00:00:00Z"))
                        .offsetSeconds());
        Libhegel.TimezoneResolution gap = resolve(lib, source, "Test/Rule", LocalDateTime.of(2050, 3, 27, 2, 30));
        assertEquals(
                new Libhegel.TimezoneResolution(
                        Abi.TIMEZONE_KIND_GAP, 3600, 7200, 7200, true, Instant.parse("2050-03-27T01:30:00Z")),
                gap);
        Libhegel.ZoneRuleDate day100 = new Libhegel.ZoneRuleDate(Abi.ZONE_RULE_DAY_JULIAN, 100, 0, 0, 0, 0);
        Libhegel.ZoneRuleDate day300 = new Libhegel.ZoneRuleDate(Abi.ZONE_RULE_DAY_JULIAN, 300, 0, 0, 0, 0);
        assertEquals(
                Abi.OK,
                lib.zoneSourceAddZone(
                        source,
                        "Test/Julian",
                        new Libhegel.ZoneData(
                                PLUS_ONE,
                                List.of(),
                                Abi.ZONE_AFTER_RULE,
                                PLUS_ONE,
                                true,
                                PLUS_TWO,
                                day100,
                                day300,
                                0)));
        assertEquals(
                7200,
                offsetAt(lib, source, "Test/Julian", Instant.parse("2030-07-01T00:00:00Z"))
                        .offsetSeconds());
        assertEquals(
                3600,
                offsetAt(lib, source, "Test/Julian", Instant.parse("2030-12-01T00:00:00Z"))
                        .offsetSeconds());
        assertEquals(
                Abi.E_INVALID_ARG,
                lib.zoneSourceAddZone(
                        source,
                        "Test/Unordered",
                        new Libhegel.ZoneData(
                                PLUS_ONE,
                                List.of(
                                        new Libhegel.ZoneTransition(10, PLUS_TWO),
                                        new Libhegel.ZoneTransition(5, PLUS_ONE)),
                                Abi.ZONE_AFTER_HOLD,
                                PLUS_ONE,
                                false,
                                PLUS_ONE,
                                day100,
                                day100,
                                0)));

        long change = Y2K.getEpochSecond();
        assertEquals(
                Abi.OK,
                lib.zoneSourceAddOracle(
                        source,
                        "Test/Sampled",
                        instant -> instant < change ? PLUS_ONE : PLUS_TWO,
                        null,
                        1998,
                        2002,
                        Abi.ZONE_AFTER_HOLD));
        assertEquals(
                Abi.OK,
                lib.zoneSourceAddOracle(
                        source,
                        "Test/Stepped",
                        instant -> instant < change ? PLUS_ONE : PLUS_TWO,
                        after -> after < change ? OptionalLong.of(change) : OptionalLong.empty(),
                        1998,
                        2002,
                        Abi.ZONE_AFTER_INFER));
        for (String name : List.of("Test/Sampled", "Test/Stepped")) {
            assertEquals(3600, offsetAt(lib, source, name, Y2K.minusSeconds(1)).offsetSeconds(), name);
            assertEquals(7200, offsetAt(lib, source, name, Y2K).offsetSeconds(), name);
        }
        assertEquals(
                Abi.E_INVALID_ARG,
                lib.zoneSourceAddOracle(
                        source,
                        "Test/Broken",
                        instant -> {
                            throw new IllegalStateException("the oracle broke");
                        },
                        null,
                        2000,
                        2001,
                        Abi.ZONE_AFTER_HOLD));
        assertTrue(lib.lastErrorMessage().contains("the oracle broke"), lib.lastErrorMessage());
        assertEquals(
                Abi.E_INVALID_ARG,
                lib.zoneSourceAddOracle(
                        source,
                        "Test/Broken",
                        instant -> PLUS_ONE,
                        after -> {
                            throw new UnsupportedOperationException();
                        },
                        2000,
                        2001,
                        Abi.ZONE_AFTER_HOLD));
        assertTrue(lib.lastErrorMessage().contains("UnsupportedOperationException"), lib.lastErrorMessage());

        assertEquals(Abi.OK, lib.zoneSourceAddLink(source, "Test/Link", "Test/Rule"));
        Libhegel.TimezoneInfo link = info(lib, source, "Test/Link");
        assertTrue(link.known());
        assertEquals("Test/Rule", link.primaryName());
        assertEquals(Abi.E_INVALID_ARG, lib.zoneSourceAddLink(source, "Test/Self", "Test/Self"));

        assertEquals(Abi.OK, lib.zoneSourceDeclare(source, List.of("Test/Lazy", "Test/Failing", "Test/Inner")));
        assertEquals(Abi.E_INVALID_ARG, lib.zoneSourceDeclare(source, List.of("")));
        assertEquals(Abi.OK, lib.zoneSourceSetLoader(source, (into, name) -> {
            if (name.equals("Test/Failing")) {
                throw new IllegalStateException("no data for " + name);
            }
            if (name.equals("Test/Inner")) {
                assertEquals(Abi.E_INVALID_ARG, lib.zoneSourceAddTzif(into, name, new byte[0]));
                throw new IllegalStateException("inner: " + lib.lastErrorMessage());
            }
            if (name.equals("Test/Lazy")) {
                assertEquals(Abi.OK, lib.zoneSourceAddLink(into, name, "Test/Rule"));
            }
        }));
        assertEquals(
                7200, offsetAt(lib, source, "Test/Lazy", Y2K.minusSeconds(1)).offsetSeconds());
        Libhegel.TimezoneInfo[] failed = {null};
        assertEquals(Abi.E_INVALID_ARG, lib.timezoneInfo(source, "Test/Failing", failed));
        assertNull(failed[0]);
        assertTrue(lib.lastErrorMessage().contains("no data for Test/Failing"), lib.lastErrorMessage());
        Libhegel.TimezoneOffset[] noOffset = {null};
        assertEquals(Abi.E_INVALID_ARG, lib.timezoneOffsetAt(source, "Test/Inner", Y2K, noOffset));
        assertNull(noOffset[0]);
        assertTrue(lib.lastErrorMessage().contains("inner: "), lib.lastErrorMessage());
        Libhegel.TimezoneResolution[] noResolution = {null};
        assertEquals(
                Abi.E_INVALID_ARG,
                lib.timezoneResolve(source, "Mars/Olympus_Mons", LocalDateTime.of(2000, 1, 1, 0, 0), noResolution));
        assertNull(noResolution[0]);
        assertFalse(info(lib, source, "Mars/Olympus_Mons").known());
        assertEquals(Abi.OK, lib.zoneSourceSetLoader(source, null));

        assertEquals("", lib.zoneSourceVersion(source));
        assertEquals(Abi.OK, lib.zoneSourceSetVersion(source, "test-1"));
        assertEquals("test-1", lib.zoneSourceVersion(source));

        long[] spec = new long[1];
        assertEquals(Abi.OK, lib.timezoneSpecNew(spec));
        lib.timezoneSpecSetLegacy(spec[0], true);
        assertEquals(Abi.OK, lib.timezoneSpecSetSource(spec[0], source), lib.lastErrorMessage());
        assertEquals(Abi.OK, lib.timezoneSpecSetNames(spec[0], List.of("Test/Rule", "Test/Link")));
        long[] empty = new long[1];
        assertEquals(Abi.OK, lib.zoneSourceNew(empty));
        assertEquals(Abi.E_INVALID_ARG, lib.timezoneSpecSetSource(spec[0], empty[0]));
        lib.timezoneSpecFree(spec[0]);
        lib.zoneSourceFree(empty[0]);

        try {
            lib.setDefaultZoneSource(source);
            assertTrue(info(lib, 0, "Test/Rule").known());
            lib.setDefaultZoneSource(0);
            assertFalse(info(lib, 0, "Test/Rule").known());
        } finally {
            ZoneSource.setDefault(ZoneSource.getDefault());
        }
        lib.zoneSourceFree(source);
    }
}
