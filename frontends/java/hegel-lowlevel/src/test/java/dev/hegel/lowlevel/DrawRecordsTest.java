package dev.hegel.lowlevel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Instant;
import java.time.LocalDateTime;
import org.junit.jupiter.api.Test;

/** The records a binding fills in from the engine's structs carry every field through. */
class DrawRecordsTest {
    @Test
    void zonedDatetimeCarriesEveryField() {
        LocalDateTime civil = LocalDateTime.of(2024, 3, 10, 2, 30);
        Instant instant = Instant.ofEpochSecond(1710055800);
        Libhegel.ZonedDatetime drawn = new Libhegel.ZonedDatetime(
                "America/New_York", civil, -14400, true, false, Abi.TIMEZONE_KIND_GAP, -18000, -14400, instant);
        assertEquals("America/New_York", drawn.zone());
        assertEquals(civil, drawn.civil());
        assertEquals(-14400, drawn.offsetSeconds());
        assertTrue(drawn.offsetKnown());
        assertFalse(drawn.fold());
        assertEquals(Abi.TIMEZONE_KIND_GAP, drawn.kind());
        assertEquals(-18000, drawn.offsetBefore());
        assertEquals(-14400, drawn.offsetAfter());
        assertEquals(instant, drawn.timestamp());
    }

    @Test
    void timezoneInfoCarriesEveryField() {
        Libhegel.TimezoneInfo info = new Libhegel.TimezoneInfo(true, "Asia/Kolkata", true, 3, Abi.TIMEZONE_TAG_LINK);
        assertTrue(info.known());
        assertEquals("Asia/Kolkata", info.primaryName());
        assertTrue(info.link());
        assertEquals(3, info.rank());
        assertEquals(Abi.TIMEZONE_TAG_LINK, info.tags());
    }
}
