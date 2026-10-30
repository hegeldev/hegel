package dev.hegel.lowlevel;

import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Iterator;
import java.util.List;
import java.util.ServiceLoader;
import java.util.function.Consumer;

/**
 * The libhegel binding surface, as a table of operations: one method per {@code hegel_*} function
 * in {@code hegel.h}.
 *
 * <p>This is the contract between a <em>binding</em> (an implementation over some FFI mechanism)
 * and a <em>frontend</em> (code that drives the engine). Hegel ships two bindings — the Foreign
 * Function and Memory API binding in {@code dev.hegel:hegel} and the JNA binding in {@code
 * dev.hegel:hegel-jna} — each registered as a {@link LibhegelBackend}; {@link #load()} picks up
 * whichever is on the classpath. Frontends may also substitute a fake for tests.
 *
 * <p>Opaque handles ({@code hegel_settings_t*}, {@code hegel_run_t*}, {@code hegel_test_case_t*},
 * {@code hegel_run_result_t*}, {@code hegel_string_generator_t*}, {@code hegel_date_options_t*},
 * {@code hegel_timezone_spec_t*}, {@code hegel_zoned_spec_t*}, {@code hegel_span_spec_t*}) are
 * passed as raw addresses
 * ({@code long}; {@code 0} is NULL); callers treat them as opaque and never dereference them.
 * Handles are caller-owned: every handle a method returns must be released with its matching
 * {@code *Free} method. A binding keeps one {@code hegel_context_t} per thread internally, so the
 * C API's leading context argument does not appear here; its last error message is read with
 * {@link #lastErrorMessage()}.
 *
 * <p>Two calling conventions coexist here, mirroring how a frontend consumes the ABI:
 *
 * <ul>
 *   <li>Per-test-case primitives (draws, spans, collections, pools, state machines, {@code target},
 *       {@code markComplete}) and the string-generator constructors return the raw libhegel return
 *       code ({@link Abi#OK}, {@link Abi#E_STOP_TEST}, {@link Abi#E_ASSUME}, ...); the frontend
 *       translates it and reads {@link #lastErrorMessage()} immediately on a non-OK code.
 *       Out-values are written into caller-supplied one-element arrays only on {@link Abi#OK}
 *       (except where noted).
 *   <li>Infrastructure calls (settings setters and getters, run lifecycle, result readers, frees)
 *       cannot legitimately fail with well-formed arguments, so implementations check the return
 *       code themselves and throw {@link LibhegelException} on an unexpected non-OK code. {@link
 *       #settingsNew} is the exception: the engine resolves the settings profile and the {@code
 *       HEGEL_*} environment variables while constructing the handle, so it follows the first
 *       convention and returns the raw code.
 * </ul>
 *
 * <p>Strings the engine returns are copied out before the method returns, so they remain valid.
 * Implementations are thread-safe; a process needs one instance.
 */
public interface Libhegel {
    /**
     * Resolve {@code libhegel} (see {@link LibraryLoader}) and open it with the binding registered
     * on the classpath.
     *
     * @return a binding over the resolved library
     * @throws LibhegelException if no library can be resolved or no binding is registered
     */
    static Libhegel load() {
        return load(LibraryLoader.fromEnvironment().resolve());
    }

    /**
     * Open the shared library at {@code library} with the binding registered on the classpath
     * (through {@link ServiceLoader}). With several registered, the first one found wins.
     *
     * @param library the path of the {@code libhegel} shared object
     * @return a binding over that library
     * @throws LibhegelException if no binding is registered
     */
    static Libhegel load(Path library) {
        return load(library, ServiceLoader.load(LibhegelBackend.class));
    }

    /**
     * Open the shared library at {@code library} with the first of {@code backends}.
     *
     * @param library the path of the {@code libhegel} shared object
     * @param backends candidate bindings, in preference order
     * @return a binding over that library
     * @throws LibhegelException if {@code backends} is empty
     */
    static Libhegel load(Path library, Iterable<? extends LibhegelBackend> backends) {
        Iterator<? extends LibhegelBackend> it = backends.iterator();
        if (!it.hasNext()) {
            throw new LibhegelException("No libhegel binding is registered on the classpath. Add dev.hegel:hegel"
                    + " (FFM, Java 22+) or dev.hegel:hegel-jna (JNA, Java 17+), or register your own"
                    + " dev.hegel.lowlevel.LibhegelBackend service provider.");
        }
        return it.next().open(library);
    }

    // Settings. Setters and getters cannot fail with this binding's inputs; implementations throw
    // on non-OK.

    /**
     * {@code hegel_settings_new}: a handle initialized from the engine's default settings profile
     * (a {@code hegel.toml} in the working directory or an ancestor, {@code HEGEL_DEFAULT_PROFILE},
     * and the shipped {@code development}/{@code ci}/{@code workload} profiles) with the {@code
     * HEGEL_TEST_CASES}, {@code HEGEL_DATABASE}, {@code HEGEL_STATISTICS}, {@code HEGEL_SEED},
     * {@code HEGEL_DERANDOMIZE}, {@code HEGEL_PRINT_BLOB} and {@code HEGEL_NONDETERMINISM_STRICTNESS}
     * environment variables applied over it.
     * Returns {@link Abi#E_INVALID_ARG} (with the message in {@link #lastErrorMessage()}) when a
     * {@code hegel.toml} or one of those variables is malformed; {@code out[0]} receives the handle
     * on {@link Abi#OK} and {@code 0} otherwise.
     */
    int settingsNew(long[] out);

    void settingsFree(long s);

    /** {@code hegel_settings_get_test_cases}: the resolved test-case budget. */
    long settingsGetTestCases(long s);

    /** {@code hegel_settings_get_print_blob}: whether the resolved settings print reproduce blobs. */
    boolean settingsGetPrintBlob(long s);

    /**
     * {@code hegel_settings_get_nondeterminism_strictness}: the resolved reaction to a
     * nondeterministic test ({@link Abi#NONDETERMINISM_QUIET}, {@link Abi#NONDETERMINISM_WARN} or
     * {@link Abi#NONDETERMINISM_ERROR}).
     */
    int settingsGetNondeterminismStrictness(long s);

    void settingsBackend(long s, int backend);

    void settingsTestCases(long s, long n);

    void settingsVerbosity(long s, int v);

    void settingsSeed(long s, long seed, boolean hasSeed);

    void settingsDerandomize(long s, boolean derandomize);

    void settingsReportMultipleFailures(long s, boolean yes);

    /**
     * {@code path == null} leaves the engine default; {@code ""} disables; otherwise sets the dir.
     */
    void settingsDatabase(long s, String path);

    void settingsDatabaseKey(long s, String key);

    void settingsPhases(long s, int mask);

    void settingsSuppressHealthCheck(long s, int mask);

    /** {@code hegel_settings_set_print_blob}: whether to print a reproduce blob per failure. */
    void settingsPrintBlob(long s, boolean yes);

    /**
     * {@code hegel_settings_set_nondeterminism_strictness}: how the run reacts when it detects a
     * test whose structure or outcome changes when the same choices are replayed — one of {@link
     * Abi#NONDETERMINISM_QUIET}, {@link Abi#NONDETERMINISM_WARN} or {@link Abi#NONDETERMINISM_ERROR}.
     */
    void settingsNondeterminismStrictness(long s, int strictness);

    // Run lifecycle.

    /**
     * Start a run. Engine output (progress lines, verbose traces) is delivered per line to {@code
     * output}; {@code null} leaves it on stderr. The callback stays registered until {@link
     * #runFree}.
     */
    long runStart(long settings, Consumer<String> output);

    /**
     * {@code hegel_run_start_blob}: start a run that replays a reproduce blob until a replay fails
     * (under the engine's bounded budget) instead of exploring. Driven exactly like a run from
     * {@link #runStart}: a reproducing replay is the run's failure and a run with no failures
     * means the blob is stale. A blob the engine cannot decode (corrupt, or from an incompatible
     * engine version) fails the call itself with {@code HEGEL_E_INVALID_ARG}, which the binding
     * throws as a {@link LibhegelException} carrying the engine's message; no run is started.
     * {@code output} has the same contract as in {@link #runStart}.
     */
    long runStartBlob(long settings, String blob, Consumer<String> output);

    /** The next test-case handle, or {@code 0} once the run is finished. */
    long nextTestCase(long run);

    /** A caller-owned snapshot of the finished run's result; release with {@link #runResultFree}. */
    long runResult(long run);

    void runResultFree(long result);

    void runFree(long run);

    /**
     * Replay a base64 reproduce blob as a standalone test case, in a single attempt. Returns the raw
     * rc ({@link Abi#E_INVALID_ARG} for a corrupt or incompatible blob); on OK, {@code out[0]}
     * receives the caller-owned handle. {@code output} has the same contract as in {@link #runStart}
     * but need not outlive the call. A nondeterministic blob may need several attempts to fail, so
     * reproduce-failure features should prefer {@link #runStartBlob}.
     */
    int testCaseFromBlob(long settings, String blob, Consumer<String> output, long[] out);

    void testCaseFree(long tc);

    /**
     * {@code hegel_test_case_should_capture}: whether the engine stamped this case for capture — the
     * caller should keep the case's output and, if it fails, its exception, keyed by the failure's
     * origin, because a stamped failing execution is the material for that origin's failure report.
     * Read once at case start.
     */
    boolean testCaseShouldCapture(long tc);

    /**
     * {@code hegel_test_case_clone}: a caller-owned handle onto an independent choice stream of
     * the same test case. The clone shares the case's outcome and budgets (and its collections,
     * pools and state machines, which are family-wide) but draws from its own sequence, so a clone
     * and its source can be driven concurrently from different threads while staying
     * deterministic under replay. Release it with {@link #testCaseFree}.
     *
     * <p>Cloning consumes one choice position on the source stream, so it is a draw: it returns
     * the raw rc — {@link Abi#E_STOP_TEST} when the choice budget is exhausted (as on a replay of
     * a shorter sequence), {@link Abi#E_CONCURRENT_USE} if the source is mid-operation on another
     * thread, {@link Abi#E_ALREADY_COMPLETE} once the case has completed — and on OK writes the
     * handle into {@code out[0]}.
     */
    int testCaseClone(long tc, long[] out);

    /**
     * {@code hegel_test_case_set_worker}: attribute the handle's engine-side output (notes and
     * printer lines) to concurrent worker {@code workerIndex}, which must be non-negative. Blocks
     * and clones derived from the handle afterwards inherit the attribution.
     */
    void testCaseSetWorker(long tc, long workerIndex);

    // Per-test-case draws. Each returns the raw rc.
    int generateBoolean(long tc, double p, boolean[] out);

    int generateInteger(long tc, long min, long max, long[] out);

    int generateFloat(
            long tc,
            int width,
            double min,
            double max,
            boolean allowNan,
            boolean allowInfinity,
            boolean excludeMin,
            boolean excludeMax,
            double smallestNonzeroMagnitude,
            double[] out);

    int generateBytes(long tc, long minSize, long maxSize, byte[][] out);

    int generateString(long tc, long generator, String[] out);

    /**
     * {@code hegel_generate_date}: {@code options} is a handle from {@link #dateOptionsNew}, or
     * {@code 0} for every weekday and month.
     */
    int generateDate(long tc, LocalDate min, LocalDate max, long options, LocalDate[] out);

    /** {@code precision} is one of the {@code Abi.PRECISION_*} constants. */
    int generateTime(long tc, LocalTime min, LocalTime max, int precision, LocalTime[] out);

    /** {@code precision} is one of the {@code Abi.PRECISION_*} constants. */
    int generateDatetime(long tc, LocalDateTime min, LocalDateTime max, int precision, LocalDateTime[] out);

    /**
     * {@code hegel_generate_timestamp}: an instant of whole {@code precision} units, as seconds since
     * the epoch and a nanosecond adjustment, which is {@link Instant}'s own model.
     */
    int generateTimestamp(long tc, Instant min, Instant max, int precision, Instant[] out);

    /**
     * {@code hegel_generate_offset}: {@code granularity} is one of the {@code
     * Abi.OFFSET_GRANULARITY_*} constants. On OK the offset in seconds east of UTC is written to
     * {@code outSeconds[0]} and whether it is known to {@code outKnown[0]}; the unknown offset (RFC
     * 3339's {@code -00:00}) is drawn only with {@code allowUnknown} and reads as 0 seconds.
     */
    int generateOffset(
            long tc,
            int minSeconds,
            int maxSeconds,
            int granularity,
            boolean allowUnknown,
            int[] outSeconds,
            boolean[] outKnown);

    /**
     * {@code hegel_generate_duration}: the engine's seconds and nanoseconds share a sign, and a
     * {@link Duration} is normalised on the way in and out, so {@code -1.5s} crosses as {@code {-1,
     * -500000000}}.
     */
    int generateDuration(long tc, Duration min, Duration max, int precision, Duration[] out);

    /**
     * {@code hegel_generate_timezone}: on OK the drawn IANA name, copied out of the engine's owned
     * string, is written to {@code out[0]}.
     */
    int generateTimezone(long tc, long spec, String[] out);

    /** A drawn zoned datetime: {@code hegel_zoned_datetime_t} plus the zone name the draw reports. */
    record ZonedDatetime(
            String zone,
            LocalDateTime civil,
            int offsetSeconds,
            boolean offsetKnown,
            boolean fold,
            int kind,
            int offsetBefore,
            int offsetAfter,
            Instant timestamp) {}

    /** {@code hegel_generate_zoned_datetime}: on OK the value is written to {@code out[0]}. */
    int generateZonedDatetime(long tc, long tzSpec, long zonedSpec, ZonedDatetime[] out);

    /**
     * {@code hegel_generate_span}: on OK the ten unit values of the {@code hegel_span_t}, years
     * first and nanoseconds last ({@code Abi.SPAN_UNIT_*} order), are written to {@code out10}.
     */
    int generateSpan(long tc, long spec, long[] out10);

    // Date options, time zone specs, zoned specs and span specs. Constructors write the new handle
    // (0 on failure) and return the raw rc; a setter that can reject its argument returns the raw rc
    // (INVALID_ARG leaves the spec as it was); the frees and the setters that cannot fail follow the
    // infrastructure convention.
    int dateOptionsNew(long[] out);

    void dateOptionsFree(long options);

    /** {@code mask} has bit 0 for Monday through bit 6 for Sunday. */
    int dateOptionsSetWeekdays(long options, int mask);

    /** {@code mask} has bit 0 for January through bit 11 for December. */
    int dateOptionsSetMonths(long options, int mask);

    int timezoneSpecNew(long[] out);

    void timezoneSpecFree(long spec);

    /** {@code hegel_timezone_spec_set_names}: each name exactly as the IANA database spells it. */
    int timezoneSpecSetNames(long spec, List<String> names);

    void timezoneSpecSetLinks(long spec, boolean links);

    void timezoneSpecSetLegacy(long spec, boolean legacy);

    int zonedSpecNew(long[] out);

    void zonedSpecFree(long spec);

    int zonedSpecSetCivilBounds(long spec, LocalDateTime min, LocalDateTime max);

    int zonedSpecSetInstantBounds(long spec, Instant min, Instant max);

    int zonedSpecSetPrecision(long spec, int precision);

    /** {@code imaginary} is {@code Abi.IMAGINARY_ALLOW} or {@code Abi.IMAGINARY_AVOID}. */
    int zonedSpecSetImaginary(long spec, int imaginary);

    /** {@code ambiguous} is one of the {@code Abi.AMBIGUOUS_*} constants. */
    int zonedSpecSetAmbiguous(long spec, int ambiguous);

    int spanSpecNew(long[] out);

    void spanSpecFree(long spec);

    /** {@code unit} is one of the {@code Abi.SPAN_UNIT_*} constants. */
    int spanSpecSetUnitBounds(long spec, int unit, long min, long max);

    /** {@code sign} is one of the {@code Abi.SPAN_SIGN_*} constants. */
    int spanSpecSetSign(long spec, int sign);

    /**
     * What {@code hegel_timezone_info} knows about a name: whether it is in the bundled database,
     * the primary zone it resolves to ({@code ""} when not known), whether it is a link, its shrink
     * rank and its {@code Abi.TIMEZONE_TAG_*} bits.
     */
    record TimezoneInfo(boolean known, String primaryName, boolean link, int rank, int tags) {}

    /** {@code hegel_timezone_info}: a query, never a draw; an unknown name reports {@code known} false. */
    TimezoneInfo timezoneInfo(String name);

    /** On OK writes the UUID's 16 big-endian bytes into {@code out16}. */
    int generateUuid(long tc, int version, boolean hasVersion, byte[] out16);

    /** On OK writes the address's 4 network-order bytes into {@code out4}. */
    int generateIpv4(long tc, byte[] out4);

    /** On OK writes the address's 16 network-order bytes into {@code out16}. */
    int generateIpv6(long tc, byte[] out16);

    // String-generator handles. Constructors return the raw rc (INVALID_ARG for a configuration
    // that is rejected, e.g. an empty alphabet with max_size > 0); handles are released with
    // stringGeneratorFree.
    int stringGeneratorText(
            long minSize,
            long maxSize,
            String codec,
            long minCodepoint,
            long maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeCharacters,
            String excludeCharacters,
            long[] out);

    /**
     * {@code hegel_string_generator_regex}. The pattern crosses as a length-delimited UTF-8 buffer,
     * so it may contain the NUL character, which Python's {@code re} accepts.
     */
    int stringGeneratorRegex(String pattern, boolean fullmatch, long alphabet, long[] out);

    int stringGeneratorEmail(long[] out);

    int stringGeneratorUrl(long[] out);

    int stringGeneratorDomain(long maxLength, long[] out);

    void stringGeneratorFree(long generator);

    // Structure: spans, collections, pools, state machines. Each returns the raw rc.
    int startSpan(long tc, long label);

    int stopSpan(long tc, boolean discard);

    int newCollection(long tc, long minSize, long maxSize, long[] outId);

    int collectionMore(long tc, long id, boolean[] outMore);

    int collectionReject(long tc, long id, String why);

    int newPool(long tc, long[] outId);

    int poolAdd(long tc, long poolId, long[] outVariableId);

    int poolGenerate(long tc, long poolId, boolean consume, long[] outVariableId);

    /**
     * {@code hegel_new_state_machine}: {@code ruleGroups} is parallel to {@code ruleNames} (any
     * value but {@link Abi#STATE_MACHINE_DONE}), as is {@code ruleWeights} — each rule's selection
     * weight relative to the other enabled rules of its group, finite and strictly positive, or
     * {@code null} for all-equal weights. {@code invariantAlwaysCheck} is parallel to {@code
     * invariantNames}. The engine draws the concurrency level in {@code [minConcurrency,
     * maxConcurrency]} and writes it to {@code outConcurrency}; pass {@code 1, 1} for a sequential
     * machine. {@code stepCount} is the target number of counted rounds per test case (at least 1;
     * the engine has no default, and 50 is the conventional choice).
     */
    int newStateMachine(
            long tc,
            List<String> ruleNames,
            long[] ruleGroups,
            double[] ruleWeights,
            List<String> invariantNames,
            boolean[] invariantAlwaysCheck,
            long minConcurrency,
            long maxConcurrency,
            long stepCount,
            long[] outId,
            long[] outConcurrency);

    /** The id of the group current for the new round, or {@link Abi#STATE_MACHINE_DONE}. */
    int stateMachineNextGroup(long tc, long stateMachineId, long[] outGroupId);

    /** {@code outRuleIndex[0]} receives the rule index, or {@link Abi#STATE_MACHINE_DONE}. */
    int stateMachineNextRule(long tc, long stateMachineId, long workerIndex, long[] outRuleIndex);

    int stateMachineRuleRejected(long tc, long stateMachineId, long workerIndex);

    int stateMachineShouldCheckInvariant(long tc, long stateMachineId, long invariantIndex, boolean[] outShouldCheck);

    /** {@code hegel_state_machine_free}: the handle is independent of its test case and run. */
    void stateMachineFree(long stateMachineId);

    int target(long tc, double value, String label);

    int markComplete(long tc, int status, String origin);

    // Results.
    int runResultStatus(long result);

    /** The run-level error message, or {@code null} when the run completed normally. */
    String runResultError(long result);

    long runResultFailureCount(long result);

    /**
     * The reproduce blob of the {@code index}-th distinct failure, or {@code null} if libhegel
     * produced none for it.
     */
    String failureBlob(long result, long index);

    /** The origin string the shrinker grouped the {@code index}-th distinct failure under. */
    String failureOrigin(long result, long index);

    /**
     * {@code hegel_failure_caveat}: the {@code index}-th distinct failure's confirmation caveat — its
     * standing under the run's nondeterministic handling, quoting the run's replay evidence — or
     * {@code null} for a deterministic failure.
     */
    String failureCaveat(long result, long index);

    // Diagnostics.
    String lastErrorMessage();

    String version();

    /** {@code hegel_tzdb_version}: the IANA release of the time zone database bundled in the engine. */
    String tzdbVersion();
}
