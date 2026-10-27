package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import dev.hegel.lowlevel.LibhegelException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Consumer;
import java.util.function.LongFunction;

/**
 * Drives a single property test: builds the settings handle, pumps the engine's run loop, and turns
 * the aggregated result into a {@link RunReport}.
 *
 * <p>The engine owns the whole run — generation, shrinking, the replays that confirm a failure, and
 * the final replay of every failure it is about to report — so the loop only pumps cases and keeps
 * the report material as they run. The engine stamps the executions a failure report can be built
 * from ({@code hegel_test_case_should_capture}); a stamped case prints its draws and notes into the
 * engine's document for the case and keeps the rendered lines, and every interesting case's
 * exception is kept per origin, a stamped capture winning over an unstamped one and the newest
 * winning at equal rank. Once the loop drains, each reported failure is built from its origin's
 * capture: the rendered lines go to the reporter, the exception is the body's own, and the
 * engine's caveat and reproduce blob are attached. A {@link
 * Settings#reproduceFailure} run is the same loop over {@code hegel_run_start_blob}. Run-level errors
 * (a failed health check, a nondeterminism abort under {@link NondeterminismStrictness#ERROR}, an
 * engine panic) surface with the engine's own message.
 *
 * <p>Everything the run has to say goes through its {@link Reporter}; the runner itself never
 * prints. Binding and engine errors ({@link HegelException}) are thrown, not reported: they are bugs
 * in the plumbing, not verdicts on the property.
 */
final class Runner {
    private Runner() {}

    /**
     * Package prefixes treated as Hegel/JDK/test-framework infrastructure: {@link #originOf} skips
     * frames in these to find the user frame that owns a failure (used as the shrink-dedup origin).
     * {@link Settings#infrastructurePackages(String...)} adds a frontend's own.
     */
    private static final String[] INFRA_PREFIXES = {
        "dev.hegel.", "org.junit.", "org.opentest4j.", "jdk.", "java.", "sun.", "com.sun."
    };

    /** Message for {@link Settings#reproduceFailure} blobs none of whose replays failed. */
    static final String STALE_BLOB = "reproduceFailure: no supplied failure blob reproduced a failure."
            + " The failure may have been fixed — or, for a nondeterministic blob, it may not have"
            + " recurred within the replay budget. Re-run to retry, or remove the blob once the failure"
            + " is fixed.";

    /**
     * The body's outcome against one case: whether the engine stamped it for capture, its rendered
     * output and draws (when it was stamped), the exception that made it interesting ({@code null}
     * otherwise), and the origin the case was marked complete with.
     */
    private static final class CaseRun {
        final boolean captured;
        final List<String> output;
        final Map<String, Object> draws;
        final Throwable interesting;
        final String origin;

        CaseRun(
                boolean captured,
                List<String> output,
                Map<String, Object> draws,
                Throwable interesting,
                String origin) {
            this.captured = captured;
            this.output = output;
            this.draws = draws;
            this.interesting = interesting;
            this.origin = origin;
        }

        /** A stamped capture outranks an unstamped one, whose only material is the exception. */
        int rank() {
            return captured ? 1 : 0;
        }
    }

    static RunReport run(Libhegel lib, Settings settings, Consumer<TestCase> body, Reporter reporter) {
        RunReport report = withSettings(lib, settings, s -> {
            // The engine parses the string settings as they are applied, so the run and its
            // reporters read the effective configuration back.
            Settings effective = settings.effective(
                    lib.settingsGetTestCases(s),
                    lib.settingsGetPrintBlob(s),
                    NondeterminismStrictness.fromCode(lib.settingsGetNondeterminismStrictness(s)),
                    Verbosity.fromCode(lib.settingsGetVerbosity(s)));
            reporter.runStarted(effective);
            RunStatistics.Counter counts = new RunStatistics.Counter();
            if (!effective.reproduceFailure.isEmpty()) {
                return replay(lib, s, effective, body, reporter, counts);
            }
            List<Failure> explicitFailures = List.of();
            if (!effective.examples.isEmpty() && (lib.settingsGetPhases(s) & Abi.PHASE_EXPLICIT) != 0) {
                boolean reportMultiple = lib.settingsGetReportMultipleFailures(s);
                explicitFailures = runExamples(lib, effective, body, reporter, reportMultiple);
                if (!explicitFailures.isEmpty() && !reportMultiple) {
                    return reportFailures(reporter, counts, explicitFailures, null);
                }
            }
            Verdict verdict = drive(
                    lib, lib.runStart(s, reporter::engineOutput), effective, body, reporter, counts, explicitFailures);
            List<Failure> failures = new ArrayList<>(explicitFailures);
            failures.addAll(verdict.failures());
            if (failures.isEmpty()) {
                return verdict.error() != null
                        ? verdict.error()
                        : new RunReport(RunStatus.PASSED, counts.snapshot(), List.of());
            }
            return reportFailures(reporter, counts, failures, verdict.error());
        });
        reporter.runFinished(report);
        return report;
    }

    /**
     * Replay every one of {@code settings}' reproduce blobs through {@code hegel_run_start_blob},
     * reporting every distinct failure they reproduce. A replay that errors ends the replays, its
     * error reported as a run's is. Throws {@link DidNotReproduceException} when no blob fails.
     */
    private static RunReport replay(
            Libhegel lib,
            long s,
            Settings settings,
            Consumer<TestCase> body,
            Reporter reporter,
            RunStatistics.Counter counts) {
        List<Failure> failures = new ArrayList<>();
        RunReport error = null;
        for (String blob : settings.reproduceFailure) {
            Verdict verdict = drive(
                    lib, lib.runStartBlob(s, blob, reporter::engineOutput), settings, body, reporter, counts, failures);
            failures.addAll(verdict.failures());
            error = verdict.error();
            if (error != null) {
                break;
            }
        }
        if (failures.isEmpty()) {
            if (error != null) {
                return error;
            }
            throw new DidNotReproduceException(STALE_BLOB);
        }
        return reportFailures(reporter, counts, failures, error);
    }

    /**
     * What one engine run concluded: the failures it found that are not among those already
     * reported, and, when it ended in an error instead of a verdict, that error as a report.
     */
    private record Verdict(List<Failure> failures, RunReport error) {}

    /**
     * Run the explicit examples in order, returning their failures: the first only, unless {@code
     * reportMultiple}, in which case one per origin.
     */
    private static List<Failure> runExamples(
            Libhegel lib, Settings settings, Consumer<TestCase> body, Reporter reporter, boolean reportMultiple) {
        List<Failure> failures = new ArrayList<>();
        Set<String> origins = new HashSet<>();
        for (Example example : settings.examples) {
            Failure failure = runExample(lib, example, settings, body, reporter);
            if (failure != null && origins.add(failure.origin())) {
                failures.add(failure);
                if (!reportMultiple) {
                    break;
                }
            }
        }
        return failures;
    }

    /**
     * Run the body once with {@code example}'s values, returning its failure, or {@code null} if it
     * passed. A verbose run reports the case as it does a generated one.
     */
    private static Failure runExample(
            Libhegel lib, Example example, Settings settings, Consumer<TestCase> body, Reporter reporter) {
        boolean verbose = settings.verbosity.code >= Verbosity.VERBOSE.code;
        try (ExplicitDraws draws = new ExplicitDraws(lib, example)) {
            TestCase testCase = TestCase.explicit(draws);
            if (verbose) {
                reporter.caseStarted(false);
                reporter.caseOutput("Running explicit " + example, false);
            }
            Throwable failure = null;
            try {
                body.accept(testCase);
            } catch (InvalidArgument e) {
                throw e;
            } catch (AssumeRejected e) {
                throw new InvalidExample("Explicit test cases must not be rejected, but explicit " + example
                        + " was rejected (assume or reject)");
            } catch (LoopDone e) {
            } catch (Throwable e) {
                failure = e;
            }
            List<String> output = draws.output();
            if (verbose) {
                for (String line : output) {
                    reporter.caseOutput(line, false);
                }
                reporter.caseFinished(failure == null ? CaseOutcome.VALID : CaseOutcome.INTERESTING, false);
            }
            if (failure == null) {
                draws.checkConsumed();
                return null;
            }
            return new Failure(
                    originOf(failure, settings.infrastructurePackages), null, null, failure, testCase.draws(), output);
        }
    }

    /**
     * Run {@code body} on an engine settings handle holding every field of {@code settings}. The
     * handle is freed when {@code body} returns.
     */
    private static <T> T withSettings(Libhegel lib, Settings settings, LongFunction<T> body) {
        long s = newSettings(lib, null);
        try {
            applySettings(lib, s, settings);
            return body.apply(s);
        } finally {
            lib.settingsFree(s);
        }
    }

    /**
     * Construct a settings handle from {@code profile} ({@code null} for the default one). The
     * engine resolves the profile and the {@code HEGEL_*} environment variables here, so this can
     * fail on user input: an unknown profile or a malformed {@code hegel.toml} or variable surfaces
     * as an {@link IllegalArgumentException} carrying the engine's diagnostic.
     */
    static long newSettings(Libhegel lib, String profile) {
        long[] out = new long[1];
        if (profile == null) {
            checkUserCall(lib, lib.settingsNew(out), "hegel_settings_new");
        } else {
            checkUserCall(lib, lib.settingsNewForProfile(profile, out), "hegel_settings_new_for_profile");
        }
        return out[0];
    }

    /** Fail with the engine's diagnostic unless the engine accepts {@code value} for {@code key}. */
    static void checkStringSetting(Libhegel lib, String key, String value) {
        long s = newSettings(lib, null);
        try {
            setFromString(lib, s, key, value);
        } finally {
            lib.settingsFree(s);
        }
    }

    private static void setFromString(Libhegel lib, long s, String key, String value) {
        checkUserCall(lib, lib.settingsSetFromString(s, key, value), "hegel_settings_set_from_string");
    }

    /** Register {@code settings} as the profile {@code name}. */
    static void registerProfile(Libhegel lib, String name, Settings settings) {
        withSettings(lib, settings, s -> {
            checkUserCall(lib, lib.settingsRegisterProfile(name, s), "hegel_settings_register_profile");
            return null;
        });
    }

    /** Make {@code name} the process's default profile ({@code ""} clears it). */
    static void setDefaultProfile(Libhegel lib, String name) {
        checkUserCall(lib, lib.setDefaultProfile(name), "hegel_set_default_profile");
    }

    /**
     * Translate the code of a call that takes user input: {@code E_INVALID_ARG} is the caller's
     * mistake, an {@link IllegalArgumentException} with the engine's diagnostic; anything else
     * non-OK is a {@link HegelException}.
     */
    private static void checkUserCall(Libhegel lib, int rc, String op) {
        if (rc == Abi.E_INVALID_ARG) {
            throw new IllegalArgumentException(nullToEmpty(lib.lastErrorMessage()));
        }
        if (rc != Abi.OK) {
            throw new HegelException(op + " failed (rc=" + rc + "): " + nullToEmpty(lib.lastErrorMessage()));
        }
    }

    /**
     * Pump {@code run} to completion, keeping every interesting case's capture per origin, translate
     * its verdict and free it. Failures with the origin of one of {@code reported} are dropped.
     */
    private static Verdict drive(
            Libhegel lib,
            long run,
            Settings settings,
            Consumer<TestCase> body,
            Reporter reporter,
            RunStatistics.Counter counts,
            List<Failure> reported) {
        try {
            Map<String, CaseRun> captures = new HashMap<>();
            while (true) {
                long tc = lib.nextTestCase(run);
                if (isNull(tc)) {
                    break;
                }
                CaseRun caseRun = driveOneCase(lib, tc, body, settings, reporter, counts);
                if (caseRun.interesting != null) {
                    CaseRun stored = captures.get(caseRun.origin);
                    if (stored == null || caseRun.rank() >= stored.rank()) {
                        captures.put(caseRun.origin, caseRun);
                    }
                }
            }
            long result = lib.runResult(run);
            try {
                return verdict(lib, result, counts, captures, reported);
            } finally {
                lib.runResultFree(result);
            }
        } finally {
            lib.runFree(run);
        }
    }

    /** Translate a drained run's result. */
    private static Verdict verdict(
            Libhegel lib,
            long result,
            RunStatistics.Counter counts,
            Map<String, CaseRun> captures,
            List<Failure> reported) {
        switch (lib.runResultStatus(result)) {
            case Abi.RUN_STATUS_PASSED:
                return new Verdict(List.of(), null);
            case Abi.RUN_STATUS_ERROR:
                int[] healthCheck = new int[1];
                RunErrorKind kind = RunErrorKind.fromCode(lib.runResultErrorKind(result, healthCheck));
                return new Verdict(
                        List.of(),
                        new RunReport(
                                RunStatus.ERROR,
                                counts.snapshot(),
                                nullToEmpty(lib.runResultError(result)),
                                kind,
                                HealthCheck.fromBit(healthCheck[0]),
                                List.of()));
            default:
                return new Verdict(engineFailures(lib, result, captures, reported), null);
        }
    }

    /**
     * Build every distinct failure the engine reports from its origin's capture, skipping those
     * with the origin of a failure already reported.
     */
    private static List<Failure> engineFailures(
            Libhegel lib, long result, Map<String, CaseRun> captures, List<Failure> alreadyReported) {
        Set<String> reported = new HashSet<>();
        for (Failure failure : alreadyReported) {
            reported.add(failure.origin());
        }
        long count = lib.runResultFailureCount(result);
        List<Failure> failures = new ArrayList<>();
        for (int i = 0; i < count; i++) {
            String origin = lib.failureOrigin(result, i);
            CaseRun capture = captures.remove(origin);
            if (capture == null) {
                throw new HegelException(
                        "internal error: failure " + i + " (" + origin + ") has no captured failing execution");
            }
            if (reported.contains(origin)) {
                continue;
            }
            failures.add(new Failure(
                    origin,
                    lib.failureBlob(result, i),
                    lib.failureCaveat(result, i),
                    capture.interesting,
                    capture.draws,
                    capture.output));
        }
        return failures;
    }

    /**
     * Hand every failure to the reporter and report the run FAILED, carrying {@code error}'s
     * message and classification when an engine run errored after other failures were found.
     */
    private static RunReport reportFailures(
            Reporter reporter, RunStatistics.Counter counts, List<Failure> failures, RunReport error) {
        reporter.failuresFound(failures.size());
        for (Failure failure : failures) {
            reporter.caseStarted(true);
            for (String line : failure.output()) {
                reporter.caseOutput(line, true);
            }
            reporter.caseFinished(CaseOutcome.INTERESTING, true);
            reporter.failure(failure);
        }
        if (error == null) {
            return new RunReport(RunStatus.FAILED, counts.snapshot(), failures);
        }
        return new RunReport(
                RunStatus.FAILED,
                counts.snapshot(),
                error.error().orElse(null),
                error.errorKind().orElse(null),
                error.failedHealthCheck().orElse(null),
                failures);
    }

    /**
     * Run the body once against {@code tc}, report the outcome to the engine and the reporter, and
     * free the handle. The returned {@link CaseRun} carries the exception that made the case
     * interesting ({@code null} for any other outcome) and, if the engine stamped the case for
     * capture, its rendered output and draws.
     */
    private static CaseRun driveOneCase(
            Libhegel lib,
            long tc,
            Consumer<TestCase> body,
            Settings settings,
            Reporter reporter,
            RunStatistics.Counter counts) {
        LiveDataSource source = new LiveDataSource(lib, tc);
        try {
            TestCase testCase = new TestCase(
                    source, lib.testCaseShouldCapture(tc), settings.verbosity.code >= Verbosity.VERBOSE.code);
            try {
                return runBody(lib, tc, testCase, body, settings, reporter, counts);
            } finally {
                testCase.releaseAll();
            }
        } finally {
            try {
                source.releasePools();
            } finally {
                lib.testCaseFree(tc);
            }
        }
    }

    /** Run the body against {@code testCase}, collect its output and report the outcome. */
    private static CaseRun runBody(
            Libhegel lib,
            long tc,
            TestCase testCase,
            Consumer<TestCase> body,
            Settings settings,
            Reporter reporter,
            RunStatistics.Counter counts) {
        reporter.caseStarted(false);
        CaseOutcome outcome;
        String origin = null;
        Throwable interesting = null;
        try {
            body.accept(testCase);
            outcome = CaseOutcome.VALID;
        } catch (LoopDone e) {
            outcome = CaseOutcome.VALID;
        } catch (AssumeRejected e) {
            outcome = CaseOutcome.INVALID;
        } catch (StopTest e) {
            outcome = CaseOutcome.OVERRUN;
        } catch (LibhegelException | InvalidArgument e) {
            throw e;
        } catch (Throwable e) {
            outcome = CaseOutcome.INTERESTING;
            origin = originOf(e, settings.infrastructurePackages);
            interesting = e;
        }
        List<String> output = output(testCase, interesting);
        if (settings.verbosity.code >= Verbosity.VERBOSE.code) {
            for (String line : output) {
                reporter.caseOutput(line, false);
            }
        }
        int rc = lib.markComplete(tc, outcome.status, origin);
        if (rc != Abi.OK) {
            throw new HegelException(
                    "hegel_mark_complete failed (rc=" + rc + "): " + nullToEmpty(lib.lastErrorMessage()));
        }
        counts.record(outcome);
        reporter.caseFinished(outcome, false);
        return testCase.isFinal()
                ? new CaseRun(true, output, testCase.draws(), interesting, origin)
                : new CaseRun(false, List.of(), Map.of(), interesting, origin);
    }

    /**
     * The case's rendered output. An error reading it never replaces the body's own failure: it is
     * attached to {@code interesting} as suppressed, and the case reported without output.
     */
    private static List<String> output(TestCase testCase, Throwable interesting) {
        try {
            return testCase.output();
        } catch (RuntimeException e) {
            if (interesting == null) {
                throw e;
            }
            interesting.addSuppressed(e);
            return List.of();
        }
    }

    /**
     * Send every field of {@code st} to the handle. A {@code Settings} holds a value for each
     * engine setting, resolved from the engine's profile and the {@code HEGEL_*} variables when it
     * was created, so the handle's own resolution is replaced wholesale.
     */
    static void applySettings(Libhegel lib, long s, Settings st) {
        lib.settingsTestCases(s, st.testCases);
        lib.settingsVerbosity(s, st.verbosity.code);
        lib.settingsSeed(s, st.seed, st.hasSeed);
        lib.settingsDerandomize(s, st.derandomize);
        lib.settingsReportMultipleFailures(s, st.reportMultipleFailures);
        lib.settingsStopOnFirstFailure(s, st.stopOnFirstFailure);
        lib.settingsShowStatistics(s, st.showStatistics);
        lib.settingsUnboundedChoices(s, st.unboundedChoices);
        lib.settingsPrintBlob(s, st.printBlob);
        lib.settingsNondeterminismStrictness(s, st.nondeterminismStrictness.code);
        lib.settingsBackend(s, st.backend.code);
        lib.settingsSuppressHealthCheck(s, st.suppressMask);
        lib.settingsPhases(s, st.phasesMask);
        switch (st.database.kind) {
            case DISABLED:
                lib.settingsDatabase(s, "");
                break;
            case PATH:
                lib.settingsDatabase(s, st.database.path);
                break;
            default:
                lib.settingsDatabase(s, null);
                break;
        }
        // The key is sent whenever there is one, even with the database off: the engine also derives
        // the derandomized seed from it, so gating this on dbEnabled would make every named test in
        // CI (where the database is disabled) derandomize off the same fallback key.
        if (st.name != null) {
            lib.settingsDatabaseKey(s, st.name);
        }
        if (st.testLocation != null) {
            TestLocation l = st.testLocation;
            lib.settingsTestLocation(s, l.file(), l.beginLine(), l.className(), l.function());
        }
        for (Map.Entry<String, String> setting : st.stringSettings) {
            setFromString(lib, s, setting.getKey(), setting.getValue());
        }
    }

    /**
     * Frames {@link #callerLocation} looks past to find the code that called into Hegel: Hegel's
     * entry points and the JDK (the stack walk itself, reflection).
     */
    private static final String[] ENTRY_PREFIXES = {"dev.hegel.Hegel", "dev.hegel.Runner", "java.", "jdk."};

    /**
     * The location of the code that called into Hegel: the innermost frame on the calling thread
     * outside Hegel's entry points, the JDK and {@code infrastructurePackages}, or {@code null} when
     * there is none.
     */
    static TestLocation callerLocation(List<String> infrastructurePackages) {
        return StackWalker.getInstance()
                .walk(frames -> frames.filter(f -> isCallerFrame(f.getClassName(), infrastructurePackages))
                        .findFirst()
                        .map(f -> TestLocation.of(
                                f.getClassName(), f.getFileName(), f.getLineNumber(), f.getMethodName()))
                        .orElse(null));
    }

    private static boolean isCallerFrame(String className, List<String> infrastructurePackages) {
        for (String prefix : ENTRY_PREFIXES) {
            if (className.startsWith(prefix)) {
                return false;
            }
        }
        for (String prefix : infrastructurePackages) {
            if (className.startsWith(prefix)) {
                return false;
            }
        }
        return true;
    }

    /**
     * The shrink-dedup origin of a failure: the exception's simple type name and the first stack
     * frame outside Hegel, the JDK, the test framework, and {@code infrastructurePackages}.
     */
    static String originOf(Throwable e, List<String> infrastructurePackages) {
        for (StackTraceElement f : e.getStackTrace()) {
            if (isUserFrame(f.getClassName(), infrastructurePackages)) {
                return e.getClass().getSimpleName() + " at " + f.getFileName() + ":" + f.getLineNumber();
            }
        }
        return e.getClass().getName();
    }

    private static boolean isUserFrame(String className, List<String> infrastructurePackages) {
        for (String prefix : INFRA_PREFIXES) {
            if (className.startsWith(prefix)) {
                return false;
            }
        }
        for (String prefix : infrastructurePackages) {
            if (className.startsWith(prefix)) {
                return false;
            }
        }
        return true;
    }

    private static String nullToEmpty(String s) {
        return s == null ? "" : s;
    }

    static boolean isNull(long handle) {
        return handle == 0;
    }
}
