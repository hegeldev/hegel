package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.List;

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

        Libhegel.TimezoneInfo utc = lib.timezoneInfo("UTC");
        assertTrue(utc.known());
        assertEquals("UTC", utc.primaryName());
        assertFalse(utc.link());
        assertEquals(0, utc.rank());
        Libhegel.TimezoneInfo calcutta = lib.timezoneInfo("Asia/Calcutta");
        assertTrue(calcutta.known());
        assertTrue(calcutta.link());
        assertEquals("Asia/Kolkata", calcutta.primaryName());
        assertTrue((calcutta.tags() & Abi.TIMEZONE_TAG_LINK) != 0);
        Libhegel.TimezoneInfo unknown = lib.timezoneInfo("Mars/Olympus_Mons");
        assertFalse(unknown.known());
        assertTrue(lib.tzdbVersion().matches("\\d{4}[a-z]"), lib.tzdbVersion());
    }
}
