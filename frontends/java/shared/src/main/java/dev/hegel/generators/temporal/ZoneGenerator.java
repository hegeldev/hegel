package dev.hegel.generators.temporal;

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
 * <p>The names are those of the generator's {@link ZoneSource} that {@link
 * ZoneId#getAvailableZoneIds()} also lists, so every drawn zone resolves on this JVM. The source is
 * {@link ZoneSource#getDefault()}, the JVM's own {@code java.time} database unless replaced, or the
 * one {@link #source(ZoneSource)} names. By default link names such as {@code Asia/Calcutta} are
 * included and legacy names such as {@code EST5EDT} and {@code US/Eastern} are excluded; {@link
 * #links(boolean)} and {@link #legacy(boolean)} switch each, and {@link #names(String...)}
 * restricts the draw to an explicit list.
 *
 * <p>Half the draws are uniform over the names and half weighted toward zones with a rare feature
 * (a fractional offset, negative daylight saving, a skipped day, ...), so each is reached by a
 * short run. A name shrinks to a zone a reader recognises: real zones with a whole-hour offset and
 * a simple daylight-saving rule first, then the other real zones, then the {@code Etc/GMT} family,
 * then the legacy names, and last any name the engine's catalogue of IANA names lacks.
 */
public final class ZoneGenerator implements Generator<ZoneId> {
    private record HostKey(ZoneSource source, boolean links, boolean legacy) {}

    private record SingleKey(ZoneSource source, String name) {}

    private static final Map<HostKey, SpecCache> HOST_SPECS = new ConcurrentHashMap<>();
    private static final Map<SingleKey, SpecCache> SINGLE_SPECS = new ConcurrentHashMap<>();

    private final List<String> names;
    private final boolean links;
    private final boolean legacy;
    private final ZoneSource source;
    private final Map<ZoneSource, SpecCache> specs = new ConcurrentHashMap<>();

    public ZoneGenerator() {
        this(null, true, false, null);
    }

    private ZoneGenerator(List<String> names, boolean links, boolean legacy, ZoneSource source) {
        this.names = names;
        this.links = links;
        this.legacy = legacy;
        this.source = source;
    }

    /**
     * Draw only these names, each exactly as this JVM spells it. {@link #links(boolean)} and {@link
     * #legacy(boolean)} do not apply to an explicit list. A name this JVM cannot resolve is
     * rejected here; one the zone source lacks is rejected with an {@link IllegalArgumentException}
     * at the first draw.
     *
     * @param names the allowed names, at least one
     * @return a copy restricted to those names
     */
    public ZoneGenerator names(String... names) {
        return names(Arrays.asList(names));
    }

    /**
     * @param names the allowed names, at least one
     * @return a copy restricted to those names
     * @see #names(String...)
     */
    public ZoneGenerator names(Collection<String> names) {
        if (names.isEmpty()) {
            throw new IllegalArgumentException("zones: names must not be empty");
        }
        for (String name : names) {
            try {
                ZoneId.of(name);
            } catch (DateTimeException e) {
                throw new IllegalArgumentException("zones: \"" + name + "\" is not a zone this JVM knows", e);
            }
        }
        return new ZoneGenerator(List.copyOf(names), links, legacy, source);
    }

    /**
     * @param links whether the default set includes link names such as {@code Asia/Calcutta} (on by
     *     default)
     * @return a copy with the setting applied
     */
    public ZoneGenerator links(boolean links) {
        return new ZoneGenerator(names, links, legacy, source);
    }

    /**
     * @param legacy whether the default set includes legacy names such as {@code EST5EDT} and
     *     {@code US/Eastern} (off by default)
     * @return a copy with the setting applied
     */
    public ZoneGenerator legacy(boolean legacy) {
        return new ZoneGenerator(names, links, legacy, source);
    }

    /**
     * Draw from, and resolve wall-clock times against, {@code source} rather than {@link
     * ZoneSource#getDefault()}.
     *
     * @param source the time zone data to draw against
     * @return a copy with the source set
     */
    public ZoneGenerator source(ZoneSource source) {
        return new ZoneGenerator(names, links, legacy, source);
    }

    /** The names in {@code available} the source knows, sorted, filtered by the link and legacy flags. */
    private static List<String> hostNames(
            TestCase tc, long source, Collection<String> available, boolean links, boolean legacy) {
        List<String> selected = new ArrayList<>();
        for (String name : new TreeSet<>(available)) {
            Libhegel.TimezoneInfo info = tc.timezoneInfo(source, name);
            if (!info.known() || (info.link() && !links) || ((info.tags() & Abi.TIMEZONE_TAG_LEGACY) != 0 && !legacy)) {
                continue;
            }
            selected.add(name);
        }
        return selected;
    }

    SpecHandle spec(TestCase tc) {
        ZoneSource from = source != null ? source : ZoneSource.getDefault();
        if (names != null) {
            return specs.computeIfAbsent(from, k -> new SpecCache()).get(tc, t -> {
                long handle = from.handle(t.binding());
                for (String name : names) {
                    if (!t.timezoneInfo(handle, name).known()) {
                        throw new IllegalArgumentException("zones: \"" + name + "\" is not a zone the source knows");
                    }
                }
                return t.timezoneSpec(names, links, legacy, handle);
            });
        }
        return HOST_SPECS
                .computeIfAbsent(new HostKey(from, links, legacy), k -> new SpecCache())
                .get(tc, t -> {
                    long handle = from.handle(t.binding());
                    return t.timezoneSpec(
                            hostNames(t, handle, ZoneId.getAvailableZoneIds(), links, legacy), links, legacy, handle);
                });
    }

    /**
     * A spec drawing only {@code zone} from the default source, when it is a region the source
     * has, or {@code null} for a fixed offset or a region only this JVM knows.
     */
    static SpecHandle specFor(TestCase tc, ZoneId zone) {
        ZoneSource from = ZoneSource.getDefault();
        long handle = from.handle(tc.binding());
        if (zone instanceof ZoneOffset || !tc.timezoneInfo(handle, zone.getId()).known()) {
            return null;
        }
        String id = zone.getId();
        return SINGLE_SPECS
                .computeIfAbsent(new SingleKey(from, id), k -> new SpecCache())
                .get(tc, t -> t.timezoneSpec(List.of(id), true, true, from.handle(t.binding())));
    }

    /** @hidden */
    @Override
    public ZoneId doDraw(TestCase tc) {
        return ZoneId.of(tc.generateTimezone(spec(tc)));
    }
}
