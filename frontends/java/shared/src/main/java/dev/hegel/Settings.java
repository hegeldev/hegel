package dev.hegel;

import dev.hegel.lowlevel.Libhegel;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.OptionalLong;
import java.util.function.Consumer;

/**
 * Immutable configuration for a Hegel run, built with a fluent builder.
 *
 * <p>Start from {@code new Settings()}, adjust with the fluent methods, and pass the result to
 * {@link Hegel#test(java.util.function.Consumer, Settings)}:
 *
 * <pre>{@code
 * Hegel.test(tc -> { ... }, new Settings().testCases(500).seed(42));
 * }</pre>
 *
 * <p>Creating a {@code Settings} resolves an engine settings profile and copies every value it
 * resolves: the default profile for {@code new Settings()}, or the profile named to {@link
 * #fromProfile(String)}. The engine resolves the default profile from {@link
 * #setDefaultProfile(String)}, the {@code HEGEL_DEFAULT_PROFILE} environment variable and a {@code
 * hegel.toml} in the working directory or one of its parents (or the file named by {@code
 * HEGEL_CONFIG}), else from the detected environment: in CI (which it detects from {@code CI},
 * {@code GITHUB_ACTIONS} and similar variables) the {@code ci} profile applies, under which runs are
 * deterministic ({@code derandomize}) and the example database is disabled. The {@code HEGEL_*}
 * settings environment variables then apply over the profile. The shipped profiles run 100 test
 * cases.
 *
 * <p>Each fluent method replaces the resolved value of its setting; every other setting keeps it.
 * The values are fixed when the {@code Settings} is created: a profile that {@link
 * #registerProfile} or {@link #setDefaultProfile} changes afterwards does not change it, and a
 * malformed {@code hegel.toml} or {@code HEGEL_*} variable throws an {@link
 * IllegalArgumentException} from the creation. {@link #set(String, String)} is the exception: the
 * engine parses a string setting when the run starts, so it applies over the typed settings.
 */
public final class Settings {
    final long testCases;
    final boolean hasSeed;
    final long seed;
    final boolean derandomize;
    final Database database;
    final int suppressMask;
    final int phasesMask;
    final Verbosity verbosity;
    final Backend backend;
    final boolean reportMultipleFailures;
    final boolean stopOnFirstFailure;
    final boolean showStatistics;
    final boolean unboundedChoices;
    final boolean printBlob;
    final NondeterminismStrictness nondeterminismStrictness;
    final List<String> reproduceFailure;
    final boolean annotated;
    final String name;
    final TestLocation testLocation; // null = locate the test where it is run from
    final List<String> infrastructurePackages;
    final List<Example> examples;
    final List<Map.Entry<String, String>> stringSettings;

    /**
     * Create settings resolved from the engine's default profile and the environment (100 test
     * cases, all phases and a single reported failure in the shipped profiles).
     *
     * @throws IllegalArgumentException if a {@code hegel.toml} or settings environment variable is
     *     malformed
     */
    public Settings() {
        this(resolve(Engine.get(), null));
    }

    private Settings(Builder b) {
        this.testCases = b.testCases;
        this.hasSeed = b.hasSeed;
        this.seed = b.seed;
        this.derandomize = b.derandomize;
        this.database = b.database;
        this.suppressMask = b.suppressMask;
        this.phasesMask = b.phasesMask;
        this.verbosity = b.verbosity;
        this.backend = b.backend;
        this.reportMultipleFailures = b.reportMultipleFailures;
        this.stopOnFirstFailure = b.stopOnFirstFailure;
        this.showStatistics = b.showStatistics;
        this.unboundedChoices = b.unboundedChoices;
        this.printBlob = b.printBlob;
        this.nondeterminismStrictness = b.nondeterminismStrictness;
        this.reproduceFailure = b.reproduceFailure;
        this.annotated = b.annotated;
        this.name = b.name;
        this.testLocation = b.testLocation;
        this.infrastructurePackages = b.infrastructurePackages;
        this.examples = b.examples;
        this.stringSettings = b.stringSettings;
    }

    /** Settings resolved by {@code lib} from {@code profile} ({@code null} for the default one). */
    static Settings resolved(Libhegel lib, String profile) {
        return new Settings(resolve(lib, profile));
    }

    /**
     * Every value {@code lib} resolves for {@code profile} ({@code null} for the default one), read
     * back from a settings handle that is freed again before this returns.
     */
    private static Builder resolve(Libhegel lib, String profile) {
        long s = Runner.newSettings(lib, profile);
        try {
            Builder b = new Builder();
            b.testCases = lib.settingsGetTestCases(s);
            OptionalLong seed = lib.settingsGetSeed(s);
            b.hasSeed = seed.isPresent();
            b.seed = seed.orElse(0L);
            b.derandomize = lib.settingsGetDerandomize(s);
            b.database = Database.fromEngine(lib.settingsGetDatabase(s));
            b.suppressMask = lib.settingsGetSuppressHealthCheck(s);
            b.phasesMask = lib.settingsGetPhases(s);
            b.verbosity = Verbosity.fromCode(lib.settingsGetVerbosity(s));
            b.backend = Backend.fromCode(lib.settingsGetBackend(s));
            b.reportMultipleFailures = lib.settingsGetReportMultipleFailures(s);
            b.stopOnFirstFailure = lib.settingsGetStopOnFirstFailure(s);
            b.showStatistics = lib.settingsGetShowStatistics(s);
            b.unboundedChoices = lib.settingsGetUnboundedChoices(s);
            b.printBlob = lib.settingsGetPrintBlob(s);
            b.nondeterminismStrictness = NondeterminismStrictness.fromCode(lib.settingsGetNondeterminismStrictness(s));
            return b;
        } finally {
            lib.settingsFree(s);
        }
    }

    /** Return a copy of these settings with {@code mutator} applied to the changed fields. */
    private Settings with(Consumer<Builder> mutator) {
        Builder b = new Builder();
        b.testCases = testCases;
        b.hasSeed = hasSeed;
        b.seed = seed;
        b.derandomize = derandomize;
        b.database = database;
        b.suppressMask = suppressMask;
        b.phasesMask = phasesMask;
        b.verbosity = verbosity;
        b.backend = backend;
        b.reportMultipleFailures = reportMultipleFailures;
        b.stopOnFirstFailure = stopOnFirstFailure;
        b.showStatistics = showStatistics;
        b.unboundedChoices = unboundedChoices;
        b.printBlob = printBlob;
        b.nondeterminismStrictness = nondeterminismStrictness;
        b.reproduceFailure = reproduceFailure;
        b.annotated = annotated;
        b.name = name;
        b.testLocation = testLocation;
        b.infrastructurePackages = infrastructurePackages;
        b.examples = examples;
        b.stringSettings = stringSettings;
        mutator.accept(b);
        return new Settings(b);
    }

    /**
     * Mutable field holder used only to construct and copy {@link Settings}. The engine settings
     * are filled in by {@link #resolve}; the settings that identify one test start empty.
     */
    private static final class Builder {
        long testCases;
        boolean hasSeed;
        long seed;
        boolean derandomize;
        Database database;
        int suppressMask;
        int phasesMask;
        Verbosity verbosity;
        Backend backend;
        boolean reportMultipleFailures;
        boolean stopOnFirstFailure;
        boolean showStatistics;
        boolean unboundedChoices;
        boolean printBlob;
        NondeterminismStrictness nondeterminismStrictness;
        List<String> reproduceFailure = List.of();
        boolean annotated = false;
        String name = null;
        TestLocation testLocation = null;
        List<String> infrastructurePackages = List.of();
        List<Example> examples = List.of();
        List<Map.Entry<String, String>> stringSettings = List.of();
    }

    /**
     * Create settings resolved from the named profile instead of the default one: a reserved
     * ({@code base}, {@code default}), shipped ({@code development}, {@code ci}, {@code workload}),
     * {@code hegel.toml}-defined or {@link #registerProfile registered} profile. The settings
     * environment variables still apply over it, and the fluent methods replace what they resolve.
     *
     * @param name the profile to resolve
     * @return settings resolved from that profile
     * @throws IllegalArgumentException if the profile is unknown, or a {@code hegel.toml} or settings
     *     environment variable is malformed
     */
    public static Settings fromProfile(String name) {
        return resolved(Engine.get(), name);
    }

    /**
     * Register {@code settings} as the profile {@code name}, process-wide, replacing any earlier
     * registration of the same name. The profile is a complete snapshot of {@code settings};
     * registering a shipped profile's name replaces that profile, and a {@code hegel.toml} section
     * for {@code name} still merges over the snapshot. The settings that identify one test ({@link
     * #name(String)}, {@link #testLocation(TestLocation)}, {@link #reproduceFailure(String...)}) are
     * not part of a profile.
     *
     * <p>Registration is not retroactive: a {@code Settings} created before it keeps the values it
     * resolved, so register profiles before the settings that use them are created, for example
     * from a JUnit {@code @BeforeAll} or a {@code static} initializer. A {@code hegel.toml} is the
     * more robust place for a suite-wide profile.
     *
     * <pre>{@code
     * Settings.registerProfile("nightly", Settings.fromProfile("ci").testCases(10_000));
     * }</pre>
     *
     * @param name the profile name: ASCII letters, digits, {@code -} and {@code _}, and not one of
     *     the reserved {@code base} and {@code default}
     * @param settings the settings to snapshot
     * @throws IllegalArgumentException if the name is invalid or reserved
     */
    public static void registerProfile(String name, Settings settings) {
        Runner.registerProfile(Engine.get(), name, settings);
    }

    /**
     * Set the default profile for the whole process: the profile {@code new Settings()} resolves,
     * and that profiles without an explicit {@code extends} layer over. It takes precedence over
     * {@code HEGEL_DEFAULT_PROFILE}, the {@code default} entry in {@code hegel.toml} and the
     * engine's environment detection. The name need not exist yet; an empty name clears an earlier
     * call. Like {@link #registerProfile} it is not retroactive: a {@code Settings} created before
     * it keeps the values it resolved.
     *
     * @param name the profile, or {@code ""} to clear
     * @throws IllegalArgumentException if the name is invalid
     */
    public static void setDefaultProfile(String name) {
        Runner.setDefaultProfile(Engine.get(), name);
    }

    /**
     * Set the maximum number of valid test cases to run. The engine's profile or the {@code
     * HEGEL_TEST_CASES} environment variable gives the resolved value (100 in the shipped profiles).
     *
     * @param n the test-case budget
     * @return a new settings instance
     */
    public Settings testCases(long n) {
        if (n <= 0) {
            throw new IllegalArgumentException("testCases must be positive, got " + n);
        }
        return with(b -> b.testCases = n);
    }

    /**
     * Pin the RNG seed for a reproducible run. The engine's profile or the {@code HEGEL_SEED}
     * environment variable gives the resolved value (none in the shipped profiles).
     *
     * @param seed the seed: the engine reads its bits as an unsigned 64-bit value, so a negative
     *     {@code long} gives a seed of {@code 2^63} or more
     * @return a new settings instance
     */
    public Settings seed(long seed) {
        return with(b -> {
            b.hasSeed = true;
            b.seed = seed;
        });
    }

    /**
     * Force deterministic (or non-deterministic) input selection. The engine's profile
     * (deterministic in CI) or the {@code HEGEL_DERANDOMIZE} environment variable gives the resolved
     * value.
     *
     * @param derandomize whether to derive the seed deterministically
     * @return a new settings instance
     */
    public Settings derandomize(boolean derandomize) {
        return with(b -> b.derandomize = derandomize);
    }

    /**
     * Configure the example database. Pass {@link Database#defaultLocation()} to use the default
     * location, {@link Database#disabled()} to turn it off entirely, or {@link Database#path(String)}
     * to use a specific directory. The engine's profile (disabled in CI) or the {@code
     * HEGEL_DATABASE} environment variable gives the resolved value.
     *
     * @param database the database setting
     * @return a new settings instance
     */
    public Settings database(Database database) {
        return with(b -> b.database = database);
    }

    /**
     * Suppress exactly the listed health checks, replacing any previously set suppressions. The
     * engine's profile or the {@code HEGEL_SUPPRESS_HEALTH_CHECK} environment variable gives the
     * resolved suppressions (none in the shipped profiles); calling this with no checks runs every
     * health check even where the profile suppresses some.
     *
     * @param checks the checks to disable
     * @return a new settings instance
     */
    public Settings suppressHealthCheck(HealthCheck... checks) {
        int mask = 0;
        for (HealthCheck c : checks) {
            mask |= c.bit;
        }
        int suppressed = mask;
        return with(b -> b.suppressMask = suppressed);
    }

    /**
     * Enable only the listed phases; phases not listed are disabled. The engine's profile or the
     * {@code HEGEL_PHASES} environment variable gives the resolved phases (all of them in the
     * shipped profiles). With an empty argument list the run does nothing.
     *
     * @param phases the phases to enable
     * @return a new settings instance
     */
    public Settings phases(Phase... phases) {
        return with(b -> {
            b.phasesMask = 0;
            for (Phase p : phases) {
                b.phasesMask |= p.bit;
            }
        });
    }

    /**
     * Set engine output verbosity. The engine's profile or the {@code HEGEL_VERBOSITY} environment
     * variable gives the resolved value ({@link Verbosity#NORMAL} in the shipped profiles).
     *
     * @param verbosity the verbosity level
     * @return a new settings instance
     * @throws IllegalArgumentException for {@link Verbosity#DEFAULT}, which only {@link HegelTest}
     *     uses, to leave the resolved value in place
     */
    public Settings verbosity(Verbosity verbosity) {
        if (verbosity.code == null) {
            throw new IllegalArgumentException(
                    "Verbosity.DEFAULT is only for @HegelTest; Settings hold the resolved verbosity");
        }
        return with(b -> b.verbosity = verbosity);
    }

    /**
     * Select the source of randomness. The engine's profile or the {@code HEGEL_BACKEND} environment
     * variable gives the resolved value ({@link Backend#URANDOM} inside Antithesis and {@link
     * Backend#DEFAULT} otherwise).
     *
     * @param backend the randomness backend
     * @return a new settings instance
     * @throws IllegalArgumentException for {@link Backend#AUTO}, which only {@link HegelTest} uses,
     *     to leave the resolved value in place
     */
    public Settings backend(Backend backend) {
        if (backend.code == null) {
            throw new IllegalArgumentException(
                    "Backend.AUTO is only for @HegelTest; Settings hold the resolved backend");
        }
        return with(b -> b.backend = backend);
    }

    /**
     * Control whether the run reports every distinct failure it finds or only the smallest. The
     * engine's profile or the {@code HEGEL_REPORT_MULTIPLE_FAILURES} environment variable gives the
     * resolved value ({@code false} in the shipped profiles). When enabled, several distinct bugs
     * aggregate into one report (carrying the originals as suppressed exceptions); a run that finds
     * a single bug still rethrows it directly, preserving its type and stack trace.
     *
     * @param yes whether to report multiple failures
     * @return a new settings instance
     */
    public Settings reportMultipleFailures(boolean yes) {
        return with(b -> b.reportMultipleFailures = yes);
    }

    /**
     * Control whether the run stops generating as soon as it finds a failure. Turned off, a run
     * spends its whole test-case budget before shrinking and reporting, and replays every stored
     * failure rather than stopping at the first that still fails. The engine's profile or the
     * {@code HEGEL_STOP_ON_FIRST_FAILURE} environment variable gives the resolved value (on in the
     * shipped profiles).
     *
     * @param yes whether to stop at the first failure
     * @return a new settings instance
     */
    public Settings stopOnFirstFailure(boolean yes) {
        return with(b -> b.stopOnFirstFailure = yes);
    }

    /**
     * Print a statistics block at the end of the run: for each label recorded with an event, the
     * fraction of generated test cases it occurred in, and for each numeric observation a summary
     * of its distribution. The engine's profile or the {@code HEGEL_STATISTICS} environment
     * variable gives the resolved value (off in the shipped profiles).
     *
     * @param yes whether to print run statistics
     * @return a new settings instance
     */
    public Settings showStatistics(boolean yes) {
        return with(b -> b.showStatistics = yes);
    }

    /**
     * Let a test case make any number of choices. By default the engine concludes a test case as an
     * overrun once it has made 2<sup>20</sup> choices; a long-running test case, such as a
     * concurrent state machine driven for hours, needs the limit removed, at the cost of the memory
     * to record every choice. The engine's profile or the {@code HEGEL_UNBOUNDED_CHOICES}
     * environment variable gives the resolved value (off in the shipped profiles).
     *
     * @param yes whether to remove the per-test-case choice limit
     * @return a new settings instance
     */
    public Settings unboundedChoices(boolean yes) {
        return with(b -> b.unboundedChoices = yes);
    }

    /**
     * Print a copy-pasteable {@code reproduceFailure} line for each reported failure. The reproduce
     * blob is always attached to a failure; this only controls whether it is printed. The engine's
     * profile or the {@code HEGEL_PRINT_BLOB} environment variable gives the resolved value (on in
     * the shipped profiles).
     *
     * @param yes whether to print reproduce blobs with failures
     * @return a new settings instance
     */
    public Settings printBlob(boolean yes) {
        return with(b -> b.printBlob = yes);
    }

    /**
     * How the run reacts when it detects a nondeterministic test. The engine's profile or the
     * {@code HEGEL_NONDETERMINISM_STRICTNESS} environment variable gives the resolved value ({@link
     * NondeterminismStrictness#QUIET} in the shipped profiles).
     *
     * @param strictness the reaction
     * @return a new settings instance
     * @throws IllegalArgumentException for {@link NondeterminismStrictness#DEFAULT}, which only
     *     {@link HegelTest} uses, to leave the resolved value in place
     */
    public Settings nondeterminismStrictness(NondeterminismStrictness strictness) {
        if (strictness.code == null) {
            throw new IllegalArgumentException(
                    "NondeterminismStrictness.DEFAULT is only for @HegelTest; Settings hold the resolved strictness");
        }
        return with(b -> b.nondeterminismStrictness = strictness);
    }

    /**
     * Replay stored failures instead of running the property: each blob (from {@link
     * #printBlob(boolean)} output) is decoded and the test body re-run against the choices it
     * encodes, bypassing generation and shrinking. The engine replays each blob until a replay
     * fails, under a bounded budget: a deterministic blob a few times, a nondeterministic one's
     * recorded runs until one of them fails again. The run fails with every failure the blobs
     * reproduce, or throws a {@link DidNotReproduceException} if none of them does. A blob is only
     * guaranteed to reproduce under the Hegel version that produced it. Replaces any blobs given
     * earlier; with none, the property runs normally.
     *
     * @param blobs the base64 reproduce blobs
     * @return a new settings instance
     */
    public Settings reproduceFailure(String... blobs) {
        List<String> copy = List.of(blobs);
        return with(b -> b.reproduceFailure = copy);
    }

    /** These settings, as a {@link HegelTest} method's: its failures say how to replay them there. */
    Settings annotated() {
        return with(b -> b.annotated = true);
    }

    /**
     * Name this property: the key its failures are stored under in the example database, and the
     * seed of a derandomized run. {@link HegelTest} names a test after its class and method
     * ({@code com.example.CartTest.totalsAddUp}); a property run through {@link Hegel#test} has no
     * name, and so no database entry, unless given one here.
     *
     * @param name the test name
     * @return a new settings instance
     */
    public Settings name(String name) {
        return with(b -> b.name = name);
    }

    /**
     * Say where this property lives, for the engine to report its verdict under inside Antithesis.
     * By default {@link Hegel#test} and {@link Hegel#run} use their call site, and {@link HegelTest}
     * the first line of the annotated method's body (see {@link TestLocation}).
     *
     * @param location the test's location
     * @return a new settings instance
     */
    public Settings testLocation(TestLocation location) {
        return with(b -> b.testLocation = location);
    }

    /**
     * Class-name prefixes to treat as infrastructure when locating the user frame a failure was
     * thrown from. Hegel tells distinct bugs apart by the exception's type and the first stack
     * frame outside Hegel, the JDK, and JUnit; a frontend whose own frames sit between Hegel and
     * the user's code (for example {@code "clojure.lang."} and {@code "clojure.core"}) lists them
     * here so its frames are skipped too. Replaces any previously set prefixes.
     *
     * @param prefixes class-name prefixes to skip
     * @return a new settings instance
     */
    public Settings infrastructurePackages(String... prefixes) {
        List<String> copy = List.of(prefixes);
        return with(b -> b.infrastructurePackages = copy);
    }

    /**
     * Set a setting from its string form, as {@code hegel.toml} and the {@code HEGEL_*} environment
     * variables spell it: {@code key} is the setting's {@code hegel.toml} key ({@code test_cases},
     * {@code seed}, {@code phases}, {@code suppress_health_check}, ...) or {@code unbounded_choices}
     * or {@code database_key}, and {@code value} is in that key's vocabulary ({@code "500"}, {@code
     * "true"}, {@code "generate,shrink"}, ...). The engine parses it, so every engine setting is
     * reachable this way, which suits settings read from a command line or a configuration file.
     * String settings apply when the run starts, after the typed ones, in the order given, so a
     * later one wins.
     *
     * <pre>{@code
     * new Settings().set("test_cases", "500").set("suppress_health_check", "too_slow")
     * }</pre>
     *
     * @param key the setting's key
     * @param value its value
     * @return a new settings instance
     * @throws IllegalArgumentException for an unknown key or a value outside its vocabulary
     */
    public Settings set(String key, String value) {
        Runner.checkStringSetting(Engine.get(), key, value);
        List<Map.Entry<String, String>> copy = new ArrayList<>(stringSettings);
        copy.add(Map.entry(key, value));
        List<Map.Entry<String, String>> settings = List.copyOf(copy);
        return with(b -> b.stringSettings = settings);
    }

    /**
     * Run the property with these explicit {@link Example examples} before any values are
     * generated, in the {@link Phase#EXPLICIT explicit phase}. Replaces any previously set examples.
     * {@link Example} describes how a test body's draws take an example's values, and what the rest
     * of the test-case API does inside one.
     *
     * @param examples the examples
     * @return a new settings instance
     */
    public Settings examples(Example... examples) {
        List<Example> copy = List.of(examples);
        return with(b -> b.examples = copy);
    }

    /**
     * These settings with the values the engine holds once the string settings are applied, so a
     * run and its reporters see the effective configuration.
     */
    Settings effective(long testCases, boolean printBlob, NondeterminismStrictness strictness, Verbosity verbosity) {
        return with(b -> {
            b.testCases = testCases;
            b.verbosity = verbosity;
            b.printBlob = printBlob;
            b.nondeterminismStrictness = strictness;
        });
    }
}
