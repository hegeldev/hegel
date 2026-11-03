package dev.hegel.generators.temporal;

import dev.hegel.EngineAccess;
import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.Instant;
import java.time.ZoneOffset;
import java.time.zone.ZoneRules;
import java.time.zone.ZoneRulesProvider;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.TreeSet;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Supplier;

/**
 * Where the zone generators get the time zone data they classify, target and resolve wall-clock
 * times against: which transitions a zone has, and so which wall times are skipped or repeated.
 *
 * <p>By default that is {@link #javaTime()}, the {@code java.time} database of this JVM ({@link
 * ZoneRulesProvider}), so the gaps and folds Hegel aims at are the ones the code under test sees.
 * {@link #bundled()} is the engine's own IANA database (release {@link
 * dev.hegel.Generators.Temporal#tzdbVersion()}), the same on every machine. A generator draws from
 * the source given to {@link ZoneGenerator#source(ZoneSource)}, or else from the default, which
 * {@link #setDefault(ZoneSource)} replaces for the whole process.
 *
 * <p>A library with a database of its own provides a source through {@link #builder()}: zones from
 * TZif files, from {@code java.time} {@link ZoneRules}, or from an {@link Oracle} that answers
 * offset lookups, plus links, and names declared up front whose data a {@link Loader} supplies
 * the first time a draw needs it. A generator draws a {@link java.time.ZoneId}, so only the
 * source's names this JVM also resolves are drawn.
 *
 * <p>Whatever the source, the names, their shrink order and the weighting toward rare features
 * come from the engine's catalogue of IANA names, so a saved failure replays to the same zone and
 * wall-clock time everywhere. A name the catalogue lacks is drawn too, after every name it has.
 * Build a source once and keep it, as a generator is kept: the engine holds its data for the life
 * of the process.
 */
public final class ZoneSource {
    private static final ZoneSource BUNDLED = new ZoneSource(true, List.of(), null, null, null);
    private static final ZoneSource JAVA_TIME = new ZoneSource(
            false,
            List.of(),
            ZoneRulesProvider::getAvailableZoneIds,
            ZoneSource::loadJavaTime,
            ZoneSource::javaTimeVersion);

    private static volatile ZoneSource defaultSource = JAVA_TIME;

    private final boolean bundled;
    private final List<Consumer<Zones>> additions;
    private final Supplier<Collection<String>> declared;
    private final Loader loader;
    private final Supplier<String> version;

    private Libhegel binding;
    private long handle;

    private ZoneSource(
            boolean bundled,
            List<Consumer<Zones>> additions,
            Supplier<Collection<String>> declared,
            Loader loader,
            Supplier<String> version) {
        this.bundled = bundled;
        this.additions = additions;
        this.declared = declared;
        this.loader = loader;
        this.version = version;
    }

    /**
     * The engine's bundled IANA time zone database, one pinned release on every machine. A name
     * this JVM does not know, or knows with other rules, can then be drawn; {@link
     * ZoneGenerator} draws only the names this JVM knows, and {@link ZonedDateTimeGenerator}
     * rejects a test case whose value the two databases resolve differently.
     *
     * @return the bundled source
     */
    public static ZoneSource bundled() {
        return BUNDLED;
    }

    /**
     * The zone rules of this JVM's {@link ZoneRulesProvider}s, the database {@link
     * java.time.ZoneId#of(String)} answers from: every name {@link
     * java.time.ZoneId#getAvailableZoneIds()} lists, with each zone's data uploaded the first time
     * a draw needs it. The default source.
     *
     * @return the {@code java.time} source
     */
    public static ZoneSource javaTime() {
        return JAVA_TIME;
    }

    /**
     * @return a builder for a source of a library's own zone data
     */
    public static Builder builder() {
        return new Builder();
    }

    /**
     * @return the source a {@link ZoneGenerator} given none draws from: {@link #javaTime()} unless
     *     {@link #setDefault(ZoneSource)} replaced it
     */
    public static ZoneSource getDefault() {
        return defaultSource;
    }

    /**
     * Make {@code source} the default for the rest of the process: the one every {@link
     * ZoneGenerator} without a {@link ZoneGenerator#source(ZoneSource)} of its own draws from
     * from now on.
     *
     * @param source the new default
     */
    public static void setDefault(ZoneSource source) {
        Libhegel engine = EngineAccess.binding();
        synchronized (ZoneSource.class) {
            engine.setDefaultZoneSource(source.handle(engine));
            defaultSource = source;
        }
    }

    /**
     * Make the default source the engine's, as the frontend does when it loads the engine.
     *
     * @hidden
     */
    public static synchronized void installDefault(Libhegel engine) {
        engine.setDefaultZoneSource(defaultSource.handle(engine));
    }

    /**
     * @return the release of its database the source holds: the bundled IANA release for {@link
     *     #bundled()}, the {@code ZoneRulesProvider} version for {@link #javaTime()}, what {@link
     *     Builder#version(String)} recorded, or {@code ""}
     */
    public String version() {
        Libhegel engine = EngineAccess.binding();
        return engine.zoneSourceVersion(handle(engine));
    }

    /**
     * The engine handle of this source for {@code engine}, built the first time.
     *
     * @hidden
     */
    public synchronized long handle(Libhegel engine) {
        if (binding != engine) {
            long built = build(engine);
            if (binding != null) {
                binding.zoneSourceFree(handle);
            }
            binding = engine;
            handle = built;
        }
        return handle;
    }

    private long build(Libhegel engine) {
        long[] out = new long[1];
        if (bundled) {
            check(engine, engine.zoneSourceBundled(out));
            return out[0];
        }
        check(engine, engine.zoneSourceNew(out));
        long source = out[0];
        try {
            Zones zones = new EngineZones(engine, source);
            for (Consumer<Zones> addition : additions) {
                addition.accept(zones);
            }
            check(engine, engine.zoneSourceDeclare(source, List.copyOf(declared.get())));
            if (loader != null) {
                check(
                        engine,
                        engine.zoneSourceSetLoader(
                                source, (into, name) -> loader.load(name, new EngineZones(engine, into))));
            }
            String release = version.get();
            if (release != null) {
                check(engine, engine.zoneSourceSetVersion(source, release));
            }
            return source;
        } catch (RuntimeException e) {
            engine.zoneSourceFree(source);
            throw e;
        }
    }

    private static void check(Libhegel engine, int rc) {
        if (rc != Abi.OK) {
            throw new IllegalArgumentException(engine.lastErrorMessage());
        }
    }

    private static void loadJavaTime(String name, Zones zones) {
        zones.rules(name, ZoneRulesProvider.getRules(name, false));
    }

    private static String javaTimeVersion() {
        String first = new TreeSet<>(ZoneRulesProvider.getAvailableZoneIds()).first();
        return ZoneRulesProvider.getVersions(first).lastKey();
    }

    /**
     * Where a source's zone data goes: the {@link Builder} records it, and a {@link Loader} adds to
     * it as the engine asks. Data added for a name replaces what the source had for it.
     */
    public interface Zones {
        /**
         * Add a zone from a TZif file (RFC 9636, version 2 or later), such as one under {@code
         * /usr/share/zoneinfo}: its 64-bit transitions and the rule in its footer.
         *
         * @param name the name the zone is drawn by
         * @param tzif the file's contents
         */
        void tzif(String name, byte[] tzif);

        /**
         * Add a zone from {@code java.time} rules: their explicit transitions and the yearly rule
         * of their {@link ZoneRules#getTransitionRules()} after them.
         *
         * @param name the name the zone is drawn by
         * @param rules the zone's rules
         */
        void rules(String name, ZoneRules rules);

        /**
         * Add a zone by probing an oracle over its years.
         *
         * @param name the name the zone is drawn by
         * @param oracle the lookups and the years to probe
         */
        void oracle(String name, Oracle oracle);

        /**
         * Make {@code name} another name for {@code target}'s zone.
         *
         * @param name the link's name
         * @param target the name whose data applies, which need not be added yet
         */
        void link(String name, String target);
    }

    /**
     * Supplies a declared name's data the first time a draw or a query needs it, by adding the zone
     * (or a link) to {@code zones}, or nothing for a name it does not know. It is called on
     * whichever thread draws, possibly concurrently and more than once for a name; an exception it
     * throws fails the draw with an {@link IllegalArgumentException} carrying its message.
     */
    @FunctionalInterface
    public interface Loader {
        /**
         * @param name the name the engine needs
         * @param zones where to add its data
         */
        void load(String name, Zones zones);
    }

    /** What a zone has in force after the years an {@link Oracle} is probed over. */
    public enum Beyond {
        /** The last type probed, for ever: right for a library that stops applying rules. */
        HOLD(Abi.ZONE_AFTER_HOLD),
        /**
         * A yearly daylight-saving rule inferred from the last two years probed, or the last type
         * when they show none: right for a library that applies the current rule for ever.
         */
        INFER(Abi.ZONE_AFTER_INFER);

        final int engineValue;

        Beyond(int engineValue) {
            this.engineValue = engineValue;
        }
    }

    /**
     * A local time type an {@link Oracle} reports.
     *
     * @param offset the offset from UTC
     * @param dst whether it is daylight-saving time, which is reported but never classifies
     * @param abbreviation such as {@code "EST"}
     */
    public record LocalType(ZoneOffset offset, boolean dst, String abbreviation) {}

    /**
     * A zone a library answers lookups for but does not expose as data: probed into explicit
     * transitions over a range of years, by asking for each next transition when {@link
     * #nextTransition(Function)} gives a way to, and by sampling the offset once a day (which
     * misses a pair of changes less than a day apart) otherwise.
     */
    public static final class Oracle {
        private final Function<Instant, LocalType> typeAt;
        private final Function<Instant, Optional<Instant>> nextTransition;
        private final int firstYear;
        private final int lastYear;
        private final Beyond beyond;

        private Oracle(
                Function<Instant, LocalType> typeAt,
                Function<Instant, Optional<Instant>> nextTransition,
                int firstYear,
                int lastYear,
                Beyond beyond) {
            this.typeAt = typeAt;
            this.nextTransition = nextTransition;
            this.firstYear = firstYear;
            this.lastYear = lastYear;
            this.beyond = beyond;
        }

        /**
         * @param typeAt the local time type in force at an instant, which is always a whole second
         * @param firstYear the first year probed; the type at its start holds before it
         * @param lastYear the last year probed, at least {@code firstYear + 1} for {@link
         *     Beyond#INFER}
         * @param beyond what is in force after {@code lastYear}
         * @return an oracle probed by sampling
         */
        public static Oracle of(Function<Instant, LocalType> typeAt, int firstYear, int lastYear, Beyond beyond) {
            return new Oracle(typeAt, null, firstYear, lastYear, beyond);
        }

        /**
         * @param nextTransition the first instant after the given one (in whole seconds) at which
         *     the type changes, or empty when it never changes again
         * @return a copy probed by asking for each transition in turn
         */
        public Oracle nextTransition(Function<Instant, Optional<Instant>> nextTransition) {
            return new Oracle(typeAt, nextTransition, firstYear, lastYear, beyond);
        }
    }

    /**
     * Collects a source's zones, links, declared names, loader and version. Each method returns
     * this builder.
     */
    public static final class Builder {
        private final List<Consumer<Zones>> additions = new ArrayList<>();
        private final List<String> declared = new ArrayList<>();
        private Loader loader;
        private String version;

        private Builder() {}

        /**
         * @param name the name the zone is drawn by
         * @param tzif the contents of a TZif file
         * @return this builder
         * @see Zones#tzif(String, byte[])
         */
        public Builder tzif(String name, byte[] tzif) {
            byte[] copy = tzif.clone();
            additions.add(zones -> zones.tzif(name, copy));
            return this;
        }

        /**
         * @param name the name the zone is drawn by
         * @param rules the zone's rules
         * @return this builder
         * @see Zones#rules(String, ZoneRules)
         */
        public Builder rules(String name, ZoneRules rules) {
            additions.add(zones -> zones.rules(name, rules));
            return this;
        }

        /**
         * @param name the name the zone is drawn by
         * @param oracle the lookups and the years to probe
         * @return this builder
         * @see Zones#oracle(String, Oracle)
         */
        public Builder oracle(String name, Oracle oracle) {
            additions.add(zones -> zones.oracle(name, oracle));
            return this;
        }

        /**
         * @param name the link's name
         * @param target the name whose data applies
         * @return this builder
         * @see Zones#link(String, String)
         */
        public Builder link(String name, String target) {
            additions.add(zones -> zones.link(name, target));
            return this;
        }

        /**
         * Names the source knows whose data the {@link #loader(Loader)} supplies when a draw
         * needs it, so a zone is only loaded once it is drawn.
         *
         * @param names the names
         * @return this builder
         */
        public Builder declare(String... names) {
            return declare(Arrays.asList(names));
        }

        /**
         * @param names the names
         * @return this builder
         * @see #declare(String...)
         */
        public Builder declare(Collection<String> names) {
            declared.addAll(names);
            return this;
        }

        /**
         * @param loader supplies the data of a name the source lacks, declared or not, the first
         *     time a draw or a query needs it
         * @return this builder
         */
        public Builder loader(Loader loader) {
            this.loader = loader;
            return this;
        }

        /**
         * @param version which release of its database the source holds, as {@link
         *     ZoneSource#version()} reports it
         * @return this builder
         */
        public Builder version(String version) {
            this.version = version;
            return this;
        }

        /**
         * @return a source of what this builder collected; its data reaches the engine when a
         *     generator first draws from it
         */
        public ZoneSource build() {
            List<String> names = List.copyOf(declared);
            String release = version;
            return new ZoneSource(false, List.copyOf(additions), () -> names, loader, () -> release);
        }
    }

    private record EngineZones(Libhegel engine, long source) implements Zones {
        @Override
        public void tzif(String name, byte[] tzif) {
            check(engine, engine.zoneSourceAddTzif(source, name, tzif));
        }

        @Override
        public void rules(String name, ZoneRules rules) {
            check(engine, engine.zoneSourceAddZone(source, name, JavaTimeZones.data(rules)));
        }

        @Override
        public void oracle(String name, Oracle oracle) {
            Function<Instant, Optional<Instant>> next = oracle.nextTransition;
            check(
                    engine,
                    engine.zoneSourceAddOracle(
                            source,
                            name,
                            instant -> {
                                LocalType type = oracle.typeAt.apply(Instant.ofEpochSecond(instant));
                                return new Libhegel.ZoneType(
                                        type.offset().getTotalSeconds(), type.dst(), type.abbreviation());
                            },
                            next == null
                                    ? null
                                    : after -> next.apply(Instant.ofEpochSecond(after))
                                            .map(i -> OptionalLong.of(i.getEpochSecond()))
                                            .orElse(OptionalLong.empty()),
                            oracle.firstYear,
                            oracle.lastYear,
                            oracle.beyond.engineValue));
        }

        @Override
        public void link(String name, String target) {
            check(engine, engine.zoneSourceAddLink(source, name, target));
        }
    }
}
