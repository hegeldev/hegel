package dev.hegel;

import static dev.hegel.Generators.integers;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.Test;

/**
 * The library-facing surface against the real engine: {@link Hegel#run} reports instead of
 * throwing, final replays are identifiable, and spans work from a foreign generator.
 */
class LibraryApiTest {
    private static final Settings QUIET =
            new Settings().database(Database.disabled()).seed(123);

    @Test
    void runReportsTheShrunkCounterexample() {
        AtomicInteger finals = new AtomicInteger();
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(1000), "x");
                    tc.note("observed x=" + x);
                    if (tc.wantsOutput()) {
                        finals.incrementAndGet();
                    }
                    assertTrue(x <= 10, "x was too big: " + x);
                },
                QUIET,
                Reporter.silent());
        assertEquals(RunStatus.FAILED, report.status());
        assertEquals(1, report.failures().size());
        Failure f = report.failures().get(0);
        assertEquals(Map.of("x", 11), f.draws());
        assertEquals(List.of("observed x=11"), f.notes());
        assertTrue(f.exception() instanceof AssertionError);
        assertTrue(f.exception().getMessage().contains("x was too big: 11"));
        assertTrue(f.origin().startsWith("AssertionFailedError at "), f.origin());
        assertTrue(f.reproduceBlob().isPresent());
        assertFalse(f.nondeterministic());
        assertEquals(Optional.empty(), f.caveat());
        // The engine stamped at least the final replay for capture, and the run counted more than
        // the stamped cases.
        assertTrue(finals.get() >= 1, "stamped cases: " + finals.get());
        assertTrue(report.statistics().interesting() >= 2, report.statistics().toString());
        assertTrue(report.statistics().total() > report.statistics().interesting());
    }

    @Test
    void passingRunReportsValidCasesOnly() {
        AtomicInteger finals = new AtomicInteger();
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(100));
                    if (tc.wantsOutput()) {
                        finals.incrementAndGet();
                    }
                    assertTrue(x >= 0);
                },
                QUIET.testCases(20),
                Reporter.silent());
        assertTrue(report.passed());
        assertEquals(0, finals.get());
        assertTrue(report.statistics().valid() >= 20, report.statistics().toString());
        assertEquals(report.statistics().valid(), report.statistics().total());
        assertTrue(report.failures().isEmpty());
    }

    @Test
    void healthCheckFailureIsReported() {
        RunReport report = Hegel.run(
                tc -> {
                    tc.draw(integers());
                    tc.assume(false);
                },
                QUIET,
                Reporter.silent());
        assertEquals(RunStatus.ERROR, report.status());
        assertTrue(report.healthCheckFailed(), report.error().orElse(""));
        assertTrue(report.statistics().invalid() > 0);
    }

    /** A foreign composite generator: two draws enclosed in a custom-labelled span. */
    private static final Generator<int[]> PAIR = tc -> tc.span(Label.of("dev.hegel.test.pair"), () -> new int[] {
        tc.draw(integers().min(0).max(50)), tc.draw(integers().min(0).max(50))
    });

    @Test
    void foreignGeneratorsCanOpenSpans() {
        RunReport report = Hegel.run(
                tc -> {
                    int[] p = tc.draw(PAIR, "p");
                    assertTrue(p[0] + p[1] < 60, "sum too big");
                },
                QUIET,
                Reporter.silent());
        assertEquals(RunStatus.FAILED, report.status());
        int[] shrunk = (int[]) report.failures().get(0).draws().get("p");
        // The engine shrinks the pair as a unit toward the boundary.
        assertEquals(60, shrunk[0] + shrunk[1]);
    }

    private static List<String> streamedEvents(Verbosity verbosity) {
        List<String> events = new ArrayList<>();
        Reporter recording = new Reporter() {
            @Override
            public void engineOutput(String line) {
                events.add("engine:" + line);
            }

            @Override
            public void draw(String label, Object value, boolean finalReplay) {
                if (!finalReplay) {
                    events.add("draw:" + label);
                }
            }

            @Override
            public void caseFinished(CaseOutcome outcome, boolean finalReplay) {
                if (!finalReplay) {
                    events.add("finished:" + outcome);
                }
            }
        };
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(1000), "x");
                    tc.assume(x % 7 != 3);
                    assertTrue(x <= 10, "x was too big: " + x);
                },
                QUIET.verbosity(verbosity),
                recording);
        assertEquals(RunStatus.FAILED, report.status());
        return events;
    }

    @Test
    void verboseRunsStreamTheSummaryAndStopLinesOfEachCase() {
        List<String> events = streamedEvents(Verbosity.VERBOSE);

        int invalid = events.indexOf("finished:INVALID");
        assertTrue(invalid > 0, events.toString());
        assertEquals("engine:Test case stopped: failed assumption", events.get(invalid - 1));

        int interesting = events.indexOf("finished:INTERESTING");
        assertTrue(interesting > 1, events.toString());
        String summary = events.get(interesting - 1);
        assertTrue(
                summary.matches("engine:\\S+\\.java:\\d+: org\\.opentest4j\\.AssertionFailedError:"
                        + " x was too big: \\d+ ==> expected: <true> but was: <false>"),
                summary);
        assertEquals("draw:x", events.get(interesting - 2));
    }

    @Test
    void debugRunsStreamTheStackTraceOfEachFailingCase() {
        List<String> events = streamedEvents(Verbosity.DEBUG);

        int interesting = events.indexOf("finished:INTERESTING");
        assertTrue(interesting > 0, events.toString());
        int line = interesting - 1;
        while (events.get(line).startsWith("engine:\tat ")) {
            line--;
        }
        assertTrue(interesting - line > 1, events.toString());
        assertTrue(
                events.get(line).startsWith("engine:org.opentest4j.AssertionFailedError: x was too big"),
                events.toString());
        assertEquals("draw:x", events.get(line - 1));
    }

    @Test
    void quietRunsReportTheShrunkCounterexampleWithoutAnEngineReport() {
        AtomicInteger finals = new AtomicInteger();
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(1000), "x");
                    if (tc.wantsOutput()) {
                        finals.incrementAndGet();
                    }
                    assertTrue(x <= 10, "x was too big: " + x);
                },
                QUIET.verbosity(Verbosity.QUIET),
                Reporter.silent());
        assertEquals(RunStatus.FAILED, report.status());
        Failure f = report.failures().get(0);
        assertTrue(
                f.exception().getMessage().contains("x was too big: 11"),
                f.exception().getMessage());
        assertEquals(Map.of("x", 11), f.draws());
        assertEquals(null, f.diagnostic());
        assertTrue(finals.get() >= 1, "stamped cases: " + finals.get());
    }

    @Test
    void testReturnsTheReportOfAPassedRun() {
        RunReport report = Hegel.test(tc -> tc.draw(integers()), QUIET.testCases(5), Reporter.silent());
        assertTrue(report.passed());
        assertEquals(RunStatus.PASSED, Hegel.run(tc -> {}).status());
    }
}
