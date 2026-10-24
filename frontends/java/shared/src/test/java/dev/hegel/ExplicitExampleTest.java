package dev.hegel;

import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.lists;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class ExplicitExampleTest {
    private static final Settings SETTINGS = new Settings().database(Database.disabled());

    /** Records every reporter callback as a line. */
    private static final class Recorder implements Reporter {
        final List<String> events = new ArrayList<>();

        @Override
        public void caseOutput(String line, boolean finalReplay) {
            events.add((finalReplay ? "final: " : "live: ") + line);
        }

        @Override
        public void failuresFound(int count) {
            events.add("failures: " + count);
        }
    }

    @Test
    void examplesRunBeforeGeneration() {
        List<Object> seen = new ArrayList<>();
        Hegel.test(
                tc -> {
                    int x = tc.draw(integers(), "x");
                    int y = tc.draw(integers().min(0).max(10));
                    seen.add(List.of(x, y));
                },
                SETTINGS.testCases(3)
                        .examples(
                                Example.of(-7).with("x", 1000),
                                Example.of(5, 6),
                                Example.of().with("x", 1).with("draw_1", 2)));
        assertEquals(List.of(List.of(1000, -7), List.of(5, 6), List.of(1, 2)), seen.subList(0, 3));
    }

    @Test
    void aFailingExampleIsReportedWithItsValues() {
        Recorder recorder = new Recorder();
        RunReport report = Hegel.run(
                tc -> {
                    List<Integer> xs = tc.draw(lists(integers()), "xs");
                    tc.note("checking");
                    assertTrue(xs.size() != 2, "two");
                },
                SETTINGS.examples(Example.of(List.of(3, 4))),
                recorder);
        Failure failure = report.failures().get(0);
        assertTrue(failure.exception().getMessage().startsWith("two"));
        assertEquals(List.of("xs = [3, 4];", "checking"), failure.output());
        assertEquals(Map.of("xs", List.of(3, 4)), failure.draws());
        assertTrue(failure.reproduceBlob().isEmpty());
        assertEquals(List.of("failures: 1", "final: xs = [3, 4];", "final: checking"), recorder.events);
    }

    @Test
    void verboseRunsReportExamplesAsTheyRun() {
        Recorder recorder = new Recorder();
        Hegel.run(
                tc -> tc.draw(integers(), "x"),
                SETTINGS.testCases(1).verbosity(Verbosity.VERBOSE).examples(Example.of("a\nb")),
                recorder);
        assertEquals("live: Running explicit Example(\"a\\nb\")", recorder.events.get(0));
        assertEquals("live: x = \"a\\nb\";", recorder.events.get(1));

        Recorder failing = new Recorder();
        Hegel.run(
                tc -> {
                    throw new IllegalStateException();
                },
                SETTINGS.verbosity(Verbosity.VERBOSE).examples(Example.of()),
                failing);
        assertEquals("live: Running explicit Example()", failing.events.get(0));
    }

    @Test
    void theFirstFailingExampleEndsTheRun() {
        List<Object> seen = new ArrayList<>();
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers());
                    seen.add(x);
                    assertTrue(x < 10);
                },
                SETTINGS.examples(Example.of(1), Example.of(11), Example.of(12)),
                Reporter.silent());
        assertEquals(List.of(1, 11), seen);
        assertEquals(1, report.failures().size());
        assertEquals(0, report.statistics().total());
    }

    @Test
    void withMultipleFailuresTheEngineStillRuns() {
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(100), "x");
                    if (x == 1000) {
                        throw new IllegalStateException("explicit");
                    }
                    if (x == 2000) {
                        throw new UnsupportedOperationException("also explicit");
                    }
                    assertTrue(x < 50, "generated");
                },
                SETTINGS.reportMultipleFailures(true).examples(Example.of(1000), Example.of(2000), Example.of(60)),
                Reporter.silent());
        List<String> messages = new ArrayList<>();
        for (Failure failure : report.failures()) {
            messages.add(failure.exception().getMessage().split(" ==>")[0]);
        }
        assertEquals(List.of("explicit", "also explicit", "generated"), messages);
    }

    @Test
    void anExplicitFailureSharingAnOriginWithAGeneratedOneIsReportedOnce() {
        RunReport report = Hegel.run(
                tc -> assertTrue(tc.draw(integers().min(0).max(100), "x") < 50),
                SETTINGS.reportMultipleFailures(true).examples(Example.of(70), Example.of(80)),
                Reporter.silent());
        assertEquals(1, report.failures().size());
        assertEquals(Map.of("x", 70), report.failures().get(0).draws());
        assertTrue(report.failures().get(0).reproduceBlob().isEmpty());
    }

    @Test
    void anExplicitFailureOutranksAPassingRun() {
        RunReport report = Hegel.run(
                tc -> assertTrue(tc.draw(integers().min(0).max(100)) <= 100),
                SETTINGS.reportMultipleFailures(true).examples(Example.of(101)),
                Reporter.silent());
        assertEquals(RunStatus.FAILED, report.status());
        assertFalse(report.error().isPresent());
    }

    @Test
    void anExplicitFailureOutranksAnErroredRun() {
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(100));
                    assertTrue(x != 101);
                    tc.assume(false);
                },
                SETTINGS.reportMultipleFailures(true).examples(Example.of(101)),
                Reporter.silent());
        assertEquals(RunStatus.FAILED, report.status());
        assertEquals(1, report.failures().size());
        assertTrue(report.error().isPresent());
        assertTrue(report.errorKind().isPresent());
        AssertionError thrown = assertThrows(AssertionError.class, report::throwIfFailed);
        assertSame(report.failures().get(0).exception(), thrown);
    }

    @Test
    void examplesAreSkippedOutsideTheExplicitPhase() {
        List<Object> seen = new ArrayList<>();
        Hegel.test(
                tc -> seen.add(tc.draw(integers().min(0).max(10))),
                SETTINGS.testCases(5).phases(Phase.GENERATE).examples(Example.of(1000)));
        assertFalse(seen.contains(1000));
    }

    @Test
    void examplesAreSkippedWhenReplayingABlob() {
        RunReport failing = Hegel.run(
                tc -> assertTrue(tc.draw(integers().min(0).max(10)) < 5), SETTINGS.printBlob(true), Reporter.silent());
        String blob = failing.failures().get(0).reproduceBlob().orElseThrow();
        List<Object> seen = new ArrayList<>();
        RunReport replay = Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(10));
                    seen.add(x);
                    assertTrue(x < 5);
                },
                SETTINGS.reproduceFailure(blob).examples(Example.of(1000)),
                Reporter.silent());
        assertFalse(seen.contains(1000));
        assertEquals(RunStatus.FAILED, replay.status());
    }

    @Test
    void misfittingExamplesAreUsageErrors() {
        assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(tc -> tc.draw(integers()), SETTINGS.examples(Example.of()), Reporter.silent()));
        IllegalArgumentException unused = assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(
                        tc -> tc.draw(integers()),
                        SETTINGS.examples(Example.of(1, 2).with("y", 3)),
                        Reporter.silent()));
        assertTrue(unused.getMessage().contains("no draw is named y"), unused.getMessage());
        assertTrue(unused.getMessage().contains("1 positional value(s)"), unused.getMessage());
        IllegalArgumentException onlyNamed = assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(tc -> {}, SETTINGS.examples(Example.of().with("y", 3)), Reporter.silent()));
        assertFalse(onlyNamed.getMessage().contains("positional"), onlyNamed.getMessage());
        assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(tc -> tc.drawSilent(integers()), SETTINGS.examples(Example.of()), Reporter.silent()));
    }

    @Test
    void anExampleFailingAnAssumptionIsSkipped() {
        List<Object> seen = new ArrayList<>();
        List<CaseOutcome> outcomes = new ArrayList<>();
        Reporter recorder = new Reporter() {
            @Override
            public void caseFinished(CaseOutcome outcome, boolean finalReplay) {
                outcomes.add(outcome);
            }
        };
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.draw(integers(), "x");
                    if (x == 1000) {
                        seen.add(x);
                        tc.assume(false);
                    }
                    if (x == 2000) {
                        seen.add(x);
                        tc.reject();
                    }
                },
                SETTINGS.testCases(1)
                        .verbosity(Verbosity.VERBOSE)
                        .examples(Example.of(1000, "never drawn"), Example.of(2000)),
                recorder);
        assertTrue(report.passed());
        assertEquals(List.of(1000, 2000), seen);
        assertEquals(CaseOutcome.INVALID, outcomes.get(0));
        assertEquals(CaseOutcome.INVALID, outcomes.get(1));
    }

    @Test
    void aSilentDrawTakesTheNextPositionalValue() {
        Recorder recorder = new Recorder();
        RunReport report = Hegel.run(
                tc -> {
                    int x = tc.drawSilent(integers());
                    int y = tc.draw(integers(), "y");
                    assertTrue(x != 3 || y != 4, "both");
                },
                SETTINGS.testCases(1).examples(Example.of(3, 4)),
                recorder);
        assertFalse(report.passed());
        assertEquals(List.of("y = 4;"), report.failures().get(0).output());
    }

    @Test
    void aForkOfAnExampleDrawsFromTheSameValues() throws Exception {
        List<Object> seen = new ArrayList<>();
        Hegel.test(
                tc -> {
                    TestCase fork = tc.fork(1);
                    int[] drawn = new int[1];
                    Thread thread = new Thread(() -> drawn[0] = fork.draw(integers(), "x"));
                    thread.start();
                    try {
                        thread.join();
                    } catch (InterruptedException e) {
                        throw new AssertionError(e);
                    }
                    if (seen.isEmpty()) {
                        seen.add(drawn[0]);
                        seen.add(tc.draw(integers()));
                    }
                },
                SETTINGS.testCases(1).examples(Example.of(2).with("x", 1)));
        assertEquals(List.of(1, 2), seen);
    }

    @Test
    void anExampleRunsARepeatBodyOnce() {
        List<Object> seen = new ArrayList<>();
        Hegel.test(
                tc -> tc.repeat(() -> {
                    int x = tc.draw(integers(), "x");
                    if (seen.size() < 1) {
                        seen.add(x);
                    }
                }),
                SETTINGS.testCases(1).examples(Example.of(7)));
        assertEquals(List.of(7), seen);

        Hegel.test(
                tc -> tc.repeat(() -> tc.assume(tc.draw(integers()) != 8)),
                SETTINGS.testCases(1).examples(Example.of(8)));

        RunReport failing = Hegel.run(
                tc -> tc.repeat(() -> assertTrue(tc.draw(integers(), "x") != 9, "nine")),
                SETTINGS.testCases(1).examples(Example.of(9)),
                Reporter.silent());
        assertFalse(failing.passed());
        assertEquals(
                List.of("// Repetition #1", "x = 9;"), failing.failures().get(0).output());

        IllegalArgumentException unused = assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(
                        tc -> tc.repeat(() -> tc.draw(integers())),
                        SETTINGS.examples(Example.of(1, 2)),
                        Reporter.silent()));
        assertTrue(unused.getMessage().contains("1 positional value(s)"), unused.getMessage());
    }

    @Test
    void anExplicitExampleIgnoresTargetsAndEvents() {
        Hegel.test(
                tc -> {
                    tc.target(1.0);
                    tc.event("e");
                    tc.eventValue("v", 2.0);
                },
                SETTINGS.testCases(1).examples(Example.of()));
    }

    @Test
    void aNamedValueIsUsedOnceThenPositionalOnes() {
        List<Object> seen = new ArrayList<>();
        Hegel.test(
                tc -> {
                    int first = tc.draw(integers(), "x");
                    int second = tc.draw(integers(), "x");
                    if (seen.isEmpty()) {
                        seen.add(first);
                        seen.add(second);
                    }
                },
                SETTINGS.testCases(1).examples(Example.of(2).with("x", 1)));
        assertEquals(List.of(1, 2), seen);
    }

    @Test
    void examplesDescribeThemselves() {
        assertEquals(
                "Example(1, null, \"s\", k=[1])",
                Example.of(1, null, "s").with("k", Arrays.asList(1)).toString());
    }
}
