package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.LibhegelException;
import java.math.BigInteger;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.function.Function;
import java.util.function.IntFunction;
import java.util.function.Supplier;

/**
 * The handle a property test body uses to draw values and steer the engine.
 *
 * <p>An instance is supplied to the test body for each case the engine runs. Draw values with
 * {@link #draw(Generator)}, reject uninteresting inputs with {@link #assume(boolean)}, attach debug
 * context with {@link #note(String)}, and guide the search with {@link #target(double)}.
 *
 * <p>On an execution the engine stamped for capture ({@link #isFinal()}) — the final replay of a
 * minimal failing example among them — and on every execution of a {@linkplain Verbosity#VERBOSE
 * verbose} run, each top-level {@code draw} is printed as {@code x = 42;} and each note as its
 * text into the engine's document for the case, which lays out long values over several lines and
 * attributes concurrent workers' lines. If the case fails and the engine reports that failure, the
 * run's {@link Reporter} receives the rendered lines and the resulting {@link Failure} carries
 * them, so the counterexample is readable. A label drawn more than once in a case is numbered from
 * its second use ({@code x}, {@code x_2}, {@code x_3}); unlabelled draws are {@code draw_1}, {@code
 * draw_2}, ...
 *
 * <p>Frontends implementing their own composite generators enclose their draws in a labelled
 * {@link #span(long, Supplier) span} so the engine can shrink the structure they build.
 */
public final class TestCase {
    /** The handle this instance draws through; {@link #repeat} swaps in a block for each repetition. */
    private DataSource source;
    /** What every handle of one test case shares: the capture decision, draw names and draws. */
    private final Shared shared;
    /** The example an explicit test case draws from; {@code null} for a generated one. */
    private final ExplicitDraws explicit;
    /** This handle's printer onto its own print region, fetched on its first printed draw. */
    private Printer printer;

    private int drawDepth;

    /** The state every handle of one test case shares, whichever thread it is driven from. */
    private static final class Shared {
        /** Whether the engine stamped this execution for capture: its draws are recorded. */
        final boolean captured;
        /** Whether draws and notes are printed to the case's document at all. */
        final boolean emits;
        /** Uses per draw name, for numbering repeats ({@code x}, {@code x_2}, ...; {@code draw_N}). */
        private final Map<String, Integer> nameUses = new HashMap<>();

        private final Map<String, Object> draws = new LinkedHashMap<>();
        /** Every block and fork handle opened in the case, kept alive until the case is over. */
        private final List<TestCase> blocks = new ArrayList<>();

        Shared(boolean captured, boolean verbose) {
            this.captured = captured;
            this.emits = captured || verbose;
        }

        /**
         * The name a top-level draw reports under: a label prints bare the first time and numbered
         * from its second use ({@code x}, {@code x_2}, ...); an unlabelled draw is {@code draw_N},
         * counting unlabelled draws only. Names are unique across the whole case, so concurrent
         * workers name their draws through here too.
         */
        synchronized String nextName(String label) {
            String base = label != null ? label : "draw";
            int uses = nameUses.merge(base, 1, Integer::sum);
            if (label == null) {
                return base + "_" + uses;
            }
            return uses == 1 ? label : label + "_" + uses;
        }

        synchronized void record(String name, Object value) {
            if (captured) {
                draws.put(name, value);
            }
        }

        synchronized Map<String, Object> draws() {
            return new LinkedHashMap<>(draws);
        }

        synchronized void addBlock(TestCase block) {
            blocks.add(block);
        }

        synchronized List<TestCase> takeBlocks() {
            List<TestCase> taken = new ArrayList<>(blocks);
            blocks.clear();
            return taken;
        }
    }

    TestCase(DataSource source, boolean captured) {
        this(source, captured, false);
    }

    TestCase(DataSource source, boolean captured, boolean verbose) {
        this(source, new Shared(captured, verbose));
    }

    private TestCase(DataSource source, Shared shared) {
        this(source, shared, null);
    }

    private TestCase(DataSource source, Shared shared, ExplicitDraws explicit) {
        this.source = source;
        this.shared = shared;
        this.explicit = explicit;
    }

    /** A test case whose draws take the values of an explicit example instead of drawing them. */
    static TestCase explicit(ExplicitDraws draws) {
        return new TestCase(ExplicitDraws.noEngine(), new Shared(true, false), draws);
    }

    /**
     * A handle for concurrent worker {@code worker} to draw through for one round of a stateful
     * machine: an independent choice stream of the same case (see {@link
     * DataSource#cloneForWorker}) whose output the engine stamps {@code [worker N +X.XXXms]} and
     * anchors where the handle was made. Release it with {@link #release()} once the round is over.
     */
    TestCase forWorker(int worker) {
        return new TestCase(source.cloneForWorker(worker), shared);
    }

    /**
     * A handle for an indented section of this one's output, such as a stateful rule's body under
     * its heading: it draws from this handle's own choice stream (so the two must not be driven
     * concurrently) and everything it prints lands in a block {@code indent} columns further in.
     * The handle stays valid until the case is over — a {@link Pool} made inside a rule keeps
     * drawing through it — and is freed by {@link #releaseBlocks()}.
     */
    TestCase block(long indent) {
        TestCase block = new TestCase(source.block(indent), shared);
        shared.addBlock(block);
        return block;
    }

    /** Free a handle from {@link #forWorker}, and its printer. */
    void release() {
        try {
            closePrinter();
        } finally {
            source.release();
        }
    }

    /** Free every block and fork handle opened in the case, and their printers. Called once the case is over. */
    void releaseBlocks() {
        for (TestCase block : shared.takeBlocks()) {
            block.release();
        }
    }

    private void closePrinter() {
        if (printer != null) {
            Printer p = printer;
            printer = null;
            p.close();
        }
    }

    /**
     * The case's rendered output, one entry per line: every draw and note printed through any of
     * its handles, laid out by the engine. Reading seals the document, so this is called once, on
     * the root handle, after the body has finished. Empty when nothing was printed (the execution
     * was neither stamped for capture nor verbose).
     */
    List<String> output() {
        closePrinter();
        if (!shared.emits) {
            return List.of();
        }
        String text;
        try (Printer root = source.printer()) {
            text = root.read();
        }
        return text.lines().toList();
    }

    /**
     * Whether the engine stamped this execution as one a failure report can be built from — the
     * final replay of a minimal counterexample (or of a {@link Settings#reproduceFailure(String...)}
     * blob), the replays that confirm a discovered failure — as opposed to one of the many cases it
     * runs while generating and shrinking. Draws and notes are recorded only on a stamped
     * execution (or on every execution of a verbose run), and reported only if it fails and the
     * engine reports that failure; a test body can use this to do its own expensive diagnostics
     * only where they may be seen.
     *
     * @return {@code true} on an execution stamped for capture
     */
    public boolean isFinal() {
        return shared.captured;
    }

    /**
     * Draw a value from {@code generator}.
     *
     * @param generator the generator to draw from
     * @param <T> the value type
     * @return the generated value
     */
    public <T> T draw(Generator<T> generator) {
        return draw(generator, null);
    }

    /**
     * Draw a value, naming it {@code label} in the falsifying-example output.
     *
     * @param generator the generator to draw from
     * @param label the variable name to show in counterexample output
     * @param <T> the value type
     * @return the generated value
     */
    public <T> T draw(Generator<T> generator, String label) {
        if (explicit != null) {
            return explicitDraw(label);
        }
        if (drawDepth > 0 || !shared.emits) {
            return drawNested(generator);
        }
        Printer p = printer();
        p.beginSpeculative();
        boolean committed = false;
        try {
            T value = drawNested(generator);
            String name = shared.nextName(label);
            shared.record(name, value);
            p.text(name + " = ");
            p.value(value);
            p.text(";");
            p.hardBreak();
            p.commitSpeculative();
            committed = true;
            return value;
        } finally {
            if (!committed) {
                p.abortSpeculative();
            }
        }
    }

    /**
     * Draw a value from {@code generator} without printing it: it is neither shown in the
     * counterexample output nor recorded in {@link Failure#draws()}.
     *
     * @param generator the generator to draw from
     * @param <T> the value type
     * @return the generated value
     */
    @SuppressWarnings("unchecked")
    public <T> T drawSilent(Generator<T> generator) {
        if (explicit != null) {
            return (T) explicit.takeSilent();
        }
        return drawNested(generator);
    }

    @SuppressWarnings("unchecked")
    private <T> T explicitDraw(String label) {
        String name = shared.nextName(label);
        T value = (T) explicit.take(name);
        shared.record(name, value);
        return value;
    }

    private <T> T drawNested(Generator<T> generator) {
        drawDepth++;
        try {
            return generator.doDraw(this);
        } finally {
            drawDepth--;
        }
    }

    private Printer printer() {
        if (printer == null) {
            printer = source.printer();
        }
        return printer;
    }

    /**
     * Reject the current test case unless {@code condition} holds. The engine discards it without
     * counting it against the test-case budget and tries another input.
     *
     * @param condition the precondition that must hold
     */
    public void assume(boolean condition) {
        if (!condition) {
            throw new AssumeRejected();
        }
    }

    /**
     * Reject the current test case unconditionally, as {@code assume(false)} does: this never
     * returns.
     */
    public void reject() {
        throw new AssumeRejected();
    }

    /**
     * Record a debug message, reported with the counterexample when this case's failure is
     * reported (and on every case under {@link Verbosity#VERBOSE}). A note made while a top-level
     * draw is in progress — from inside a composite generator — is reported after that draw's
     * value, never before it. A message may span several lines.
     *
     * @param message the message to record
     */
    public void note(String message) {
        if (explicit != null) {
            explicit.note(message);
        } else if (shared.emits) {
            source.note(message);
        }
    }

    /**
     * Provide a score for the coverage-guided search; higher is treated as more interesting.
     *
     * @param value the observation
     */
    public void target(double value) {
        target(value, "");
    }

    /**
     * Provide a labelled score for the coverage-guided search.
     *
     * @param value the observation
     * @param label groups observations for multi-objective search
     */
    public void target(double value, String label) {
        source.target(value, label);
    }

    /**
     * Record an event for the end-of-run statistics report. With statistics on ({@link
     * Settings#showStatistics(boolean)}, or the {@code HEGEL_STATISTICS} environment variable), the
     * report shows, per label, the fraction of test cases in which the label was recorded at least
     * once: a quick check that the interesting situations in a test actually occur. With
     * statistics off, events cost almost nothing and report nothing.
     *
     * @param label the event's label
     */
    public void event(String label) {
        source.event(label);
    }

    /**
     * Record a numeric observation under {@code label} for the end-of-run statistics report. With
     * statistics on, the report shows, per label, a summary of the observed distribution (count,
     * min, median, mean, p90, max). Unlike {@link #target(double, String)}, a label may be observed
     * any number of times per test case, and observations do not steer generation.
     *
     * @param label the observation's label
     * @param value the observation; must be finite
     */
    public void eventValue(String label, double value) {
        source.eventValue(label, value);
    }

    /**
     * A handle for drawing from another thread: an independent stream of choices of this same test
     * case, whose output the engine attributes to worker {@code worker} ({@code [worker N
     * +X.XXXms]}). Draws on one handle never perturb the values another produces, so several threads
     * can generate concurrently and the test stays deterministic: the same seed replays the same
     * values on every stream, and failures shrink and replay normally. A single handle must only be
     * driven by one thread at a time, so fork one per thread.
     *
     * <p>The forks share the test case's outcome: an exception on a fork's thread is a failure only
     * once the test body rethrows it, typically by joining the thread. Forks are released when the
     * test case ends. Forking is itself a draw.
     *
     * @param worker the worker index the engine attributes this handle's output to
     * @return the forked handle
     */
    public TestCase fork(int worker) {
        if (explicit != null) {
            return new TestCase(source, shared, explicit);
        }
        TestCase fork = forWorker(worker);
        shared.addBlock(fork);
        return fork;
    }

    /**
     * Run {@code body} over and over until the engine decides the test case is long enough, then end
     * the test case: it never returns normally. Use it for tests that are logically infinite loops,
     * such as driving a system with a stream of random operations, with better shrinking than a
     * hand-written loop: the engine can delete individual repetitions. Each repetition is introduced
     * in the output by a {@code // Repetition #N} note, with its own output indented under it. A
     * repetition that fails an {@link #assume assumption} is skipped; any other exception ends the
     * test case.
     *
     * @param body one repetition, drawing through this test case
     */
    public void repeat(Runnable body) {
        if (explicit != null) {
            note("// Repetition #1");
            body.run();
            throw new LoopDone();
        }
        long minSize = drawSilent(Generators.longs().min(0).max(1L << 40));
        long id = newCollection(minSize, Abi.UNBOUNDED);
        try {
            long repetition = 0;
            while (collectionMore(id)) {
                repetition++;
                note("// Repetition #" + repetition);
                repeatOnce(body);
            }
        } finally {
            collectionFree(id);
        }
        throw new LoopDone();
    }

    private void repeatOnce(Runnable body) {
        Throwable failure;
        DataSource outer = enterBlock(2);
        try {
            body.run();
            return;
        } catch (Throwable e) {
            failure = e;
        } finally {
            swapSource(outer);
        }
        if (failure instanceof AssumeRejected && !isAborted()) {
            return;
        }
        if (!isControl(failure)) {
            drawSilent(Generators.booleans());
        }
        throw rethrow(failure);
    }

    private static boolean isControl(Throwable e) {
        return e instanceof StopTest
                || e instanceof AssumeRejected
                || e instanceof LoopDone
                || e instanceof IllegalArgumentException
                || e instanceof LibhegelException;
    }

    /** Rethrow {@code t} as-is: an unchecked exception is returned for the caller to throw. */
    @SuppressWarnings("unchecked")
    private static <T extends Throwable> RuntimeException rethrow(Throwable t) throws T {
        if (t instanceof RuntimeException) {
            return (RuntimeException) t;
        }
        throw (T) t;
    }

    /** Switch this instance onto a block {@code indent} columns in, returning the handle it replaces. */
    private DataSource enterBlock(long indent) {
        DataSource block = source.block(indent);
        shared.addBlock(new TestCase(block, shared));
        return swapSource(block);
    }

    /** Make {@code next} this instance's handle, returning the one it replaces. */
    private DataSource swapSource(DataSource next) {
        closePrinter();
        DataSource previous = source;
        source = next;
        return previous;
    }

    // --- engine primitives used by generators in dev.hegel.generators (public for cross-package
    // access; not part of the user-facing API) ---

    /** @hidden */
    public boolean generateBoolean(double p) {
        return source.generateBoolean(p);
    }

    /** @hidden */
    public long generateInteger(long min, long max) {
        return source.generateInteger(min, max);
    }

    /** @hidden */
    public BigInteger generateIntegerBig(BigInteger min, BigInteger max) {
        return source.generateIntegerBig(min, max);
    }

    /** @hidden */
    public double generateFloat(
            int width,
            double min,
            double max,
            boolean allowNan,
            boolean allowInfinity,
            boolean excludeMin,
            boolean excludeMax,
            double smallestNonzeroMagnitude) {
        return source.generateFloat(
                width, min, max, allowNan, allowInfinity, excludeMin, excludeMax, smallestNonzeroMagnitude);
    }

    /** @hidden */
    public byte[] generateBytes(long minSize, long maxSize) {
        return source.generateBytes(minSize, maxSize);
    }

    /** @hidden */
    public String generateString(StringGeneratorHandle generator) {
        return source.generateString(generator);
    }

    /** @hidden */
    public LocalDate generateDate(LocalDate min, LocalDate max) {
        return source.generateDate(min, max);
    }

    /** @hidden */
    public LocalTime generateTime(LocalTime min, LocalTime max) {
        return source.generateTime(min, max);
    }

    /** @hidden */
    public LocalDateTime generateDatetime(LocalDateTime min, LocalDateTime max) {
        return source.generateDatetime(min, max);
    }

    /** @hidden */
    public UUID generateUuid(Integer version) {
        return source.generateUuid(version);
    }

    /** @hidden */
    public byte[] generateIpv4() {
        return source.generateIpv4();
    }

    /** @hidden */
    public byte[] generateIpv6() {
        return source.generateIpv6();
    }

    /** @hidden */
    public StringGeneratorHandle textGenerator(
            long minSize,
            long maxSize,
            String codec,
            long minCodepoint,
            long maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeCharacters,
            String excludeCharacters) {
        return source.textGenerator(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeCharacters,
                excludeCharacters);
    }

    /** @hidden */
    public StringGeneratorHandle regexGenerator(String pattern, boolean fullmatch, StringGeneratorHandle alphabet) {
        return source.regexGenerator(pattern, fullmatch, alphabet);
    }

    /** @hidden */
    public StringGeneratorHandle emailGenerator() {
        return source.emailGenerator();
    }

    /** @hidden */
    public StringGeneratorHandle urlGenerator() {
        return source.urlGenerator();
    }

    /** @hidden */
    public StringGeneratorHandle domainGenerator(long maxLength) {
        return source.domainGenerator(maxLength);
    }

    /** @hidden */
    public boolean ownsStringGenerator(StringGeneratorHandle generator) {
        return source.ownsStringGenerator(generator);
    }

    /** @hidden */
    public GrammarHandle grammar(
            String text,
            String start,
            long maxSize,
            int maxDepth,
            StringGeneratorHandle alphabet,
            List<String> explicitNames,
            Function<String, String> files) {
        return source.grammar(text, start, maxSize, maxDepth, alphabet, explicitNames, files);
    }

    /** @hidden */
    public String generateGrammar(GrammarHandle grammar, IntFunction<String> terminal) {
        return source.generateGrammar(grammar, terminal);
    }

    /** @hidden */
    public boolean ownsGrammar(GrammarHandle grammar) {
        return source.ownsGrammar(grammar);
    }

    /**
     * Draw a structured value inside a labelled span: open the span, run {@code body}, and close
     * the span whether or not the body completes. This is how composite generators tell the engine
     * which draws belong together, so it can shrink the structure as a unit; {@link Label} lists
     * the engine's structural labels and {@link Label#of(String)} mints custom ones.
     *
     * @param label the span's label
     * @param body the draws making up the value
     * @param <T> the value type
     * @return the body's result
     */
    public <T> T span(long label, Supplier<T> body) {
        startSpan(label);
        try {
            return body.get();
        } finally {
            stopSpan(false);
        }
    }

    /**
     * Open a labelled span. Every {@code startSpan} must be matched by a {@link #stopSpan(boolean)}
     * on every exit path, including exceptional ones; prefer {@link #span(long, Supplier)}, which
     * handles that.
     *
     * @param label the span's label (see {@link Label})
     */
    public void startSpan(long label) {
        source.startSpan(label);
    }

    /**
     * Close the innermost open span. Passing {@code discard = true} tells the engine the span's
     * draws were rejected (for example, a filtered value that failed its predicate) and will be
     * retried. A no-op once the case has been concluded by the engine, so span-closing {@code
     * finally} blocks are safe while a case unwinds.
     *
     * @param discard whether the span's draws were rejected
     */
    public void stopSpan(boolean discard) {
        source.stopSpan(discard);
    }

    /** @hidden */
    public long newCollection(long minSize, long maxSize) {
        return source.newCollection(minSize, maxSize);
    }

    /** @hidden */
    public boolean collectionMore(long id) {
        return source.collectionMore(id);
    }

    /** @hidden */
    public void collectionReject(long id, String why) {
        source.collectionReject(id, why);
    }

    /** @hidden */
    public void collectionFree(long id) {
        source.collectionFree(id);
    }

    /** @hidden */
    public long newRecursion(long maxDepth, long maxLeaves) {
        return source.newRecursion(maxDepth, maxLeaves);
    }

    /** @hidden */
    public boolean recursionBranch(long recursion, long depth) {
        return source.recursionBranch(recursion, depth);
    }

    /** @hidden */
    public boolean recursionLeaf(long recursion) {
        return source.recursionLeaf(recursion);
    }

    /** @hidden */
    public void recursionRetry(long recursion) {
        source.recursionRetry(recursion);
    }

    /** @hidden */
    public boolean recursionFinish(long recursion) {
        return source.recursionFinish(recursion);
    }

    /** @hidden */
    public void recursionFree(long recursion) {
        source.recursionFree(recursion);
    }

    // --- stateful-testing primitives, used by Stateful and Pool in this package ---

    long newPool() {
        return source.newPool();
    }

    long poolAdd(long poolId) {
        return source.poolAdd(poolId);
    }

    long poolGenerate(long poolId, boolean consume) {
        return source.poolGenerate(poolId, consume);
    }

    void poolRestore(long poolId, long variableId) {
        source.poolRestore(poolId, variableId);
    }

    DataSource.StateMachine newStateMachine(
            List<String> ruleNames,
            long[] ruleGroups,
            double[] ruleWeights,
            List<String> invariantNames,
            boolean[] invariantAlwaysCheck,
            long minConcurrency,
            long maxConcurrency,
            int stepCount) {
        return source.newStateMachine(
                ruleNames,
                ruleGroups,
                ruleWeights,
                invariantNames,
                invariantAlwaysCheck,
                minConcurrency,
                maxConcurrency,
                stepCount);
    }

    long stateMachineNextGroup(long stateMachineId) {
        return source.stateMachineNextGroup(stateMachineId);
    }

    long stateMachineNextRule(long stateMachineId, long workerIndex) {
        return source.stateMachineNextRule(stateMachineId, workerIndex);
    }

    void stateMachineRuleRejected(long stateMachineId, long workerIndex) {
        source.stateMachineRuleRejected(stateMachineId, workerIndex);
    }

    boolean stateMachineShouldCheckInvariant(long stateMachineId, long invariantIndex) {
        return source.stateMachineShouldCheckInvariant(stateMachineId, invariantIndex);
    }

    void stateMachineFree(long stateMachineId) {
        source.stateMachineFree(stateMachineId);
    }

    /** Release the calling thread's engine error context, before the thread exits. */
    void releaseThreadContext() {
        source.releaseThreadContext();
    }

    /** Whether the engine has concluded this case (overrun or invalid), so its body must unwind. */
    boolean isAborted() {
        return source.isAborted();
    }

    /** The top-level draws recorded on a captured execution, in draw order. */
    Map<String, Object> draws() {
        return shared.draws();
    }
}
