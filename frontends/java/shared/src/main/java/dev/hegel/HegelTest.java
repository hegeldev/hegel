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
 * <p>The test appears as a single entry in the JUnit test tree. Every {@link Settings} knob has a
 * corresponding attribute here, so this annotation is the preferred entry point for property tests:
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
     * Maximum number of valid test cases to run. The default, {@code 0}, leaves the budget to the
     * engine's settings profile and the {@code HEGEL_TEST_CASES} environment variable (100 unless
     * configured otherwise).
     *
     * @return the test-case budget, or {@code 0} for the engine's
     */
    long testCases() default 0;

    /**
     * A fixed RNG seed for reproducibility, or {@link #NO_SEED} for none.
     *
     * @return the seed
     */
    long seed() default NO_SEED;

    /**
     * The settings profile to start from instead of the default one (see {@link
     * Settings#fromProfile(String)}). Empty (the default) uses the default profile.
     *
     * @return the profile name, or {@code ""} for the default profile
     */
    String profile() default "";

    /**
     * Engine output verbosity. The default leaves it to the engine's settings profile ({@link
     * Verbosity#NORMAL} unless configured otherwise).
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
     * the engine default in place. An explicitly empty list runs no phases at all.
     *
     * @return the phases to enable
     */
    Phase[] phases() default {Phase.EXPLICIT, Phase.REUSE, Phase.GENERATE, Phase.TARGET, Phase.SHRINK};

    /**
     * Health checks to suppress (because the flagged behaviour is intentional).
     *
     * @return the checks to disable
     */
    HealthCheck[] suppressHealthCheck() default {};

    /**
     * The example-database root, as a single tri-state value:
     *
     * <ul>
     *   <li>{@code ""} (default) — leave the engine default in place.
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
     * The source of randomness. The default, {@link Backend#AUTO}, leaves the choice to the engine's
     * settings profile: {@link Backend#URANDOM} inside Antithesis and {@link Backend#DEFAULT} otherwise.
     *
     * @return the randomness backend
     */
    Backend backend() default Backend.AUTO;

    /**
     * Report every distinct failure the run finds rather than only the smallest. Several distinct
     * bugs aggregate into one report; a run that finds a single bug still rethrows it directly. The
     * default leaves it to the engine's settings profile (off unless configured otherwise).
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
     * Replay a stored failure blob (printed by {@link #printBlob}) instead of running the property:
     * the test body is re-run against exactly the choices the blob encodes, bypassing generation
     * and shrinking. Empty (the default) runs the property normally.
     *
     * @return the base64 reproduce blob, or {@code ""} for none
     */
    String reproduceFailure() default "";

    /**
     * Name for this property: the key its failures are stored under in the example database.
     * Defaults to the test's fully-qualified class name and method name ({@code
     * com.example.CartTest.totalsAddUp}).
     *
     * @return the property name, or {@code ""} for the class and method name
     */
    String name() default "";
}
