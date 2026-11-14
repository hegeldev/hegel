package dev.hegel.generators.temporal;

import dev.hegel.EngineAccess;
import dev.hegel.Generator;
import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.ref.Reference;
import java.time.DateTimeException;
import java.time.ZoneId;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;
import java.util.TreeSet;

/**
 * Generates region {@link ZoneId} values, such as {@code America/New_York}, shrinking toward
 * {@code UTC}.
 *
 * <p>The names are those of the generator's {@link ZoneSource} that {@link
 * ZoneId#getAvailableZoneIds()} also lists, so every drawn zone resolves on this JVM. The source is
 * the one {@link #source(ZoneSource)} names, or else {@link ZoneSource#getDefault()} as it was when
 * the generator was created: the JVM's own {@code java.time} database unless replaced. By default
 * link names such as {@code Asia/Calcutta} are included and legacy names such as {@code EST5EDT}
 * and {@code US/Eastern} are excluded; {@link #allowLinks(boolean)} and {@link
 * #allowLegacy(boolean)} switch each, and {@link #names(String...)} restricts the draw to an
 * explicit list.
 *
 * <p>Half the draws are uniform over the names and half weighted toward zones with a rare feature
 * (a fractional offset, negative daylight saving, a skipped day, ...), so each is reached by a
 * short run. A name shrinks to a zone a reader recognises: real zones with a whole-hour offset and
 * a simple daylight-saving rule first, then the other real zones, then the {@code Etc/GMT} family,
 * then the legacy names, and last any name the engine's catalogue of IANA names lacks.
 */
public final class ZoneGenerator implements Generator<ZoneId> {
    private final List<String> names;
    private final boolean allowLinks;
    private final boolean allowLegacy;
    private final ZoneSource source;
    private final SpecCache listed = new SpecCache();

    public ZoneGenerator() {
        this(null, true, false, ZoneSource.getDefault());
    }

    private ZoneGenerator(List<String> names, boolean allowLinks, boolean allowLegacy, ZoneSource source) {
        this.names = names;
        this.allowLinks = allowLinks;
        this.allowLegacy = allowLegacy;
        this.source = source;
        spec(EngineAccess.setup());
    }

    /** The default set of names of {@code source}. */
    static ZoneGenerator of(ZoneSource source) {
        return new ZoneGenerator(null, true, false, source);
    }

    /**
     * Draw only these names, each exactly as this JVM spells it. {@link #allowLinks(boolean)} and
     * {@link #allowLegacy(boolean)} do not apply to an explicit list. A name this JVM cannot
     * resolve, or that the generator's zone source lacks, is rejected with an {@link
     * IllegalArgumentException}: to draw names of another source, call {@link #source(ZoneSource)}
     * before this.
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
        return new ZoneGenerator(List.copyOf(names), allowLinks, allowLegacy, source);
    }

    /**
     * @param allow whether the default set includes link names such as {@code Asia/Calcutta} (on by
     *     default)
     * @return a copy with the setting applied
     */
    public ZoneGenerator allowLinks(boolean allow) {
        return new ZoneGenerator(names, allow, allowLegacy, source);
    }

    /**
     * @param allow whether the default set includes legacy names such as {@code EST5EDT} and {@code
     *     US/Eastern} (off by default)
     * @return a copy with the setting applied
     */
    public ZoneGenerator allowLegacy(boolean allow) {
        return new ZoneGenerator(names, allowLinks, allow, source);
    }

    /**
     * Draw from {@code source} rather than the default zone source: the names are its names, and a
     * {@link ZonedDateTimeGenerator} given this generator aims its wall-clock times at the
     * source's transitions.
     *
     * @param source the time zone data to draw from
     * @return a copy with the source set
     */
    public ZoneGenerator source(ZoneSource source) {
        return new ZoneGenerator(names, allowLinks, allowLegacy, source);
    }

    ZoneSource source() {
        return source;
    }

    /** The names in {@code available} the source knows, sorted, filtered by the link and legacy flags. */
    private List<String> hostNames(TestCase tc, long handle, Collection<String> available) {
        List<String> selected = new ArrayList<>();
        for (String name : new TreeSet<>(available)) {
            Libhegel.TimezoneInfo info = tc.timezoneInfo(handle, name);
            if (!info.known()
                    || (info.link() && !allowLinks)
                    || ((info.tags() & Abi.TIMEZONE_TAG_LEGACY) != 0 && !allowLegacy)) {
                continue;
            }
            selected.add(name);
        }
        if (selected.isEmpty()) {
            throw new IllegalArgumentException("zones: the zone source has no zone this JVM knows that the"
                    + " allowLinks and allowLegacy settings admit");
        }
        return selected;
    }

    private SpecHandle build(TestCase tc) {
        long handle = source.handle(tc.binding());
        if (names != null) {
            return tc.timezoneSpec(names, allowLinks, allowLegacy, handle);
        }
        if (source == ZoneSource.javaTime()) {
            return tc.timezoneSpec(null, allowLinks, allowLegacy, handle);
        }
        return tc.timezoneSpec(hostNames(tc, handle, ZoneId.getAvailableZoneIds()), allowLinks, allowLegacy, handle);
    }

    SpecHandle spec(TestCase tc) {
        try {
            SpecCache cache = names != null ? listed : source.hostSpec(allowLinks, allowLegacy);
            return cache.get(tc, this::build);
        } finally {
            Reference.reachabilityFence(source);
        }
    }

    /** @hidden */
    @Override
    public ZoneId doDraw(TestCase tc) {
        return ZoneId.of(tc.generateTimezone(spec(tc)));
    }
}
