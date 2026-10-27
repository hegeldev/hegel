package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/**
 * A configurable in-memory {@link Libhegel} for exercising error paths and runner logic without the
 * native engine. Every per-case primitive's return code is a public field defaulting to {@link
 * Abi#OK}; set one to a negative code to drive a specific translation path. Draws echo their lower
 * bound (or a canned value) so generator plumbing can run end to end.
 */
final class FakeLibhegel implements Libhegel {
    // Opaque handle sentinels (non-null addresses).
    static final long SETTINGS = 0x100;
    static final long RUN = 0x200;
    static final long TC = 0x300;
    static final long RESULT = 0x400;
    static final long STRING_GEN = 0x600;
    /** Clone handles are {@code CLONE_BASE + n} for the n-th clone made, so they are distinct from {@link #TC}. */
    static final long CLONE_BASE = 0x1000;
    /** Block handles are {@code BLOCK_BASE + n} for the n-th block opened. */
    static final long BLOCK_BASE = 0x8000;
    /** Printer handles are {@code PRINTER_BASE + n} for the n-th printer fetched. */
    static final long PRINTER_BASE = 0x20000;
    /** A deferred printer's region is keyed {@code DEFERRED_BASE + n} for the n-th one opened. */
    static final long DEFERRED_BASE = 0x40000;

    // Output. Like the engine, the fake keeps one document per test case made of regions: the
    // root handle's region is the body, and a clone or block opens a child region at its source's
    // current position, so concurrent workers never interleave. Text, group delimiters and
    // separators are appended as they come (groups never break); a line starts with its region's
    // block indentation and, for a worker's region, "[worker N] "; notes made inside a speculative
    // region are held back until the outermost one closes.
    private Region root = new Region(0, null);

    private final Map<Long, Region> regions = new HashMap<>();
    /** The handle each block was opened on. */
    final Map<Long, Long> blockParents = new HashMap<>();
    /** The test-case handle each printer was fetched for. */
    final Map<Long, Long> printerOwners = new LinkedHashMap<>();
    /** Printer handles released through printerFree, in order. */
    final List<Long> freedPrinters = new ArrayList<>();
    /** Block handles released through testCaseFree, in order. */
    final List<Long> freedBlocks = new ArrayList<>();
    /** Every note, as received. */
    final List<String> notes = java.util.Collections.synchronizedList(new ArrayList<>());

    /** One handle's print region: its own text and the regions anchored in it, in order. */
    private static final class Region {
        final long indent;
        Long worker;
        private final List<Object> parts = new ArrayList<>();
        private boolean atLineStart = true;
        private final Deque<int[]> speculations = new ArrayDeque<>();
        private final List<String> held = new ArrayList<>();

        Region(long indent, Long worker) {
            this.indent = indent;
            this.worker = worker;
        }

        synchronized Region child(long extraIndent) {
            Region child = new Region(indent + extraIndent, worker);
            parts.add(child);
            return child;
        }

        synchronized void text(String text) {
            if (atLineStart) {
                parts.add(" ".repeat((int) indent) + (worker == null ? "" : "[worker " + worker + "] "));
                atLineStart = false;
            }
            parts.add(text);
        }

        synchronized void newline() {
            parts.add("\n");
            atLineStart = true;
        }

        synchronized void note(String text) {
            if (!speculations.isEmpty()) {
                held.add(text);
                return;
            }
            for (String line : text.split("\n", -1)) {
                text(line);
                newline();
            }
        }

        synchronized void beginSpeculative() {
            speculations.push(new int[] {parts.size(), atLineStart ? 1 : 0});
        }

        synchronized void endSpeculative(boolean commit) {
            int[] mark = speculations.pop();
            if (!commit) {
                parts.subList(mark[0], parts.size()).clear();
                atLineStart = mark[1] == 1;
            }
            if (speculations.isEmpty()) {
                List<String> notes = new ArrayList<>(held);
                held.clear();
                for (String note : notes) {
                    note(note);
                }
            }
        }

        synchronized void render(StringBuilder out) {
            for (Object part : parts) {
                if (part instanceof Region region) {
                    region.render(out);
                } else {
                    out.append(part);
                }
            }
        }
    }

    private Region region(long tc) {
        return tc == TC ? root : regions.get(tc);
    }

    int testCaseBlockRc = Abi.OK;
    int testCasePrinterRc = Abi.OK;
    int printerValueRc = Abi.OK;
    int printerTextRc = Abi.OK;
    int printerAbortRc = Abi.OK;
    int noteRc = Abi.OK;

    // Test-case clones (concurrent stateful workers).
    int cloneRc = Abi.OK;
    boolean setWorkerFails;
    /** The source handle of each clone made, in order. */
    final List<Long> clonesMade = new ArrayList<>();
    /** Clone handles released through testCaseFree, in order. */
    final List<Long> freedClones = new ArrayList<>();
    /** The worker index each handle was attributed to. */
    final Map<Long, Long> workersSet = new LinkedHashMap<>();

    String lastError = "fake error";
    String version = "0.0.0-fake";

    // Run-loop control.
    int caseCount = 1; // how many test cases nextTestCase yields
    private int casesServed;
    boolean runStartFails;
    boolean nextTestCaseFails;
    int runStatus = Abi.RUN_STATUS_PASSED;
    String runError = "run error";
    int runErrorKind = Abi.RUN_ERROR_NONE;
    int runErrorHealthCheck;
    final List<String> failureBlobs = new ArrayList<>(); // one entry per distinct failure
    final List<String> failureCaveats = new ArrayList<>(); // parallel to failureBlobs; missing = null
    // The origin of each distinct failure. Like the engine, the fake echoes back the origins marked
    // interesting, distinct and in first-seen order; an explicit entry here overrides that.
    final List<String> failureOrigins = new ArrayList<>();
    int fromBlobRc = Abi.OK;
    final List<String> replayedBlobs = new ArrayList<>();
    String startedBlob; // what runStartBlob received; null = the run explored
    final List<String> startedBlobs = new ArrayList<>();
    final java.util.ArrayDeque<Integer> runStatusQueue = new java.util.ArrayDeque<>();
    Consumer<String> output; // the callback runStart / runStartBlob registered
    // The engine's capture stamp per served case: the i-th case is stamped when captureSequence
    // has an i-th entry and it is true; past the end of the sequence every case is stamped, so
    // the default stamps everything.
    boolean[] captureSequence = {};

    // Recorded outcomes and teardown.
    final List<Integer> markedStatuses = new ArrayList<>();
    final List<String> markedOrigins = new ArrayList<>();
    int markCompleteRc = Abi.OK;
    int freedTestCases;
    int freedStringGenerators;
    boolean runFreed;
    boolean runResultFreed;
    boolean settingsFreed;

    // Captured settings.
    int phasesMask = -1; // -1 means settingsPhases was never called
    int suppressMask = -1;
    Integer backendCode;
    Long testCases;
    String databasePath = "unset";
    String databaseKey;
    Boolean derandomize;
    Integer verbosity;
    Boolean reportMultipleFailures;
    Boolean stopOnFirstFailure;
    Boolean showStatistics;
    Boolean unboundedChoices;
    TestLocation testLocation;
    // Profiles.
    String profileRequested; // the name settingsNewForProfile was last called with
    int settingsNewForProfileRc = Abi.OK;
    int registerProfileRc = Abi.OK;
    final List<String> stringSettings = new ArrayList<>();
    final List<String> registeredProfiles = new ArrayList<>();
    int setDefaultProfileRc = Abi.OK;
    String defaultProfile;

    // Per-primitive return codes and canned values.
    int generateBooleanRc = Abi.OK;
    boolean booleanValue;
    int generateIntegerRc = Abi.OK;
    Long integerValue; // null = echo the min bound
    Long integerMin;
    Long integerMax;
    int generateFloatRc = Abi.OK;
    Double floatValue; // null = 0.0
    int generateBytesRc = Abi.OK;
    byte[] bytesValue = {1, 2};
    Long bytesMinSize;
    Long bytesMaxSize;
    int generateStringRc = Abi.OK;
    String stringValue = "s";
    int generateDateRc = Abi.OK;
    int generateTimeRc = Abi.OK;
    int generateDatetimeRc = Abi.OK;
    int generateUuidRc = Abi.OK;
    byte[] uuidBytes = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};
    int generateIpv4Rc = Abi.OK;
    byte[] ipv4Bytes = {127, 0, 0, 1};
    int generateIpv6Rc = Abi.OK;
    byte[] ipv6Bytes = new byte[16];

    int stringGeneratorTextRc = Abi.OK;
    int stringGeneratorRegexRc = Abi.OK;
    int stringGeneratorEmailRc = Abi.OK;
    int stringGeneratorUrlRc = Abi.OK;
    int stringGeneratorDomainRc = Abi.OK;
    // Captured text-generator configuration, for builder-behaviour assertions.
    Long textMinSize;
    Long textMaxSize;
    Long textMinCodepoint;
    Long textMaxCodepoint;
    List<String> textCategories;
    List<String> textExcludeCategories;
    String textIncludeCharacters;
    String textExcludeCharacters;
    Long domainMaxLength;

    int startSpanRc = Abi.OK;
    int stopSpanRc = Abi.OK;
    int stoppedSpans;
    final List<Long> startedSpans = new ArrayList<>();
    int newCollectionRc = Abi.OK;
    long collectionId = 7;
    Long collectionMinSize;
    Long collectionMaxSize;
    int collectionMoreRc = Abi.OK;
    boolean[] moreSequence = {false};
    private int moreIndex;
    int collectionRejectRc = Abi.OK;
    /** Collection handles released through collectionFree, in order. */
    final List<Long> freedCollections = new ArrayList<>();

    int targetRc = Abi.OK;
    /** Whether generate_boolean was ever called. */
    boolean booleanDrawn;

    int eventRc = Abi.OK;
    /** Every event and event value, as {@code label} or {@code label=value}. */
    final List<String> events = new ArrayList<>();

    int generateIntegerBigRc = Abi.OK;
    byte[] integerBigValue; // null = echo the lower bound

    // Recursion. Each recursion_* call returns the next code of its queue, or OK once it is empty;
    // recursion_branch answers from `branchSequence` (false once it is exhausted).
    int newRecursionRc = Abi.OK;
    long recursionId = 9;
    final Deque<Integer> recursionBranchRcs = new ArrayDeque<>();
    final Deque<Boolean> branchSequence = new ArrayDeque<>();
    final Deque<Integer> recursionLeafRcs = new ArrayDeque<>();
    final Deque<Integer> recursionRetryRcs = new ArrayDeque<>();
    final Deque<Integer> recursionFinishRcs = new ArrayDeque<>();
    /** Recursion handles released through recursionFree, in order. */
    final List<Long> freedRecursions = new ArrayList<>();

    int newPoolRc = Abi.OK;
    long poolId = 3;
    int poolAddRc = Abi.OK;
    private long nextVariableId;
    int poolGenerateRc = Abi.OK;
    Long poolGenerateValue; // null = the first added variable id (0)
    /** Answers to consuming pool_generate calls, in order, before falling back to poolGenerateValue. */
    final Deque<Long> poolGenerateSequence = new ArrayDeque<>();
    /** Codes pool_generate returns before its OK answers, in order. */
    final Deque<Integer> poolGenerateRcs = new ArrayDeque<>();

    int poolRestoreRc = Abi.OK;
    /** Variable ids put back through poolRestore, in order. */
    final List<Long> restoredVariables = new ArrayList<>();
    /** Pool handles released through poolFree, in order. */
    final List<Long> freedPools = new ArrayList<>();
    /** Names of the threads that released their context, in order. */
    final List<String> contextReleases = new ArrayList<>();
    // State machines. The fake plays the engine's round protocol. Sequentially, each round
    // (next_group) hands out exactly one rule from `ruleSequence` (next_rule), then the join
    // point; once the sequence is exhausted next_group reports DONE. Concurrently, set
    // `concurrentRounds`: round r hands worker w the rules `concurrentRounds[r][w]` (a missing or
    // empty worker entry means that worker's round is over at once), and `stateMachineConcurrency`
    // says how many workers new_state_machine reports. The protocol methods are synchronized
    // because concurrent workers call them at the same time.
    int newStateMachineRc = Abi.OK;
    long stateMachineId = 5;
    List<String> stateMachineRules;
    long[] stateMachineRuleGroups;
    double[] stateMachineRuleWeights = {-1}; // what new_state_machine received; null = equal weights
    List<String> stateMachineInvariants;
    boolean[] stateMachineAlwaysCheck;
    long stateMachineMinConcurrency = -1;
    long stateMachineMaxConcurrency = -1;
    long stateMachineStepCount = -1;
    long stateMachineConcurrency = 1; // what new_state_machine writes to out_concurrency
    int stateMachineNextGroupRc = Abi.OK;
    long stateMachineGroupId = 0;
    int stateMachineNextRuleRc = Abi.OK;
    long[] ruleSequence = {}; // the rule index handed out each round
    private int ruleIndex;
    long[][][] concurrentRounds; // null = the sequential protocol over ruleSequence
    private int roundsServed;
    private long[][] currentRound; // null = no round open
    private int[] positions; // per worker, how far into its queue for the current round
    /** The worker index of every next_rule call, in call order. */
    final List<Long> nextRuleWorkers = new ArrayList<>();

    int stateMachineRuleRejectedRc = Abi.OK;
    int rejectedRules;
    /** The worker index of every rule_rejected call, in call order. */
    final List<Long> rejectedWorkers = new ArrayList<>();

    int stateMachineShouldCheckInvariantRc = Abi.OK;
    boolean shouldCheckInvariant = true; // the sampling decision for every invariant
    final List<Long> invariantChecksAsked = new ArrayList<>();
    int freedStateMachines;

    // What hegel_settings_new hands back: the resolved profile's values, as the getters report them.
    int settingsNewRc = Abi.OK;
    long resolvedTestCases = 100;
    boolean resolvedPrintBlob = false;
    int resolvedPhases = Abi.PHASE_ALL;
    boolean resolvedReportMultipleFailures = false;
    int resolvedStrictness = Abi.NONDETERMINISM_QUIET;
    Boolean printBlob; // what the setter received; null = never set
    Integer strictness; // what the setter received; null = never set

    @Override
    public int settingsNew(long[] out) {
        if (settingsNewRc == Abi.OK) {
            out[0] = SETTINGS;
        }
        return settingsNewRc;
    }

    @Override
    public long settingsGetTestCases(long s) {
        return testCases == null ? resolvedTestCases : testCases;
    }

    @Override
    public void settingsPrintBlob(long s, boolean yes) {
        printBlob = yes;
    }

    @Override
    public boolean settingsGetPrintBlob(long s) {
        return printBlob == null ? resolvedPrintBlob : printBlob;
    }

    @Override
    public void settingsNondeterminismStrictness(long s, int strictness) {
        this.strictness = strictness;
    }

    @Override
    public int settingsGetNondeterminismStrictness(long s) {
        return strictness == null ? resolvedStrictness : strictness;
    }

    @Override
    public int settingsGetVerbosity(long s) {
        return verbosity == null ? Abi.VERBOSITY_NORMAL : verbosity;
    }

    @Override
    public int settingsGetPhases(long s) {
        return phasesMask == -1 ? resolvedPhases : phasesMask;
    }

    @Override
    public boolean settingsGetReportMultipleFailures(long s) {
        return reportMultipleFailures == null ? resolvedReportMultipleFailures : reportMultipleFailures;
    }

    @Override
    public int settingsNewForProfile(String name, long[] out) {
        profileRequested = name;
        if (settingsNewForProfileRc == Abi.OK) {
            out[0] = SETTINGS;
        }
        return settingsNewForProfileRc;
    }

    @Override
    public int settingsRegisterProfile(String name, long s) {
        if (registerProfileRc == Abi.OK) {
            registeredProfiles.add(name);
        }
        return registerProfileRc;
    }

    @Override
    public int settingsSetFromString(long s, String key, String value) {
        stringSettings.add(key + "=" + value);
        return Abi.OK;
    }

    @Override
    public int setDefaultProfile(String name) {
        if (setDefaultProfileRc == Abi.OK) {
            defaultProfile = name;
        }
        return setDefaultProfileRc;
    }

    @Override
    public void settingsStopOnFirstFailure(long s, boolean yes) {
        stopOnFirstFailure = yes;
    }

    @Override
    public void settingsShowStatistics(long s, boolean yes) {
        showStatistics = yes;
    }

    @Override
    public void settingsUnboundedChoices(long s, boolean yes) {
        unboundedChoices = yes;
    }

    @Override
    public void settingsTestLocation(long s, String file, int beginLine, String className, String function) {
        testLocation = new TestLocation(file, beginLine, className, function);
    }

    @Override
    public void settingsFree(long s) {
        settingsFreed = true;
    }

    @Override
    public void settingsBackend(long s, int backend) {
        backendCode = backend;
    }

    @Override
    public void settingsTestCases(long s, long n) {
        testCases = n;
    }

    @Override
    public void settingsVerbosity(long s, int v) {
        verbosity = v;
    }

    @Override
    public void settingsSeed(long s, long seed, boolean hasSeed) {}

    @Override
    public void settingsDerandomize(long s, boolean derandomize) {
        this.derandomize = derandomize;
    }

    @Override
    public void settingsReportMultipleFailures(long s, boolean yes) {
        reportMultipleFailures = yes;
    }

    @Override
    public void settingsDatabase(long s, String path) {
        databasePath = path;
    }

    @Override
    public void settingsDatabaseKey(long s, String key) {
        databaseKey = key;
    }

    @Override
    public void settingsPhases(long s, int mask) {
        phasesMask = mask;
    }

    @Override
    public void settingsSuppressHealthCheck(long s, int mask) {
        suppressMask = mask;
    }

    @Override
    public long runStart(long settings, Consumer<String> output) {
        if (runStartFails) {
            throw new HegelException("hegel_run_start failed: " + lastError);
        }
        this.output = output;
        nextRun();
        return RUN;
    }

    private void nextRun() {
        if (!runStatusQueue.isEmpty()) {
            runStatus = runStatusQueue.poll();
            casesServed = 0;
            runMarksFrom = markedOrigins.size();
        }
    }

    private int runMarksFrom;

    @Override
    public long runStartBlob(long settings, String blob, Consumer<String> output) {
        if (runStartFails) {
            throw new HegelException("hegel_run_start_blob failed: " + lastError);
        }
        startedBlob = blob;
        startedBlobs.add(blob);
        this.output = output;
        nextRun();
        return RUN;
    }

    @Override
    public long nextTestCase(long run) {
        if (nextTestCaseFails) {
            throw new HegelException("hegel_next_test_case failed: " + lastError);
        }
        if (casesServed >= caseCount) {
            return 0;
        }
        casesServed++;
        resetDocument();
        return TC;
    }

    @Override
    public boolean testCaseShouldCapture(long tc) {
        int index = casesServed - 1;
        return index >= captureSequence.length || captureSequence[index];
    }

    @Override
    public long runResult(long run) {
        return RESULT;
    }

    @Override
    public void runResultFree(long result) {
        runResultFreed = true;
    }

    @Override
    public void runFree(long run) {
        runFreed = true;
    }

    @Override
    public int testCaseFromBlob(long settings, String blob, Consumer<String> output, long[] out) {
        if (fromBlobRc == Abi.OK) {
            replayedBlobs.add(blob);
            resetDocument();
            out[0] = TC;
        }
        return fromBlobRc;
    }

    @Override
    public synchronized void testCaseFree(long tc) {
        freedTestCases++;
        if (blockParents.containsKey(tc)) {
            freedBlocks.add(tc);
        } else if (tc != TC) {
            freedClones.add(tc);
        }
    }

    @Override
    public synchronized int testCaseBlock(long tc, long indent, long[] out) {
        if (testCaseBlockRc == Abi.OK) {
            long block = BLOCK_BASE + blockParents.size();
            blockParents.put(block, tc);
            regions.put(block, region(tc).child(indent));
            out[0] = block;
        }
        return testCaseBlockRc;
    }

    private void resetDocument() {
        root = new Region(0, null);
    }

    @Override
    public int note(long tc, String text) {
        if (noteRc != Abi.OK) {
            return noteRc;
        }
        notes.add(text);
        region(tc).note(text);
        return Abi.OK;
    }

    @Override
    public synchronized int testCasePrinter(long tc, long[] out) {
        if (testCasePrinterRc == Abi.OK) {
            long printer = PRINTER_BASE + printerOwners.size();
            printerOwners.put(printer, tc);
            out[0] = printer;
        }
        return testCasePrinterRc;
    }

    @Override
    public long printerNew() {
        throw new UnsupportedOperationException("the fake has no standalone documents");
    }

    @Override
    public long printerNew(long options) {
        throw new UnsupportedOperationException("the fake has no standalone documents");
    }

    @Override
    public long printerOptionsNew() {
        throw new UnsupportedOperationException("the fake has no standalone documents");
    }

    @Override
    public int printerOptionsSetMaxWidth(long options, long maxWidth) {
        throw new UnsupportedOperationException("the fake has no standalone documents");
    }

    @Override
    public void printerOptionsFree(long options) {
        throw new UnsupportedOperationException("the fake has no standalone documents");
    }

    @Override
    public int printerIfBreak(long printer, String text) {
        return Abi.OK;
    }

    @Override
    public int printerComment(long printer, String text) {
        return printerText(printer, text);
    }

    @Override
    public int printerShiftIndent(long printer, long delta) {
        return Abi.OK;
    }

    @Override
    public synchronized int printerDeferred(long printer, long[] out) {
        long region = DEFERRED_BASE + regions.size();
        regions.put(region, printerRegion(printer).child(0));
        out[0] = PRINTER_BASE + printerOwners.size();
        printerOwners.put(out[0], region);
        return Abi.OK;
    }

    @Override
    public int printerIsLive(long printer, boolean[] out) {
        out[0] = true;
        return Abi.OK;
    }

    @Override
    public synchronized void printerFree(long printer) {
        freedPrinters.add(printer);
    }

    private synchronized Region printerRegion(long printer) {
        return region(printerOwners.get(printer));
    }

    @Override
    public int printerText(long printer, String text) {
        if (printerTextRc != Abi.OK) {
            return printerTextRc;
        }
        printerRegion(printer).text(text);
        return Abi.OK;
    }

    @Override
    public int printerBreakable(long printer, String sep) {
        return printerText(printer, sep);
    }

    @Override
    public int printerHardBreak(long printer) {
        printerRegion(printer).newline();
        return Abi.OK;
    }

    @Override
    public int printerBeginGroup(long printer, long indent, String open) {
        return printerText(printer, open);
    }

    @Override
    public int printerEndGroup(long printer, String close) {
        return printerText(printer, close);
    }

    @Override
    public int printerReflow(long printer, String text) {
        return printerText(printer, text);
    }

    @Override
    public int printerBeginSpeculative(long printer) {
        printerRegion(printer).beginSpeculative();
        return Abi.OK;
    }

    @Override
    public int printerCommitSpeculative(long printer) {
        printerRegion(printer).endSpeculative(true);
        return Abi.OK;
    }

    @Override
    public int printerAbortSpeculative(long printer) {
        printerRegion(printer).endSpeculative(false);
        return printerAbortRc;
    }

    /** The current case's document as the engine would hand it back. */
    String document() {
        StringBuilder out = new StringBuilder();
        root.render(out);
        return out.toString();
    }

    @Override
    public int printerResolve(long printer) {
        return Abi.OK;
    }

    @Override
    public int printerValue(long printer, String[] out) {
        if (printerValueRc == Abi.OK) {
            out[0] = document();
        }
        return printerValueRc;
    }

    @Override
    public synchronized int testCaseClone(long tc, long[] out) {
        if (cloneRc == Abi.OK) {
            out[0] = CLONE_BASE + clonesMade.size();
            clonesMade.add(tc);
            regions.put(out[0], region(tc).child(0));
        }
        return cloneRc;
    }

    @Override
    public synchronized void testCaseSetWorker(long tc, long workerIndex) {
        if (setWorkerFails) {
            throw new HegelException("hegel_test_case_set_worker failed: " + lastError);
        }
        workersSet.put(tc, workerIndex);
        region(tc).worker = workerIndex;
    }

    @Override
    public int generateBoolean(long tc, double p, boolean[] out) {
        booleanDrawn = true;
        if (generateBooleanRc == Abi.OK) {
            out[0] = booleanValue;
        }
        return generateBooleanRc;
    }

    @Override
    public int generateInteger(long tc, long min, long max, long[] out) {
        if (generateIntegerRc == Abi.OK) {
            integerMin = min;
            integerMax = max;
            out[0] = integerValue == null ? min : integerValue;
        }
        return generateIntegerRc;
    }

    @Override
    public int generateIntegerBig(long tc, byte[] min, byte[] max, byte[][] out) {
        if (generateIntegerBigRc == Abi.OK) {
            out[0] = integerBigValue == null ? min.clone() : integerBigValue;
        }
        return generateIntegerBigRc;
    }

    @Override
    public int generateFloat(
            long tc,
            int width,
            double min,
            double max,
            boolean allowNan,
            boolean allowInfinity,
            boolean excludeMin,
            boolean excludeMax,
            double smallestNonzeroMagnitude,
            double[] out) {
        if (generateFloatRc == Abi.OK) {
            out[0] = floatValue == null ? 0.0 : floatValue;
        }
        return generateFloatRc;
    }

    @Override
    public int generateBytes(long tc, long minSize, long maxSize, byte[][] out) {
        if (generateBytesRc == Abi.OK) {
            bytesMinSize = minSize;
            bytesMaxSize = maxSize;
            out[0] = bytesValue;
        }
        return generateBytesRc;
    }

    @Override
    public int generateString(long tc, long generator, String[] out) {
        if (generateStringRc == Abi.OK) {
            out[0] = stringValue;
        }
        return generateStringRc;
    }

    @Override
    public int generateDate(long tc, LocalDate min, LocalDate max, LocalDate[] out) {
        if (generateDateRc == Abi.OK) {
            out[0] = min;
        }
        return generateDateRc;
    }

    @Override
    public int generateTime(long tc, LocalTime min, LocalTime max, LocalTime[] out) {
        if (generateTimeRc == Abi.OK) {
            out[0] = min;
        }
        return generateTimeRc;
    }

    @Override
    public int generateDatetime(long tc, LocalDateTime min, LocalDateTime max, LocalDateTime[] out) {
        if (generateDatetimeRc == Abi.OK) {
            out[0] = min;
        }
        return generateDatetimeRc;
    }

    @Override
    public int generateUuid(long tc, int version, boolean hasVersion, byte[] out16) {
        if (generateUuidRc == Abi.OK) {
            System.arraycopy(uuidBytes, 0, out16, 0, 16);
        }
        return generateUuidRc;
    }

    @Override
    public int generateIpv4(long tc, byte[] out4) {
        if (generateIpv4Rc == Abi.OK) {
            System.arraycopy(ipv4Bytes, 0, out4, 0, 4);
        }
        return generateIpv4Rc;
    }

    @Override
    public int generateIpv6(long tc, byte[] out16) {
        if (generateIpv6Rc == Abi.OK) {
            System.arraycopy(ipv6Bytes, 0, out16, 0, 16);
        }
        return generateIpv6Rc;
    }

    @Override
    public int stringGeneratorText(
            long minSize,
            long maxSize,
            String codec,
            long minCodepoint,
            long maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeCharacters,
            String excludeCharacters,
            long[] out) {
        if (stringGeneratorTextRc == Abi.OK) {
            textMinSize = minSize;
            textMaxSize = maxSize;
            textMinCodepoint = minCodepoint;
            textMaxCodepoint = maxCodepoint;
            textCategories = categories;
            textExcludeCategories = excludeCategories;
            textIncludeCharacters = includeCharacters;
            textExcludeCharacters = excludeCharacters;
            out[0] = STRING_GEN;
        }
        return stringGeneratorTextRc;
    }

    @Override
    public int stringGeneratorRegex(String pattern, boolean fullmatch, long alphabet, long[] out) {
        if (stringGeneratorRegexRc == Abi.OK) {
            out[0] = STRING_GEN;
        }
        return stringGeneratorRegexRc;
    }

    @Override
    public int stringGeneratorEmail(long[] out) {
        if (stringGeneratorEmailRc == Abi.OK) {
            out[0] = STRING_GEN;
        }
        return stringGeneratorEmailRc;
    }

    @Override
    public int stringGeneratorUrl(long[] out) {
        if (stringGeneratorUrlRc == Abi.OK) {
            out[0] = STRING_GEN;
        }
        return stringGeneratorUrlRc;
    }

    @Override
    public int stringGeneratorDomain(long maxLength, long[] out) {
        if (stringGeneratorDomainRc == Abi.OK) {
            domainMaxLength = maxLength;
            out[0] = STRING_GEN;
        }
        return stringGeneratorDomainRc;
    }

    @Override
    public void stringGeneratorFree(long generator) {
        freedStringGenerators++;
    }

    /** Return code of every grammar-draw advance: by default the string is complete at once. */
    int grammarDrawNextRc = Abi.E_NOT_FOUND;
    /** Grammars compiled through grammarNew. */
    int grammarsBuilt;
    /** Grammar draw handles released through grammarDrawFree. */
    int freedGrammarDraws;

    @Override
    public int grammarBuilderNew(String text, long[] out) {
        out[0] = 0x700;
        return Abi.OK;
    }

    @Override
    public int grammarBuilderFile(long builder, long index, String[] outPath) {
        return Abi.E_NOT_FOUND;
    }

    @Override
    public int grammarBuilderProvide(long builder, long index, String text) {
        return Abi.OK;
    }

    @Override
    public void grammarBuilderFree(long builder) {}

    @Override
    public int grammarNew(
            long builder,
            String start,
            long maxSize,
            int maxDepth,
            long alphabet,
            List<String> explicitNames,
            long[] out) {
        grammarsBuilt++;
        out[0] = 0x710;
        return Abi.OK;
    }

    @Override
    public void grammarFree(long grammar) {}

    @Override
    public long grammarDrawNew(long grammar, long maxSize) {
        return 0x720;
    }

    @Override
    public int grammarDrawNext(long tc, long draw, long[] outTerminal) {
        return grammarDrawNextRc;
    }

    @Override
    public int grammarDrawProvide(long draw, String value) {
        return Abi.OK;
    }

    @Override
    public int grammarDrawResult(long draw, String[] out) {
        out[0] = "";
        return Abi.OK;
    }

    @Override
    public void grammarDrawFree(long draw) {
        freedGrammarDraws++;
    }

    @Override
    public int startSpan(long tc, long label) {
        if (startSpanRc == Abi.OK) {
            startedSpans.add(label);
        }
        return startSpanRc;
    }

    @Override
    public int stopSpan(long tc, boolean discard) {
        stoppedSpans++;
        return stopSpanRc;
    }

    @Override
    public int newCollection(long tc, long minSize, long maxSize, long[] outId) {
        if (newCollectionRc == Abi.OK) {
            collectionMinSize = minSize;
            collectionMaxSize = maxSize;
            outId[0] = collectionId;
        }
        return newCollectionRc;
    }

    @Override
    public int collectionMore(long tc, long id, boolean[] outMore) {
        if (collectionMoreRc == Abi.OK) {
            outMore[0] = moreIndex < moreSequence.length && moreSequence[moreIndex++];
        }
        return collectionMoreRc;
    }

    @Override
    public int collectionReject(long tc, long id, String why) {
        return collectionRejectRc;
    }

    @Override
    public void collectionFree(long id) {
        freedCollections.add(id);
    }

    @Override
    public int newPool(long tc, long[] outId) {
        if (newPoolRc == Abi.OK) {
            outId[0] = poolId;
        }
        return newPoolRc;
    }

    @Override
    public int poolAdd(long tc, long poolId, long[] outVariableId) {
        if (poolAddRc == Abi.OK) {
            outVariableId[0] = nextVariableId++;
        }
        return poolAddRc;
    }

    @Override
    public int poolGenerate(long tc, long poolId, boolean consume, long[] outVariableId) {
        Integer queued = poolGenerateRcs.poll();
        int rc = queued != null ? queued : poolGenerateRc;
        if (rc == Abi.OK) {
            Long next = poolGenerateSequence.poll();
            outVariableId[0] = next != null ? next : poolGenerateValue == null ? 0 : poolGenerateValue;
        }
        return rc;
    }

    @Override
    public int poolRestore(long tc, long poolId, long variableId) {
        if (poolRestoreRc == Abi.OK) {
            restoredVariables.add(variableId);
        }
        return poolRestoreRc;
    }

    @Override
    public int newRecursion(long tc, long maxDepth, long maxLeaves, long[] out) {
        out[0] = newRecursionRc == Abi.OK ? recursionId : 0;
        return newRecursionRc;
    }

    @Override
    public int recursionBranch(long tc, long recursion, long depth, boolean[] outBranch) {
        int rc = next(recursionBranchRcs);
        if (rc == Abi.OK) {
            Boolean branch = branchSequence.poll();
            outBranch[0] = branch != null && branch;
        }
        return rc;
    }

    @Override
    public int recursionLeaf(long tc, long recursion) {
        return next(recursionLeafRcs);
    }

    @Override
    public int recursionRetry(long tc, long recursion) {
        return next(recursionRetryRcs);
    }

    @Override
    public int recursionFinish(long tc, long recursion) {
        return next(recursionFinishRcs);
    }

    private static int next(Deque<Integer> rcs) {
        Integer rc = rcs.poll();
        return rc == null ? Abi.OK : rc;
    }

    @Override
    public void recursionFree(long recursion) {
        freedRecursions.add(recursion);
    }

    @Override
    public void poolFree(long poolId) {
        freedPools.add(poolId);
    }

    @Override
    public int newStateMachine(
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
            long[] outConcurrency) {
        if (newStateMachineRc == Abi.OK) {
            stateMachineRules = ruleNames;
            stateMachineRuleGroups = ruleGroups;
            stateMachineRuleWeights = ruleWeights;
            stateMachineInvariants = invariantNames;
            stateMachineAlwaysCheck = invariantAlwaysCheck;
            stateMachineMinConcurrency = minConcurrency;
            stateMachineMaxConcurrency = maxConcurrency;
            stateMachineStepCount = stepCount;
            outId[0] = stateMachineId;
            outConcurrency[0] = stateMachineConcurrency;
        }
        return newStateMachineRc;
    }

    @Override
    public synchronized int stateMachineNextGroup(long tc, long stateMachineId, long[] outGroupId) {
        if (stateMachineNextGroupRc == Abi.OK) {
            if (concurrentRounds != null) {
                currentRound = roundsServed < concurrentRounds.length ? concurrentRounds[roundsServed++] : null;
            } else {
                // Sequential: one rule for worker 0 per round.
                currentRound = ruleIndex < ruleSequence.length ? new long[][] {{ruleSequence[ruleIndex++]}} : null;
            }
            positions = new int[(int) stateMachineConcurrency];
            outGroupId[0] = currentRound != null ? stateMachineGroupId : Abi.STATE_MACHINE_DONE;
        }
        return stateMachineNextGroupRc;
    }

    @Override
    public synchronized int stateMachineNextRule(long tc, long stateMachineId, long workerIndex, long[] outRuleIndex) {
        if (stateMachineNextRuleRc == Abi.OK) {
            nextRuleWorkers.add(workerIndex);
            int w = (int) workerIndex;
            long[] queue = currentRound != null && w < currentRound.length ? currentRound[w] : new long[0];
            if (positions[w] < queue.length) {
                outRuleIndex[0] = queue[positions[w]++];
            } else {
                outRuleIndex[0] = Abi.STATE_MACHINE_DONE;
            }
        }
        return stateMachineNextRuleRc;
    }

    @Override
    public synchronized int stateMachineRuleRejected(long tc, long stateMachineId, long workerIndex) {
        if (stateMachineRuleRejectedRc == Abi.OK) {
            rejectedRules++;
            rejectedWorkers.add(workerIndex);
        }
        return stateMachineRuleRejectedRc;
    }

    @Override
    public int stateMachineShouldCheckInvariant(
            long tc, long stateMachineId, long invariantIndex, boolean[] outShouldCheck) {
        if (stateMachineShouldCheckInvariantRc == Abi.OK) {
            invariantChecksAsked.add(invariantIndex);
            outShouldCheck[0] = shouldCheckInvariant;
        }
        return stateMachineShouldCheckInvariantRc;
    }

    @Override
    public void stateMachineFree(long stateMachineId) {
        freedStateMachines++;
    }

    @Override
    public int target(long tc, double value, String label) {
        return targetRc;
    }

    @Override
    public int event(long tc, String label) {
        if (eventRc == Abi.OK) {
            events.add(label);
        }
        return eventRc;
    }

    @Override
    public int eventValue(long tc, double value, String label) {
        if (eventRc == Abi.OK) {
            events.add(label + "=" + value);
        }
        return eventRc;
    }

    @Override
    public int markComplete(long tc, int status, String origin) {
        markedStatuses.add(status);
        markedOrigins.add(origin);
        return markCompleteRc;
    }

    @Override
    public int runResultStatus(long result) {
        return runStatus;
    }

    @Override
    public String runResultError(long result) {
        return runError;
    }

    @Override
    public long runResultFailureCount(long result) {
        return failureBlobs.size();
    }

    @Override
    public String failureBlob(long result, long index) {
        return failureBlobs.get((int) index);
    }

    @Override
    public String failureOrigin(long result, long index) {
        if (index < failureOrigins.size()) {
            return failureOrigins.get((int) index);
        }
        List<String> distinct = new ArrayList<>();
        for (String origin : markedOrigins.subList(runMarksFrom, markedOrigins.size())) {
            if (origin != null && !distinct.contains(origin)) {
                distinct.add(origin);
            }
        }
        return distinct.get((int) index);
    }

    @Override
    public String failureCaveat(long result, long index) {
        return index < failureCaveats.size() ? failureCaveats.get((int) index) : null;
    }

    @Override
    public String lastErrorMessage() {
        return lastError;
    }

    @Override
    public int runResultErrorKind(long result, int[] outHealthCheck) {
        outHealthCheck[0] = runErrorHealthCheck;
        return runErrorKind;
    }

    @Override
    public long labelFromName(String name) {
        throw new UnsupportedOperationException("labels are computed by the real engine");
    }

    @Override
    public long labelCombine(long[] labels) {
        throw new UnsupportedOperationException("labels are computed by the real engine");
    }

    @Override
    public synchronized void releaseThreadContext() {
        contextReleases.add(Thread.currentThread().getName());
    }

    @Override
    public String version() {
        return version;
    }
}
