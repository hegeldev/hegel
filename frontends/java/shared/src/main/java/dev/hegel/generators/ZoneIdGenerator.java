package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.DateTimeException;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Generates region {@link ZoneId} values, such as {@code America/New_York}, shrinking toward
 * {@code UTC}.
 *
 * <p>The names are those of the engine's bundled IANA time zone database that {@link
 * ZoneId#getAvailableZoneIds()} also lists, so every drawn zone resolves on this JVM and a saved
 * failure means the same zone on every machine. By default link names such as {@code
 * Asia/Calcutta} are included and legacy names such as {@code EST5EDT} and {@code US/Eastern} are
 * excluded; {@link #links(boolean)} and {@link #legacy(boolean)} switch each, and {@link
 * #names(String...)} restricts the draw to an explicit list.
 *
 * <p>Half the draws are uniform over the names and half weighted toward zones with a rare feature
 * (a fractional offset, negative daylight saving, a skipped day, ...), so each is reached by a
 * short run. A name shrinks to a zone a reader recognises: real zones with a whole-hour offset and
 * a simple daylight-saving rule first, then the other real zones, then the {@code Etc/GMT} family,
 * then the legacy names.
 */
public final class ZoneIdGenerator implements Generator<ZoneId> {
    private static final SpecCache[] HOST_SPECS = {new SpecCache(), new SpecCache(), new SpecCache(), new SpecCache()};
    private static final Map<String, SpecCache> SINGLE_SPECS = new ConcurrentHashMap<>();

    private final List<String> names;
    private final boolean links;
    private final boolean legacy;
    private final SpecCache spec = new SpecCache();

    public ZoneIdGenerator() {
        this(null, true, false);
    }

    private ZoneIdGenerator(List<String> names, boolean links, boolean legacy) {
        this.names = names;
        this.links = links;
        this.legacy = legacy;
    }

    /**
     * Draw only these names, each exactly as the IANA database spells it. {@link #links(boolean)}
     * and {@link #legacy(boolean)} do not apply to an explicit list. A name this JVM cannot resolve
     * is rejected here; one the engine's database lacks is rejected with an {@link
     * IllegalArgumentException} at the first draw.
     *
     * @param names the allowed names, at least one
     * @return a copy restricted to those names
     */
    public ZoneIdGenerator names(String... names) {
        return names(Arrays.asList(names));
    }

    /**
     * @param names the allowed names, at least one
     * @return a copy restricted to those names
     * @see #names(String...)
     */
    public ZoneIdGenerator names(Collection<String> names) {
        if (names.isEmpty()) {
            throw new IllegalArgumentException("zoneIds: names must not be empty");
        }
        for (String name : names) {
            try {
                ZoneId.of(name);
            } catch (DateTimeException e) {
                throw new IllegalArgumentException("zoneIds: \"" + name + "\" is not a zone this JVM knows", e);
            }
        }
        return new ZoneIdGenerator(List.copyOf(names), links, legacy);
    }

    /**
     * @param links whether the default set includes link names such as {@code Asia/Calcutta} (on by
     *     default)
     * @return a copy with the setting applied
     */
    public ZoneIdGenerator links(boolean links) {
        return new ZoneIdGenerator(names, links, legacy);
    }

    /**
     * @param legacy whether the default set includes legacy names such as {@code EST5EDT} and
     *     {@code US/Eastern} (off by default)
     * @return a copy with the setting applied
     */
    public ZoneIdGenerator legacy(boolean legacy) {
        return new ZoneIdGenerator(names, links, legacy);
    }

    /** The names in {@code available} the engine knows, sorted, filtered by the link and legacy flags. */
    private static List<String> hostNames(TestCase tc, Collection<String> available, boolean links, boolean legacy) {
        List<String> selected = new ArrayList<>();
        for (String name : new TreeSet<>(available)) {
            Libhegel.TimezoneInfo info = tc.timezoneInfo(name);
            if (!info.known() || (info.link() && !links) || ((info.tags() & Abi.TIMEZONE_TAG_LEGACY) != 0 && !legacy)) {
                continue;
            }
            selected.add(name);
        }
        return selected;
    }

    SpecHandle spec(TestCase tc) {
        if (names != null) {
            return spec.get(tc, t -> t.timezoneSpec(names, links, legacy));
        }
        SpecCache host = HOST_SPECS[(links ? 1 : 0) | (legacy ? 2 : 0)];
        return host.get(
                tc, t -> t.timezoneSpec(hostNames(t, ZoneId.getAvailableZoneIds(), links, legacy), links, legacy));
    }

    /**
     * A spec drawing only {@code zone}, when it is a region the engine's database has, or {@code
     * null} for a fixed offset or a region only this JVM knows.
     */
    static SpecHandle specFor(TestCase tc, ZoneId zone) {
        if (zone instanceof ZoneOffset || !tc.timezoneInfo(zone.getId()).known()) {
            return null;
        }
        String id = zone.getId();
        return SINGLE_SPECS
                .computeIfAbsent(id, k -> new SpecCache())
                .get(tc, t -> t.timezoneSpec(List.of(id), true, true));
    }

    /** @hidden */
    @Override
    public ZoneId doDraw(TestCase tc) {
        return ZoneId.of(tc.generateTimezone(spec(tc)));
    }
}
