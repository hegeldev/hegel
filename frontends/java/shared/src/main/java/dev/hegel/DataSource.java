package dev.hegel;

import dev.hegel.lowlevel.Libhegel;
import java.math.BigInteger;
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

    /** An integer in {@code [min, max]}, of any magnitude ({@code hegel_generate_integer_big} when outside {@code long}). */
    BigInteger generateIntegerBig(BigInteger min, BigInteger max);

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

    LocalDate generateDate(LocalDate min, LocalDate max);

    LocalTime generateTime(LocalTime min, LocalTime max);

    LocalDateTime generateDatetime(LocalDateTime min, LocalDateTime max);

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

    /** Release a collection handle from {@link #newCollection}. Safe once the case is aborted. */
    void collectionFree(long id);

    /**
     * Create a pool. The pool is family-wide: it is released by {@link #releasePools()} on the
     * root source once the test case is over, wherever in the family it was created.
     */
    long newPool();

    long poolAdd(long poolId);

    long poolGenerate(long poolId, boolean consume);

    /**
     * Put back a variable a consuming {@link #poolGenerate} removed ({@code hegel_pool_restore}). A
     * no-op once the case is aborted: nothing will draw from the pool again.
     */
    void poolRestore(long poolId, long variableId);

    // Recursion (hegel_recursion_t). A recursive value is drawn in attempts; see RecursiveGenerator.

    long newRecursion(long maxDepth, long maxLeaves);

    /** Whether the sub-value at {@code depth} is a branch (else a leaf). */
    boolean recursionBranch(long recursion, long depth);

    /**
     * Count a leaf against the attempt's budget: {@code false} when the attempt has outgrown it and
     * must unwind to {@link #recursionRetry}. Until then, spans are left open for the engine to
     * close.
     */
    boolean recursionLeaf(long recursion);

    /** Discard an attempt that outgrew its budget; rejects the case when the attempts are exhausted. */
    void recursionRetry(long recursion);

    /** Report the value complete: {@code false} when the engine discarded the attempt as mispriced. */
    boolean recursionFinish(long recursion);

    /** Release the recursion handle. Safe once the case is aborted. */
    void recursionFree(long recursion);

    // Stateful testing. The root source registers the machine, advances rounds and samples
    // invariants; at concurrency > 1 each worker pulls its rules through its own clone.

    /** What registering a state machine yields: its handle and the concurrency level the engine drew. */
    record StateMachine(long id, int concurrency) {}

    /**
     * {@code ruleGroups} and {@code ruleWeights} are parallel to {@code ruleNames} ({@code null}
     * weights = all equal); {@code invariantAlwaysCheck} is parallel to {@code invariantNames}.
     * The engine draws the concurrency level in {@code [minConcurrency, maxConcurrency]}.
     */
    StateMachine newStateMachine(
            List<String> ruleNames,
            long[] ruleGroups,
            double[] ruleWeights,
            List<String> invariantNames,
            boolean[] invariantAlwaysCheck,
            long minConcurrency,
            long maxConcurrency,
            int stepCount);

    /** Start the next round: its group id, or {@link Abi#STATE_MACHINE_DONE} when the machine is done. */
    long stateMachineNextGroup(long stateMachineId);

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

    /**
     * A source over a block handle ({@code hegel_test_case_block}): the <em>same</em> choice stream
     * as this one, whose output lands in a block nested in this source's print region, every line
     * {@code indent} columns further in. It shares this source's abort state. Release it with
     * {@link #release()}.
     */
    DataSource block(long indent);

    /** Free the handle behind a source from {@link #cloneForWorker} or {@link #block}. Safe once the case is aborted. */
    void release();

    /** Append {@code message} to this source's print region ({@code hegel_note}). */
    void note(String message);

    /** A caller-owned printer onto this source's print region ({@code hegel_test_case_printer}). */
    Printer printer();

    /** The engine's sampling decision for invariant {@code invariantIndex} at this join point. */
    boolean stateMachineShouldCheckInvariant(long stateMachineId, long invariantIndex);

    /** Release the machine handle. Safe once the case is aborted. */
    void stateMachineFree(long stateMachineId);

    /**
     * Release every pool created through this source's test-case family. Called once, on the root
     * source, when the test case is over. Safe once the case is aborted.
     */
    void releasePools();

    /**
     * Release the calling thread's engine error context. Called by a worker thread that is about
     * to exit, so its context does not outlive it.
     */
    void releaseThreadContext();

    /** Whether the case has been concluded (overrun or invalid) and its primitives now short-circuit. */
    boolean isAborted();

    void target(double value, String label);

    /** Record an event for the statistics report ({@code hegel_event}). */
    void event(String label);

    /** Record a numeric observation for the statistics report ({@code hegel_event_value}). */
    void eventValue(String label, double value);
}
