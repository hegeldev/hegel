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

    /** Message for a {@link Settings#reproduceFailure} blob none of whose replays failed. */
    static final String STALE_BLOB = "reproduceFailure: the supplied failure blob did not reproduce a failure."
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
            // The engine resolved whatever the caller left unset (its profile, hegel.toml, the
            // HEGEL_* variables); the run and its reporters see the effective configuration.
            Settings effective = settings.resolved(
                    lib.settingsGetTestCases(s),
                    lib.settingsGetPrintBlob(s),
                    NondeterminismStrictness.fromCode(lib.settingsGetNondeterminismStrictness(s)),
                    Verbosity.fromCode(lib.settingsGetVerbosity(s)));
            reporter.runStarted(effective);
            RunStatistics.Counter counts = new RunStatistics.Counter();
            List<Failure> explicitFailures = List.of();
            if (!effective.examples.isEmpty()
                    && effective.reproduceFailure == null
                    && (lib.settingsGetPhases(s) & Abi.PHASE_EXPLICIT) != 0) {
                boolean reportMultiple = lib.settingsGetReportMultipleFailures(s);
                explicitFailures = runExamples(lib, effective, body, reporter, reportMultiple);
                if (!explicitFailures.isEmpty() && !reportMultiple) {
                    return reportFailures(reporter, counts, explicitFailures, null);
                }
            }
            long run = effective.reproduceFailure == null
                    ? lib.runStart(s, reporter::engineOutput)
                    : lib.runStartBlob(s, effective.reproduceFailure, reporter::engineOutput);
            try {
                return drive(lib, run, effective, body, reporter, counts, explicitFailures);
            } finally {
                lib.runFree(run);
            }
        });
        reporter.runFinished(report);
        return report;
    }

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
            boolean rejected = false;
            try {
                body.accept(testCase);
            } catch (InvalidExample e) {
                throw e;
            } catch (AssumeRejected e) {
                rejected = true;
            } catch (LoopDone e) {
            } catch (Throwable e) {
                failure = e;
            }
            List<String> output = draws.output();
            if (verbose) {
                for (String line : output) {
                    reporter.caseOutput(line, false);
                }
                reporter.caseFinished(
                        rejected
                                ? CaseOutcome.INVALID
                                : failure == null ? CaseOutcome.VALID : CaseOutcome.INTERESTING,
                        false);
            }
            if (rejected) {
                return null;
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
     * Run {@code body} on the engine's settings handle for {@code settings}: resolved from its
     * profile (the default one unless {@link Settings#fromProfile} named another), with the
     * caller's explicit settings applied over it. The handle is freed when {@code body} returns.
     */
    private static <T> T withSettings(Libhegel lib, Settings settings, LongFunction<T> body) {
        long s = newSettings(lib, settings.profile);
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
    private static long newSettings(Libhegel lib, String profile) {
        long[] out = new long[1];
        if (profile == null) {
            checkUserCall(lib, lib.settingsNew(out), "hegel_settings_new");
        } else {
            checkUserCall(lib, lib.settingsNewForProfile(profile, out), "hegel_settings_new_for_profile");
        }
        return out[0];
    }

    /** Fail with the engine's diagnostic unless {@code profile} resolves. */
    static void checkProfile(Libhegel lib, String profile) {
        lib.settingsFree(newSettings(lib, profile));
    }

    /** Register {@code settings}, as the engine resolves them now, as the profile {@code name}. */
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
     * Pump the run to completion, keeping every interesting case's capture per origin, and
     * translate its verdict.
     */
    private static RunReport drive(
            Libhegel lib,
            long run,
            Settings settings,
            Consumer<TestCase> body,
            Reporter reporter,
            RunStatistics.Counter counts,
            List<Failure> explicitFailures) {
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
            return finish(lib, result, settings, reporter, counts, captures, explicitFailures);
        } finally {
            lib.runResultFree(result);
        }
    }

    /**
     * Translate a drained run's result into a report. Failures of explicit examples come first,
     * and make the run a failure whatever the engine's verdict.
     */
    private static RunReport finish(
            Libhegel lib,
            long result,
            Settings settings,
            Reporter reporter,
            RunStatistics.Counter counts,
            Map<String, CaseRun> captures,
            List<Failure> explicitFailures) {
        boolean replayingBlob = settings.reproduceFailure != null;
        switch (lib.runResultStatus(result)) {
            case Abi.RUN_STATUS_PASSED:
                if (replayingBlob) {
                    throw new HegelException(STALE_BLOB);
                }
                if (!explicitFailures.isEmpty()) {
                    return reportFailures(reporter, counts, explicitFailures, null);
                }
                return new RunReport(RunStatus.PASSED, counts.snapshot(), List.of());
            case Abi.RUN_STATUS_ERROR:
                // The run produced no verdict on the property: a failed health check, a
                // nondeterminism abort under ERROR strictness, an engine panic. (A blob the engine
                // cannot decode never gets this far: hegel_run_start_blob rejects it.)
                String error = nullToEmpty(lib.runResultError(result));
                if (replayingBlob) {
                    throw new HegelException("reproduceFailure: the blob replay ended in an error: " + error);
                }
                int[] healthCheck = new int[1];
                RunErrorKind kind = RunErrorKind.fromCode(lib.runResultErrorKind(result, healthCheck));
                RunReport errored = new RunReport(
                        RunStatus.ERROR,
                        counts.snapshot(),
                        error,
                        kind,
                        HealthCheck.fromBit(healthCheck[0]),
                        List.of());
                if (!explicitFailures.isEmpty()) {
                    return reportFailures(reporter, counts, explicitFailures, errored);
                }
                return errored;
            default:
                List<Failure> failures = new ArrayList<>(explicitFailures);
                failures.addAll(engineFailures(lib, result, captures, explicitFailures));
                return reportFailures(reporter, counts, failures, null);
        }
    }

    /**
     * Build every distinct failure the engine reports from its origin's capture, skipping those
     * an explicit example already failed with.
     */
    private static List<Failure> engineFailures(
            Libhegel lib, long result, Map<String, CaseRun> captures, List<Failure> explicitFailures) {
        Set<String> reported = new HashSet<>();
        for (Failure failure : explicitFailures) {
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
     * message and classification when the engine's run errored after explicit examples failed.
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
                testCase.releaseBlocks();
            }
        } finally {
            // The handles are caller-owned. On the error paths above the case may be incomplete;
            // the run still holds its own reference and completes it when freed.
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
        } catch (LibhegelException e) {
            // A binding/engine error, not a property failure: abort the whole run.
            throw e;
        } catch (Throwable e) {
            outcome = CaseOutcome.INTERESTING;
            origin = originOf(e, settings.infrastructurePackages);
            interesting = e;
        }
        List<String> output = testCase.output();
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
     * Send the caller's explicit settings to the handle. Anything left unset keeps the value the
     * engine resolved from its profile (a {@code hegel.toml}, or the shipped {@code ci}/{@code
     * workload} profiles it selects from the environment) and the {@code HEGEL_*} variables, which
     * is what makes explicit Java settings win over both.
     */
    static void applySettings(Libhegel lib, long s, Settings st) {
        if (st.testCases != null) {
            lib.settingsTestCases(s, st.testCases);
        }
        if (st.verbosity.code != null) {
            lib.settingsVerbosity(s, st.verbosity.code);
        }
        if (st.hasSeed) {
            lib.settingsSeed(s, st.seed, true);
        }
        if (st.derandomize != null) {
            lib.settingsDerandomize(s, st.derandomize);
        }
        if (st.reportMultipleFailures != null) {
            lib.settingsReportMultipleFailures(s, st.reportMultipleFailures);
        }
        if (st.stopOnFirstFailure != null) {
            lib.settingsStopOnFirstFailure(s, st.stopOnFirstFailure);
        }
        if (st.showStatistics != null) {
            lib.settingsShowStatistics(s, st.showStatistics);
        }
        if (st.unboundedChoices != null) {
            lib.settingsUnboundedChoices(s, st.unboundedChoices);
        }
        if (st.printBlob != null) {
            lib.settingsPrintBlob(s, st.printBlob);
        }
        if (st.nondeterminismStrictness.code != null) {
            lib.settingsNondeterminismStrictness(s, st.nondeterminismStrictness.code);
        }
        if (st.backend.code != null) {
            lib.settingsBackend(s, st.backend.code);
        }
        if (st.suppressMask != null) {
            lib.settingsSuppressHealthCheck(s, st.suppressMask);
        }
        if (st.phasesMask != null) {
            lib.settingsPhases(s, st.phasesMask);
        }

        switch (st.database.kind) {
            case DISABLED:
                lib.settingsDatabase(s, "");
                break;
            case PATH:
                lib.settingsDatabase(s, st.database.path);
                break;
            default:
                // Unset: the engine's profile decides (the ci and workload profiles disable it).
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
