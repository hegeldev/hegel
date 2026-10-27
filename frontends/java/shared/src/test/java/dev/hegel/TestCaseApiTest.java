package dev.hegel;

import static dev.hegel.Generators.integers;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.Test;

/** {@code reject}, {@code drawSilent}, events, {@code clone} and {@code repeat}. */
class TestCaseApiTest {
    private static final Settings SETTINGS = new Settings().database(Database.disabled());

    private static final class Lines implements Reporter {
        final List<String> engine = new ArrayList<>();

        @Override
        public void engineOutput(String line) {
            engine.add(line);
        }
    }

    @Test
    void rejectDiscardsTheCase() {
        AtomicInteger negatives = new AtomicInteger();
        Hegel.test(
                tc -> {
                    int x = tc.draw(integers());
                    if (x < 0) {
                        tc.reject();
                        negatives.incrementAndGet();
                    }
                    assertTrue(x >= 0);
                },
                SETTINGS);
        assertEquals(0, negatives.get());
    }

    @Test
    void silentDrawsAreNeitherPrintedNorRecorded() {
        RunReport report = Hegel.run(
                tc -> {
                    int hidden = tc.drawSilent(integers().min(0).max(10));
                    int shown = tc.draw(integers().min(0).max(10), "shown");
                    assertTrue(hidden + shown < 15);
                },
                SETTINGS,
                Reporter.silent());
        Failure failure = report.failures().get(0);
        assertEquals(List.of("shown"), List.copyOf(failure.draws().keySet()));
        assertEquals(1, failure.output().size());
        assertTrue(
                failure.output().get(0).startsWith("var shown = "),
                failure.output().get(0));
    }

    @Test
    void eventsAppearInTheStatistics() {
        Lines lines = new Lines();
        Hegel.run(
                tc -> {
                    int x = tc.draw(integers().min(0).max(10));
                    if (x > 5) {
                        tc.event("big x");
                    }
                    tc.eventValue("x value", x);
                },
                SETTINGS.showStatistics(true),
                lines);
        String all = String.join("\n", lines.engine);
        assertTrue(all.contains("big x"), all);
        assertTrue(all.contains("x value"), all);
    }

    @Test
    void aNonFiniteEventValueIsAnError() {
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(tc -> tc.eventValue("v", Double.NaN), SETTINGS, Reporter.silent()));
        assertTrue(e.getMessage().contains("requires a finite value"), e.getMessage());
    }

    @Test
    void anIllegalArgumentFromTheTestItselfIsAFailure() {
        FakeLibhegel fake = new FakeLibhegel();
        IllegalArgumentException error = new IllegalArgumentException("the code under test rejected it");
        assertSame(error, repeatOnceThrowing(fake, () -> {
            throw error;
        }));
        assertTrue(fake.repeatFailed);
    }

    @Test
    void clonesDrawOnOtherThreads() {
        RunReport report = Hegel.run(
                tc -> {
                    TestCase clone = tc.clone();
                    clone.setWorker(1);
                    int b = CompletableFuture.supplyAsync(
                                    () -> clone.draw(integers().min(0).max(10), "b"))
                            .join();
                    int a = tc.draw(integers().min(0).max(10), "a");
                    assertTrue(a + b < 12);
                },
                SETTINGS,
                Reporter.silent());
        Failure failure = report.failures().get(0);
        assertTrue(
                failure.output().stream().anyMatch(l -> l.contains("[worker 1") && l.contains("b = ")),
                String.join("\n", failure.output()));
    }

    @Test
    void repeatRunsUntilTheEngineStopsIt() {
        AtomicInteger after = new AtomicInteger();
        AtomicInteger repetitions = new AtomicInteger();
        Hegel.test(
                tc -> {
                    tc.repeat(() -> {
                        tc.draw(integers());
                        repetitions.incrementAndGet();
                    });
                    after.incrementAndGet();
                },
                SETTINGS.testCases(5).suppressHealthCheck(HealthCheck.TOO_SLOW));
        assertEquals(0, after.get());
        assertTrue(repetitions.get() > 0);
    }

    @Test
    void aFailingRepetitionShrinksAndIsIndented() {
        RunReport report = Hegel.run(
                tc -> {
                    List<Integer> seen = new ArrayList<>();
                    tc.repeat(() -> {
                        int x = tc.draw(integers().min(0).max(100), "x");
                        tc.assume(x != 1);
                        seen.add(x);
                        assertTrue(seen.size() < 3);
                    });
                },
                SETTINGS,
                Reporter.silent());
        Failure failure = report.failures().get(0);
        assertInstanceOf(AssertionError.class, failure.exception());
        assertEquals(
                List.of(
                        "// Repetition #1",
                        "  var x = 0;",
                        "// Repetition #2",
                        "  var x = 0;",
                        "// Repetition #3",
                        "  var x = 0;"),
                failure.output());
    }

    @Test
    void aRuntimeExceptionInARepetitionIsAFailure() {
        RunReport report = Hegel.run(
                tc -> tc.repeat(() -> {
                    throw new IllegalStateException("boom");
                }),
                SETTINGS,
                Reporter.silent());
        assertInstanceOf(IllegalStateException.class, report.failures().get(0).exception());
    }

    // Control flow through a repetition, over a fake engine.

    private static TestCase fakeCase(FakeLibhegel fake) {
        return new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false);
    }

    private static Throwable repeatOnceThrowing(FakeLibhegel fake, Runnable body) {
        fake.moreSequence = new boolean[] {true, false};
        return assertThrows(Throwable.class, () -> fakeCase(fake).repeat(body));
    }

    @Test
    void controlFlowEscapesARepetitionWithoutMarkingIt() {
        List<RuntimeException> controls =
                List.of(new StopTest(), new LoopDone(), new InvalidArgument("bad"), new HegelException("engine"));
        for (RuntimeException control : controls) {
            FakeLibhegel fake = new FakeLibhegel();
            assertSame(control, repeatOnceThrowing(fake, () -> {
                throw control;
            }));
            assertFalse(fake.repeatFailed, control.toString());
        }
    }

    @Test
    void anEngineRejectionEndsTheLoop() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.moreSequence = new boolean[] {true, true};
        TestCase tc = fakeCase(fake);
        assertThrows(
                AssumeRejected.class,
                () -> tc.repeat(() -> {
                    fake.generateIntegerRc = Abi.E_ASSUME;
                    tc.draw(integers());
                }));
        assertFalse(fake.repeatFailed);
    }

    @Test
    void anErrorInARepetitionMarksItAndPropagates() {
        FakeLibhegel fake = new FakeLibhegel();
        AssertionError error = new AssertionError("nope");
        assertSame(error, repeatOnceThrowing(fake, () -> {
            throw error;
        }));
        assertTrue(fake.repeatFailed);
        assertEquals(List.of(fake.repeatId), fake.freedRepeats);
    }

    @Test
    void aRejectedRepetitionIsSkipped() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.moreSequence = new boolean[] {true, true, false};
        AtomicInteger runs = new AtomicInteger();
        TestCase tc = fakeCase(fake);
        assertThrows(
                LoopDone.class,
                () -> tc.repeat(() -> {
                    runs.incrementAndGet();
                    tc.reject();
                }));
        assertEquals(2, runs.get());
        assertEquals(2, fake.blockParents.size());
        assertEquals(2, fake.freedBlocks.size());
        tc.releaseAll();
        assertEquals(2, fake.freedBlocks.size());
    }
}
