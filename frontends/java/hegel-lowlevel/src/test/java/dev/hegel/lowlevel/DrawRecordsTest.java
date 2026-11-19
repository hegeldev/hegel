package dev.hegel.lowlevel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Instant;
import java.time.LocalDateTime;
import java.util.List;
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

    @Test
    void timezoneOffsetAndResolutionCarryEveryField() {
        Libhegel.TimezoneOffset offset = new Libhegel.TimezoneOffset(3600, true, true, "BST");
        assertEquals(3600, offset.offsetSeconds());
        assertTrue(offset.offsetKnown());
        assertTrue(offset.isDst());
        assertEquals("BST", offset.abbreviation());
        Instant instant = Instant.ofEpochSecond(1711846800);
        Libhegel.TimezoneResolution resolved =
                new Libhegel.TimezoneResolution(Abi.TIMEZONE_KIND_FOLD, 3600, 0, 3600, true, instant);
        assertEquals(Abi.TIMEZONE_KIND_FOLD, resolved.kind());
        assertEquals(3600, resolved.offsetBefore());
        assertEquals(0, resolved.offsetAfter());
        assertEquals(3600, resolved.offsetSeconds());
        assertTrue(resolved.offsetKnown());
        assertEquals(instant, resolved.timestamp());
    }

    @Test
    void zoneDataCarriesEveryField() {
        Libhegel.ZoneType gmt = new Libhegel.ZoneType(0, false, "GMT");
        Libhegel.ZoneType bst = new Libhegel.ZoneType(3600, true, "BST");
        Libhegel.ZoneTransition transition = new Libhegel.ZoneTransition(-1, bst);
        Libhegel.ZoneRuleDate start = new Libhegel.ZoneRuleDate(Abi.ZONE_RULE_DAY_MONTH_WEEK_DAY, 0, 3, 5, 0, 3600);
        Libhegel.ZoneRuleDate end = new Libhegel.ZoneRuleDate(Abi.ZONE_RULE_DAY_JULIAN, 60, 0, 0, 0, 7200);
        Libhegel.ZoneData data =
                new Libhegel.ZoneData(gmt, List.of(transition), Abi.ZONE_AFTER_RULE, gmt, true, bst, start, end, 2037);
        assertEquals(3600, bst.utoff());
        assertTrue(bst.isDst());
        assertEquals("BST", bst.abbreviation());
        assertEquals(-1, transition.instant());
        assertEquals(bst, transition.type());
        assertEquals(Abi.ZONE_RULE_DAY_MONTH_WEEK_DAY, start.kind());
        assertEquals(60, end.day());
        assertEquals(3, start.month());
        assertEquals(5, start.week());
        assertEquals(0, start.weekday());
        assertEquals(3600, start.time());
        assertEquals(gmt, data.initial());
        assertEquals(List.of(transition), data.transitions());
        assertEquals(Abi.ZONE_AFTER_RULE, data.after());
        assertEquals(gmt, data.standard());
        assertTrue(data.hasDst());
        assertEquals(bst, data.dst());
        assertEquals(start, data.dstStart());
        assertEquals(end, data.dstEnd());
        assertEquals(2037, data.lastYear());
    }
}
