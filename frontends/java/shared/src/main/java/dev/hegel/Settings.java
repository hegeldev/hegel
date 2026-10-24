package dev.hegel;

import java.util.List;
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
 * <p>Whatever is not set here is resolved by the engine: from its settings profile — a {@code
 * hegel.toml} in the working directory or one of its parents (or the file named by {@code
 * HEGEL_CONFIG}), with {@link #setDefaultProfile(String)}, {@code HEGEL_DEFAULT_PROFILE} or {@code
 * hegel.toml} selecting the profile in effect, or the profile named by {@link #fromProfile(String)}
 * — and from the {@code HEGEL_*} settings environment variables, which win over the profile.
 * Settings given here win over both. The shipped profiles run 100 test cases, and in CI (which the
 * engine detects from {@code CI}, {@code GITHUB_ACTIONS} and similar variables) the {@code ci}
 * profile applies: runs are deterministic ({@code derandomize}) and the example database is
 * disabled.
 */
public final class Settings {
    final String profile; // null = the engine's default profile
    final Long testCases; // null = leave the engine's profile / HEGEL_TEST_CASES value
    final boolean hasSeed;
    final long seed;
    final Boolean derandomize;
    final Database database;
    final Integer suppressMask; // null = leave the engine's profile / HEGEL_* value
    final Integer phasesMask; // null = leave the engine default (all phases)
    final Verbosity verbosity;
    final Backend backend;
    // Each null Boolean leaves the engine's profile / environment value in place.
    final Boolean reportMultipleFailures;
    final Boolean stopOnFirstFailure;
    final Boolean showStatistics;
    final Boolean unboundedChoices;
    final Boolean printBlob;
    final NondeterminismStrictness nondeterminismStrictness;
    final String reproduceFailure; // null = run normally instead of replaying a blob
    final String name;
    final TestLocation testLocation; // null = locate the test where it is run from
    final List<String> infrastructurePackages;
    final List<Example> examples;

    /**
     * Create settings that leave everything to the engine's default profile and the environment
     * (100 test cases, all phases and a single reported failure in the shipped profiles).
     */
    public Settings() {
        this(new Builder());
    }

    private Settings(Builder b) {
        this.profile = b.profile;
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
        this.name = b.name;
        this.testLocation = b.testLocation;
        this.infrastructurePackages = b.infrastructurePackages;
        this.examples = b.examples;
    }

    /** Return a copy of these settings with {@code mutator} applied to the changed fields. */
    private Settings with(Consumer<Builder> mutator) {
        Builder b = new Builder();
        b.profile = profile;
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
        b.name = name;
        b.testLocation = testLocation;
        b.infrastructurePackages = infrastructurePackages;
        b.examples = examples;
        mutator.accept(b);
        return new Settings(b);
    }

    /** Mutable field holder used only to construct and copy {@link Settings}; holds the defaults. */
    private static final class Builder {
        String profile = null;
        Long testCases = null;
        boolean hasSeed = false;
        long seed = 0L;
        Boolean derandomize = null;
        Database database = Database.unset();
        Integer suppressMask = null;
        Integer phasesMask = null;
        Verbosity verbosity = Verbosity.DEFAULT;
        Backend backend = Backend.AUTO;
        Boolean reportMultipleFailures = null;
        Boolean stopOnFirstFailure = null;
        Boolean showStatistics = null;
        Boolean unboundedChoices = null;
        Boolean printBlob = null;
        NondeterminismStrictness nondeterminismStrictness = NondeterminismStrictness.DEFAULT;
        String reproduceFailure = null;
        String name = null;
        TestLocation testLocation = null;
        List<String> infrastructurePackages = List.of();
        List<Example> examples = List.of();
    }

    /**
     * Create settings that start from the named profile instead of the default one: a reserved
     * ({@code base}, {@code default}), shipped ({@code development}, {@code ci}, {@code workload}),
     * {@code hegel.toml}-defined or {@link #registerProfile registered} profile. The settings
     * environment variables still apply over it, and settings given on the result win over both.
     *
     * @param name the profile to start from
     * @return settings that resolve from that profile
     * @throws IllegalArgumentException if the profile is unknown, or a {@code hegel.toml} or settings
     *     environment variable is malformed
     */
    public static Settings fromProfile(String name) {
        Runner.checkProfile(Engine.get(), name);
        return new Settings().with(b -> b.profile = name);
    }

    /**
     * Register {@code settings} as the profile {@code name}, process-wide, replacing any earlier
     * registration of the same name. The profile is a complete snapshot of {@code settings} as the
     * engine resolves them now (its profile and the environment, then what {@code settings} sets);
     * registering a shipped profile's name replaces that profile, and a {@code hegel.toml} section
     * for {@code name} still merges over the snapshot. The settings that identify one test ({@link
     * #name(String)}, {@link #testLocation(TestLocation)}, {@link #reproduceFailure(String)}) are
     * not part of a profile.
     *
     * <p>Registration is not retroactive: register profiles before the tests that use them run,
     * for example from a JUnit {@code @BeforeAll} or a {@code static} initializer. A {@code
     * hegel.toml} is the more robust place for a suite-wide profile.
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
     * call. Like {@link #registerProfile} it is not retroactive.
     *
     * @param name the profile, or {@code ""} to clear
     * @throws IllegalArgumentException if the name is invalid
     */
    public static void setDefaultProfile(String name) {
        Runner.setDefaultProfile(Engine.get(), name);
    }

    /**
     * Set the maximum number of valid test cases to run. Unset, the engine's profile or the {@code
     * HEGEL_TEST_CASES} environment variable decides (100 in the shipped profiles).
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
     * Pin the RNG seed for a reproducible run.
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
     * Force deterministic (or non-deterministic) input selection regardless of the engine's profile
     * (deterministic in CI) and the {@code HEGEL_DERANDOMIZE} environment variable.
     *
     * @param derandomize whether to derive the seed deterministically
     * @return a new settings instance
     */
    public Settings derandomize(boolean derandomize) {
        return with(b -> b.derandomize = derandomize);
    }

    /**
     * Configure the example database. Pass {@link Database#unset()} to keep the engine default,
     * {@link Database#disabled()} to turn it off entirely, or {@link Database#path(String)} to use a
     * specific directory.
     *
     * @param database the database setting
     * @return a new settings instance
     */
    public Settings database(Database database) {
        return with(b -> b.database = database);
    }

    /**
     * Suppress exactly the listed health checks, replacing any previously set suppressions. Unless
     * this is called, the suppressions come from the engine's settings profile and the {@code
     * HEGEL_*} environment variables; calling it with no checks runs every health check even where
     * the profile suppresses some.
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
     * Enable only the listed phases; phases not listed are disabled. The default is all phases. With
     * an empty argument list the run does nothing.
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
     * Set engine output verbosity. Unset ({@link Verbosity#DEFAULT}), the engine's profile decides
     * ({@link Verbosity#NORMAL} in the shipped profiles).
     *
     * @param verbosity the verbosity level
     * @return a new settings instance
     */
    public Settings verbosity(Verbosity verbosity) {
        return with(b -> b.verbosity = verbosity);
    }

    /**
     * Select the source of randomness (default {@link Backend#AUTO}: the engine's settings profile
     * decides, {@link Backend#URANDOM} inside Antithesis and {@link Backend#DEFAULT} otherwise).
     *
     * @param backend the randomness backend
     * @return a new settings instance
     */
    public Settings backend(Backend backend) {
        return with(b -> b.backend = backend);
    }

    /**
     * Control whether the run reports every distinct failure it finds or only the smallest. Unset,
     * the engine's profile decides ({@code false} in the shipped profiles). When enabled, several
     * distinct bugs aggregate into one report (carrying the originals as suppressed exceptions); a
     * run that finds a single bug still rethrows it directly, preserving its type and stack trace.
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
     * failure rather than stopping at the first that still fails. Unset, the engine's profile or
     * the {@code HEGEL_STOP_ON_FIRST_FAILURE} environment variable decides (on in the shipped
     * profiles).
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
     * of its distribution. Unset, the engine's profile or the {@code HEGEL_STATISTICS} environment
     * variable decides (off in the shipped profiles).
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
     * to record every choice. Unset, the engine's profile decides (off in the shipped profiles).
     *
     * @param yes whether to remove the per-test-case choice limit
     * @return a new settings instance
     */
    public Settings unboundedChoices(boolean yes) {
        return with(b -> b.unboundedChoices = yes);
    }

    /**
     * Print a copy-pasteable {@code reproduceFailure} line for each reported failure. The reproduce
     * blob is always attached to a failure; this only controls whether it is printed. Unset, the
     * engine's profile or the {@code HEGEL_PRINT_BLOB} environment variable decides (on in the
     * shipped profiles).
     *
     * @param yes whether to print reproduce blobs with failures
     * @return a new settings instance
     */
    public Settings printBlob(boolean yes) {
        return with(b -> b.printBlob = yes);
    }

    /**
     * How the run reacts when it detects a nondeterministic test. Unset ({@link
     * NondeterminismStrictness#DEFAULT}), the engine's profile or the {@code
     * HEGEL_NONDETERMINISM_STRICTNESS} environment variable decides ({@link
     * NondeterminismStrictness#QUIET} in the shipped profiles).
     *
     * @param strictness the reaction
     * @return a new settings instance
     */
    public Settings nondeterminismStrictness(NondeterminismStrictness strictness) {
        return with(b -> b.nondeterminismStrictness = strictness);
    }

    /**
     * Replay a single stored failure instead of running the property: the blob (from {@link
     * #printBlob(boolean)} output) is decoded and the test body re-run against the choices it
     * encodes, bypassing generation and shrinking. The engine replays until a replay fails, under a
     * bounded budget: a deterministic blob a few times, a nondeterministic one's recorded runs until
     * one of them fails again. The run fails with the reproduced failure, or reports a stale blob if
     * none of the replays fails. A blob is only guaranteed to reproduce under the Hegel version that
     * produced it.
     *
     * @param blob the base64 reproduce blob
     * @return a new settings instance
     */
    public Settings reproduceFailure(String blob) {
        return with(b -> b.reproduceFailure = blob);
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
     * the annotated method.
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
     * These settings with the values the engine resolved for what was left unset, so a run and its
     * reporters see the effective configuration.
     */
    Settings resolved(long testCases, boolean printBlob, NondeterminismStrictness strictness, Verbosity verbosity) {
        return with(b -> {
            b.testCases = testCases;
            b.verbosity = verbosity;
            b.printBlob = printBlob;
            b.nondeterminismStrictness = strictness;
        });
    }
}
