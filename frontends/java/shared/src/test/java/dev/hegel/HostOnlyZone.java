package dev.hegel;

import java.time.ZoneId;
import java.time.zone.ZoneRules;
import java.time.zone.ZoneRulesProvider;
import java.util.NavigableMap;
import java.util.Set;
import java.util.TreeMap;

/**
 * A region zone only this JVM knows: {@code Hegel/Test}, with the rules of {@code
 * America/New_York}. The engine's database has no such name, so drawing in it takes the generators'
 * host-only path, while the JVM still has daylight-saving transitions to resolve against.
 */
final class HostOnlyZone extends ZoneRulesProvider {
    static final String ID = "Hegel/Test";
    private static final ZoneRules RULES = ZoneId.of("America/New_York").getRules();

    static {
        ZoneRulesProvider.registerProvider(new HostOnlyZone());
    }

    static ZoneId zone() {
        return ZoneId.of(ID);
    }

    @Override
    protected Set<String> provideZoneIds() {
        return Set.of(ID);
    }

    @Override
    protected ZoneRules provideRules(String zoneId, boolean forCaching) {
        return RULES;
    }

    @Override
    protected NavigableMap<String, ZoneRules> provideVersions(String zoneId) {
        NavigableMap<String, ZoneRules> versions = new TreeMap<>();
        versions.put("test", RULES);
        return versions;
    }
}
