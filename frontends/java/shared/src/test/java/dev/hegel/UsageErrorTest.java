package dev.hegel;

import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.text;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.Test;

/**
 * Usage errors detected while a test runs fail the run at once, and errors printing a test case's
 * output never replace the failure the test body raised.
 */
class UsageErrorTest {
    private static final Settings SETTINGS = new Settings().database(Database.disabled());

    @Test
    void anArgumentTheEngineRejectsWhenDrawingFailsTheRunWithoutShrinking() {
        AtomicInteger calls = new AtomicInteger();
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(
                        tc -> {
                            calls.incrementAndGet();
                            tc.draw(text().codec("nonsense"));
                        },
                        SETTINGS,
                        Reporter.silent()));
        assertTrue(e.getMessage().contains("nonsense"), e.getMessage());
        assertEquals(1, calls.get());
    }

    @Test
    void aDrawLabelWithANewlineIsAUsageErrorFromTheFirstCase() {
        AtomicInteger calls = new AtomicInteger();
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> Hegel.run(
                        tc -> {
                            calls.incrementAndGet();
                            tc.draw(integers(), "a\nb");
                            throw new AssertionError("unreachable");
                        },
                        SETTINGS,
                        Reporter.silent()));
        assertEquals("draw label \"a\\nb\" must not contain a newline", e.getMessage());
        assertEquals(1, calls.get());
    }

    @Test
    void aFailingDrawKeepsItsExceptionWhenAbandoningItsLineFails() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.printerAbortRc = Abi.E_BACKEND;
        AssertionError error = new AssertionError("from the generator");
        TestCase tc = new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), true);
        AssertionError thrown = assertThrows(
                AssertionError.class,
                () -> tc.draw((PrintableGenerator<Object>) (t, p) -> {
                    throw error;
                }));
        assertSame(error, thrown);
        assertInstanceOf(HegelException.class, thrown.getSuppressed()[0]);
    }

    @Test
    void anErrorPrintingADrawAbandonsItsLine() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.printerTextRc = Abi.E_BACKEND;
        TestCase tc = new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), true);
        HegelException e = assertThrows(HegelException.class, () -> tc.draw(integers(), "x"));
        assertTrue(e.getMessage().contains("hegel_printer_text"), e.getMessage());
        assertEquals(0, e.getSuppressed().length);
    }

    @Test
    void anUnreadableDocumentDoesNotReplaceTheBodysFailure() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.runStatus = Abi.RUN_STATUS_FAILED;
        fake.failureBlobs.add(null);
        fake.printerValueRc = Abi.E_BACKEND;
        AssertionError error = new AssertionError("the property failed");
        RunReport report = Runner.run(
                fake,
                SETTINGS,
                tc -> {
                    tc.draw(integers(), "x");
                    throw error;
                },
                Reporter.silent());
        Failure failure = report.failures().get(0);
        assertSame(error, failure.exception());
        assertEquals(List.of(), failure.output());
        assertInstanceOf(HegelException.class, error.getSuppressed()[0]);
    }

    @Test
    void anUnreadableDocumentFailsAPassingCase() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.printerValueRc = Abi.E_BACKEND;
        assertThrows(HegelException.class, () -> Runner.run(fake, SETTINGS, tc -> {}, Reporter.silent()));
    }

    @Test
    void theRootPrinterIsFreedWhenTheBodyRaisesAnEngineError() {
        for (RuntimeException control : List.of(new HegelException("engine"), new InvalidArgument("usage"))) {
            FakeLibhegel fake = new FakeLibhegel();
            RuntimeException thrown = assertThrows(
                    RuntimeException.class,
                    () -> Runner.run(
                            fake,
                            SETTINGS,
                            tc -> {
                                tc.draw(integers(), "x");
                                throw control;
                            },
                            Reporter.silent()));
            assertSame(control, thrown);
            assertTrue(!fake.printerOwners.isEmpty());
            assertEquals(fake.printerOwners.size(), fake.freedPrinters.size());
        }
    }
}
