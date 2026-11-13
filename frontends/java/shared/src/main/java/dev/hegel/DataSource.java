package dev.hegel;

import dev.hegel.lowlevel.Libhegel;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.List;
import java.util.UUID;

/**
 * The per-test-case primitive surface that generators draw against.
 *
 * <p>Generators depend on this interface rather than {@link Libhegel} directly, so they can be
 * tested against a fake data source. Every method translates engine return codes: {@code STOP_TEST}
 * becomes {@link StopTest}, an assumption rejection becomes {@link AssumeRejected}, an invalid
 * argument becomes {@link IllegalArgumentException} carrying the engine's diagnostic, and any other
 * non-OK code becomes a {@link HegelException}.
 */
interface DataSource {
    boolean generateBoolean(double p);

    long generateInteger(long min, long max);

    double generateFloat(
            int width,
            double min,
            double max,
            boolean allowNan,
            boolean allowInfinity,
            boolean excludeMin,
            boolean excludeMax,
            double smallestNonzeroMagnitude);

    byte[] generateBytes(long minSize, long maxSize);

    String generateString(StringGeneratorHandle generator);

    /** {@code options} is a handle from {@link #dateOptions}, or {@code null} for every weekday and month. */
    LocalDate generateDate(LocalDate min, LocalDate max, SpecHandle options);

    /** {@code precision} is one of the {@code Abi.PRECISION_*} constants. */
    LocalTime generateTime(LocalTime min, LocalTime max, int precision);

    LocalDateTime generateDatetime(LocalDateTime min, LocalDateTime max, int precision);

    Instant generateTimestamp(Instant min, Instant max, int precision);

    /** A known offset in seconds east of UTC; {@code granularity} is an {@code Abi.OFFSET_GRANULARITY_*} constant. */
    int generateOffset(int minSeconds, int maxSeconds, int granularity);

    Duration generateDuration(Duration min, Duration max, int precision);

    // Each validate method throws the IllegalArgumentException its draw would for these arguments,
    // and draws nothing.

    void validateDate(LocalDate min, LocalDate max, SpecHandle options);

    void validateTime(LocalTime min, LocalTime max, int precision);

    void validateDatetime(LocalDateTime min, LocalDateTime max, int precision);

    void validateTimestamp(Instant min, Instant max, int precision);

    void validateOffset(int minSeconds, int maxSeconds, int granularity);

    void validateDuration(Duration min, Duration max, int precision);

    /** The drawn IANA name, from a spec built by {@link #timezoneSpec}. */
    String generateTimezone(SpecHandle spec);

    Libhegel.ZonedDatetime generateZonedDatetime(SpecHandle tzSpec, SpecHandle zonedSpec);

    /**
     * A zoned datetime in the zone {@code name} of the zone source {@code source} (a handle of this
     * binding), with no choice recorded for the zone. A name the source does not know is an {@link
     * IllegalArgumentException}; a zone with no value in the bounds rejects the test case.
     */
    Libhegel.ZonedDatetime generateZonedDatetimeForZone(long source, String name, SpecHandle zonedSpec);

    /** The ten unit values of the span, years first ({@code Abi.SPAN_UNIT_*} order). */
    long[] generateSpan(SpecHandle spec);

    // Engine configuration handles. A spec is validated as it is built, each setter checking its
    // own argument and the engine's validate call the settings against each other: a rejected
    // configuration throws IllegalArgumentException with the engine's diagnostic, and the handle is
    // released.

    /** A weekday or month mask of 0 leaves that restriction unset. */
    SpecHandle dateOptions(int weekdayMask, int monthMask);

    /**
     * {@code source} is a zone source handle of this binding, or 0 for the process default; {@code
     * names} is an explicit list, or {@code null} for every name of the source the flags admit.
     */
    SpecHandle timezoneSpec(List<String> names, boolean links, boolean legacy, long source);

    /**
     * Exactly one pair of bounds is non-null: civil (wall-clock) or instant. {@code imaginary} is an
     * {@code Abi.IMAGINARY_*} constant and {@code ambiguous} an {@code Abi.AMBIGUOUS_*} one.
     */
    SpecHandle zonedSpec(
            LocalDateTime civilMin,
            LocalDateTime civilMax,
            Instant instantMin,
            Instant instantMax,
            int precision,
            int imaginary,
            int ambiguous);

    /**
     * {@code units} are {@code Abi.SPAN_UNIT_*} constants with {@code mins} and {@code maxs}
     * parallel to them; {@code sign} is an {@code Abi.SPAN_SIGN_*} constant.
     */
    SpecHandle spanSpec(int[] units, long[] mins, long[] maxs, int sign);

    /** Whether {@code spec} was built by the binding behind this source; see {@link #ownsStringGenerator}. */
    boolean ownsSpec(SpecHandle spec);

    /** {@code hegel_timezone_info_in}: a query of {@code source} (0 for the process default), not a draw. */
    Libhegel.TimezoneInfo timezoneInfo(long source, String name);

    /** The binding behind this source, which engine handles used with it must come from. */
    Libhegel binding();

    UUID generateUuid(Integer version);

    byte[] generateIpv4();

    byte[] generateIpv6();

    // String-generator handle construction. Parameters are validated eagerly: a rejected
    // configuration throws IllegalArgumentException with the engine's diagnostic.
    StringGeneratorHandle textGenerator(
            long minSize,
            long maxSize,
            String codec,
            long minCodepoint,
            long maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeCharacters,
            String excludeCharacters);

    StringGeneratorHandle regexGenerator(String pattern, boolean fullmatch, StringGeneratorHandle alphabet);

    StringGeneratorHandle emailGenerator();

    StringGeneratorHandle urlGenerator();

    StringGeneratorHandle domainGenerator(long maxLength);

    /**
     * Whether {@code generator} was built by the binding behind this source. A cached handle from
     * another binding (e.g. after a test swapped the {@link Engine}) must be rebuilt, not drawn
     * from.
     */
    boolean ownsStringGenerator(StringGeneratorHandle generator);

    void startSpan(long label);

    void stopSpan(boolean discard);

    long newCollection(long minSize, long maxSize);

    boolean collectionMore(long id);

    void collectionReject(long id, String why);

    long newPool();

    long poolAdd(long poolId);

    long poolGenerate(long poolId, boolean consume);

    // Stateful testing. The root source registers the machine, advances rounds and samples
    // invariants; at concurrency > 1 each worker pulls its rules through its own clone.

    /**
     * An empty state machine with its concurrency bounds and step count: its handle. Declare the
     * rules and invariants with {@link #stateMachineAddRule} and {@link #stateMachineAddInvariant},
     * then seal them with {@link #stateMachineStart}.
     */
    long newStateMachine(long minConcurrency, long maxConcurrency, int stepCount);

    /**
     * Declare the next rule (index = the number of rules declared before it): its selection weight
     * (finite and positive) and the ids of the concurrency groups it belongs to (at least one).
     */
    void stateMachineAddRule(long stateMachineId, String name, double weight, long[] groups);

    /** Declare the next invariant (index = the number declared before it); always checked, or sampled. */
    void stateMachineAddInvariant(long stateMachineId, String name, boolean alwaysCheck);

    /**
     * Seal the declared rules and invariants: the number of workers to run, {@code maxConcurrency}.
     */
    int stateMachineStart(long stateMachineId);

    /**
     * Start the next round: its group id, or {@link Abi#STATE_MACHINE_DONE} when the machine is done.
     * On a new round {@code outConcurrency[0]} receives the round's concurrency level.
     */
    long stateMachineNextGroup(long stateMachineId, long[] outConcurrency);

    /**
     * The next rule index for worker {@code workerIndex} this round, or {@link
     * Abi#STATE_MACHINE_DONE} at the worker's join point.
     */
    long stateMachineNextRule(long stateMachineId, long workerIndex);

    /** The rule most recently handed to worker {@code workerIndex} failed an assumption: retry the slot. */
    void stateMachineRuleRejected(long stateMachineId, long workerIndex);

    /**
     * A source over an independent choice stream of the same test case ({@code
     * hegel_test_case_clone}), attributed to concurrent worker {@code workerIndex}, for a worker
     * thread to draw through. Release it with {@link #release()} once the worker is done with it.
     */
    DataSource cloneForWorker(long workerIndex);

    /** Free the handle behind a source from {@link #cloneForWorker}. Safe once the case is aborted. */
    void release();

    /** The engine's sampling decision for invariant {@code invariantIndex} at this join point. */
    boolean stateMachineShouldCheckInvariant(long stateMachineId, long invariantIndex);

    /** Release the machine handle. Safe once the case is aborted. */
    void stateMachineFree(long stateMachineId);

    /** Pause fault injection on this case until {@link #unpauseFaults} ({@code hegel_pause_faults}). */
    void pauseFaults();

    /**
     * Resume fault injection paused by {@link #pauseFaults}. A no-op once the case is aborted, so
     * the unpausing {@code finally} of an invariant that overran is safe.
     */
    void unpauseFaults();

    /** Whether the case has been concluded (overrun or invalid) and its primitives now short-circuit. */
    boolean isAborted();

    void target(double value, String label);
}
