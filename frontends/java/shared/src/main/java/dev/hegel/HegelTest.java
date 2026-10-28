package dev.hegel;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import org.junit.jupiter.api.TestTemplate;
import org.junit.jupiter.api.extension.ExtendWith;

/**
 * Marks a JUnit 5 method as a Hegel property test.
 *
 * <p>The method must take a single {@link TestCase} parameter. The engine runs the body many times
 * with generated inputs, shrinking any failure to a minimal counterexample:
 *
 * <pre>{@code
 * @HegelTest
 * void additionCommutes(TestCase tc) {
 *   int x = tc.draw(integers());
 *   int y = tc.draw(integers());
 *   assertEquals(x + y, y + x);
 * }
 * }</pre>
 *
 * <p>The test appears as a single entry in the JUnit test tree. Every engine setting has a
 * corresponding attribute here (only a test's {@linkplain Settings#testLocation location}, its
 * {@linkplain Settings#infrastructurePackages infrastructure packages} and {@linkplain
 * Settings#set string-form settings} are {@link Settings}-only), so this annotation is the preferred
 * entry point for property tests:
 *
 * <pre>{@code
 * @HegelTest(testCases = 500, seed = 42, phases = {Phase.GENERATE, Phase.SHRINK})
 * void property(TestCase tc) { ... }
 * }</pre>
 *
 * <p>The escape hatch is {@link Hegel#test(java.util.function.Consumer, Settings)}, needed only
 * when a setting must come from a runtime value (annotation attributes are compile-time constants —
 * e.g. a {@code @TempDir} database path) or when a property is run outside a JUnit method (a {@code
 * main}, a helper that inspects the result).
 *
 * <p>The test is located (see {@link TestLocation}) at its method's source file and the first line
 * of the method's body that has code, since Java does not record the line a method is declared on,
 * and under the name of the test class running it.
 */
@Documented
@Target(ElementType.METHOD)
@Retention(RetentionPolicy.RUNTIME)
@TestTemplate
@ExtendWith(HegelTestExtension.class)
public @interface HegelTest {
    /** Sentinel for {@link #seed()} meaning "no fixed seed". */
    long NO_SEED = Long.MIN_VALUE;

    /**
     * Maximum number of valid test cases to run. The default, {@code 0}, leaves the budget the
     * engine's settings profile and the {@code HEGEL_TEST_CASES} environment variable resolve (100
     * unless configured otherwise).
     *
     * @return the test-case budget, or {@code 0} for the engine's
     */
    long testCases() default 0;

    /**
     * A fixed RNG seed for reproducibility, or {@link #NO_SEED} for none. The engine's seed is an
     * unsigned 64-bit value, read from this {@code long}'s bits; the one seed this attribute cannot
     * give is {@code 2^63}, whose bits are {@link #NO_SEED}'s, which {@link Settings#seed(long)} and
     * the {@code HEGEL_SEED} environment variable can.
     *
     * @return the seed
     */
    long seed() default NO_SEED;

    /**
     * The settings profile to resolve instead of the default one (see {@link
     * Settings#fromProfile(String)}). Empty (the default) uses the default profile.
     *
     * @return the profile name, or {@code ""} for the default profile
     */
    String profile() default "";

    /**
     * Engine output verbosity. The default leaves what the engine's settings profile and the {@code
     * HEGEL_VERBOSITY} environment variable resolve ({@link Verbosity#NORMAL} unless configured
     * otherwise).
     *
     * @return the verbosity level
     */
    Verbosity verbosity() default Verbosity.DEFAULT;

    /**
     * Force deterministic ({@link OptBoolean#TRUE}) or random ({@link OptBoolean#FALSE}) input
     * selection. The default leaves Hegel's environment-dependent behaviour (deterministic in CI,
     * random otherwise).
     *
     * @return the derandomize toggle
     */
    OptBoolean derandomize() default OptBoolean.DEFAULT;

    /**
     * Enable only the listed phases; phases not listed are disabled. The default (all phases) leaves
     * what the engine's settings profile and the {@code HEGEL_PHASES} environment variable resolve.
     * An explicitly empty list runs no phases at all.
     *
     * @return the phases to enable
     */
    Phase[] phases() default {Phase.EXPLICIT, Phase.REUSE, Phase.GENERATE, Phase.TARGET, Phase.SHRINK};

    /**
     * Health checks to suppress (because the flagged behaviour is intentional). Empty (the
     * default) leaves the suppressions the engine's settings profile and the {@code
     * HEGEL_SUPPRESS_HEALTH_CHECK} environment variable resolve. To run every check where the
     * profile suppresses some, give {@code suppressHealthCheck = HealthCheck.NONE}.
     *
     * @return the checks to disable
     */
    HealthCheck[] suppressHealthCheck() default {};

    /**
     * The example-database root, as a single value:
     *
     * <ul>
     *   <li>{@code ""} (default) — keep the database the settings resolved from the engine's
     *       profile.
     *   <li>{@link Database#DEFAULT_LOCATION} — use the default location, {@code .hegel/examples},
     *       even under a profile that disables the database.
     *   <li>{@link Database#DISABLED} — disable the database entirely (no persistence, no replay).
     *   <li>any other string — use that directory as the database root.
     * </ul>
     *
     * <p>A compile-time constant only — use {@link Hegel#test(java.util.function.Consumer, Settings)}
     * with {@link Database} for a runtime path (e.g. a {@code @TempDir}).
     *
     * @return the database setting
     */
    String database() default "";

    /**
     * The source of randomness. The default, {@link Backend#AUTO}, leaves what the engine's settings
     * profile and the {@code HEGEL_BACKEND} environment variable resolve: {@link Backend#URANDOM}
     * inside Antithesis and {@link Backend#DEFAULT} otherwise.
     *
     * @return the randomness backend
     */
    Backend backend() default Backend.AUTO;

    /**
     * Report every distinct failure the run finds rather than only the smallest. Several distinct
     * bugs aggregate into one report; a run that finds a single bug still rethrows it directly. The
     * default leaves what the engine's settings profile and the {@code
     * HEGEL_REPORT_MULTIPLE_FAILURES} environment variable resolve (off unless configured
     * otherwise).
     *
     * @return whether to report multiple failures
     */
    OptBoolean reportMultipleFailures() default OptBoolean.DEFAULT;

    /**
     * Stop generating at the first failure (see {@link Settings#stopOnFirstFailure(boolean)}). The
     * default leaves it to the engine's settings profile and the {@code HEGEL_STOP_ON_FIRST_FAILURE}
     * environment variable (on unless configured otherwise).
     *
     * @return whether to stop at the first failure
     */
    OptBoolean stopOnFirstFailure() default OptBoolean.DEFAULT;

    /**
     * Print run statistics at the end of the run (see {@link Settings#showStatistics(boolean)}).
     * The default leaves it to the engine's settings profile and the {@code HEGEL_STATISTICS}
     * environment variable (off unless configured otherwise).
     *
     * @return whether to print run statistics
     */
    OptBoolean showStatistics() default OptBoolean.DEFAULT;

    /**
     * Remove the per-test-case choice limit (see {@link Settings#unboundedChoices(boolean)}). The
     * default leaves it to the engine's settings profile (the limit applies unless configured
     * otherwise).
     *
     * @return whether to remove the choice limit
     */
    OptBoolean unboundedChoices() default OptBoolean.DEFAULT;

    /**
     * Print a copy-pasteable {@code reproduceFailure} line for each reported failure. The default
     * leaves it to the engine's settings profile and the {@code HEGEL_PRINT_BLOB} environment
     * variable (on unless configured otherwise).
     *
     * @return whether to print reproduce blobs with failures
     */
    OptBoolean printBlob() default OptBoolean.DEFAULT;

    /**
     * How the run reacts when it detects a nondeterministic test. The default leaves it to the
     * engine's settings profile and the {@code HEGEL_NONDETERMINISM_STRICTNESS} environment variable
     * ({@link NondeterminismStrictness#QUIET} unless configured otherwise).
     *
     * @return the reaction to nondeterminism
     */
    NondeterminismStrictness nondeterminismStrictness() default NondeterminismStrictness.DEFAULT;

    /**
     * Replay stored failure blobs (printed by {@link #printBlob}) instead of running the property:
     * the test body is re-run against exactly the choices each blob encodes, bypassing generation
     * and shrinking, and the test fails with every failure they reproduce (see {@link
     * Settings#reproduceFailure(String...)}). A single blob can be given as {@code reproduceFailure
     * = "..."}. Empty (the default) runs the property normally.
     *
     * @return the base64 reproduce blobs
     */
    String[] reproduceFailure() default {};

    /**
     * Name for this property: the key its failures are stored under in the example database.
     * Defaults to the fully-qualified name of the test class running it and the method name
     * ({@code com.example.CartTest.totalsAddUp}). A test method inherited by several test classes
     * gets a key per class, so their stored failures stay apart.
     *
     * @return the property name, or {@code ""} for the class and method name
     */
    String name() default "";

    /**
     * Methods of the test class supplying explicit {@link Example examples} to run before any
     * values are generated (see {@link Settings#examples(Example...)}). Each named method takes no
     * parameters and returns an {@link Example} or an {@link Iterable} of them; it may be static.
     *
     * <pre>{@code
     * static List<Example> edgeCases() {
     *     return List.of(Example.of(0), Example.of(Integer.MIN_VALUE));
     * }
     *
     * @HegelTest(examples = "edgeCases")
     * void absIsNonNegative(TestCase tc) { ... }
     * }</pre>
     *
     * @return the names of the methods supplying examples
     */
    String[] examples() default {};
}
