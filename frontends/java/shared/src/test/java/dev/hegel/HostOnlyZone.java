package dev.hegel;

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
import java.util.List;
import java.util.Map;
import java.util.NavigableMap;
import java.util.Set;
import java.util.TreeMap;

/**
 * Region zones only this JVM knows. {@code Hegel/Test} has the rules of {@code America/New_York}:
 * the engine's database has no such name, so only the {@code java.time} zone source can draw in
 * it. {@code Hegel/OneRule} has a single yearly rule, to +02:00 each March, after a last explicit
 * transition to +03:00: {@code java.time} starts each year at the rule's +01:00 without a
 * transition, at midnight on the +03:00 wall clock, and resolves the wall-clock times around that
 * midnight to instants it reads back as other wall-clock times.
 */
final class HostOnlyZone extends ZoneRulesProvider {
    static final String ID = "Hegel/Test";
    static final String ONE_RULE_ID = "Hegel/OneRule";
    static final ZoneRules ONE_RULE = ZoneRules.of(
            ZoneOffset.ofHours(1),
            ZoneOffset.ofHours(1),
            List.of(),
            List.of(ZoneOffsetTransition.of(
                    LocalDateTime.of(1970, 1, 1, 0, 0), ZoneOffset.ofHours(1), ZoneOffset.ofHours(3))),
            List.of(ZoneOffsetTransitionRule.of(
                    Month.MARCH,
                    1,
                    null,
                    LocalTime.of(2, 0),
                    false,
                    TimeDefinition.WALL,
                    ZoneOffset.ofHours(1),
                    ZoneOffset.ofHours(1),
                    ZoneOffset.ofHours(2))));
    private static final Map<String, ZoneRules> RULES =
            Map.of(ID, ZoneId.of("America/New_York").getRules(), ONE_RULE_ID, ONE_RULE);

    static {
        ZoneRulesProvider.registerProvider(new HostOnlyZone());
    }

    static ZoneId zone() {
        return ZoneId.of(ID);
    }

    static ZoneId oneRule() {
        return ZoneId.of(ONE_RULE_ID);
    }

    @Override
    protected Set<String> provideZoneIds() {
        return RULES.keySet();
    }

    @Override
    protected ZoneRules provideRules(String zoneId, boolean forCaching) {
        return RULES.get(zoneId);
    }

    @Override
    protected NavigableMap<String, ZoneRules> provideVersions(String zoneId) {
        NavigableMap<String, ZoneRules> versions = new TreeMap<>();
        versions.put("test", RULES.get(zoneId));
        return versions;
    }
}
