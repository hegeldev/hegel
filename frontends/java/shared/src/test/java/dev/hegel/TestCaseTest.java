package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class TestCaseTest {
    /** A TestCase over a fake binding, stamped for capture as {@code captured} and not verbose. */
    private static TestCase newCase(FakeLibhegel fake, boolean captured) {
        return new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), captured);
    }

    @Test
    void drawPrintsTopLevelWithLabelAndDefaultName() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, true);
        tc.draw(constant(1), "x");
        tc.draw(constant(2));
        assertEquals(List.of("var x = 1;", "var draw_1 = 2;"), tc.output());
        // The case fetched one printer for its draws and freed it; output() read the document
        // through a second root printer, also freed.
        assertEquals(List.copyOf(fake.printerOwners.keySet()), fake.freedPrinters);
        assertEquals(2, fake.freedPrinters.size());
    }

    @Test
    void nothingIsPrintedWhenTheCaseIsNeitherCapturedNorVerbose() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, false);
        assertFalse(tc.isFinal());
        assertEquals(5, tc.draw(constant(5), "x"));
        tc.note("nope");
        assertEquals(List.of(), tc.output());
        assertTrue(tc.draws().isEmpty());
        assertTrue(fake.notes.isEmpty());
        assertTrue(fake.printerOwners.isEmpty());
    }

    @Test
    void nestedDrawsAreNotPrinted() {
        TestCase tc = newCase(new FakeLibhegel(), true);
        Generator<Integer> nested = new Generator<>() {
            @Override
            public Integer doDraw(TestCase inner) {
                int a = inner.draw(constant(10), "inner");
                return a + 1;
            }
        };
        tc.draw(nested.printAsRepr(), "top");
        assertEquals(List.of("var top = 11;"), tc.output());
        assertEquals(Map.of("top", 11), tc.draws());
    }

    @Test
    void notesGoToTheEngine() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, true);
        tc.note("hello\nworld");
        assertEquals(List.of("hello\nworld"), fake.notes);
        assertEquals(List.of("hello", "world"), tc.output());
    }

    @Test
    void capturedCaseRecordsDrawsInOrder() {
        TestCase tc = newCase(new FakeLibhegel(), true);
        assertTrue(tc.isFinal());
        tc.draw(constant(1), "x");
        tc.note("first");
        tc.draw(constant(2));
        assertEquals(List.of("x", "draw_1"), List.copyOf(tc.draws().keySet()));
        assertEquals(Map.of("x", 1, "draw_1", 2), tc.draws());
        assertEquals(List.of("var x = 1;", "first", "var draw_1 = 2;"), tc.output());
    }

    @Test
    void repeatedNamesAreNumberedFromTheirSecondUse() {
        TestCase tc = newCase(new FakeLibhegel(), true);
        for (int i = 1; i <= 3; i++) {
            tc.draw(constant(i), "x");
        }
        tc.draw(constant(8));
        tc.draw(constant(9));
        assertEquals(
                List.of("x", "x_2", "x_3", "draw_1", "draw_2"),
                List.copyOf(tc.draws().keySet()));
        assertEquals(
                List.of("var x = 1;", "var x_2 = 2;", "var x_3 = 3;", "var draw_1 = 8;", "var draw_2 = 9;"),
                tc.output());
    }

    @Test
    void notesMadeMidDrawFollowTheDrawLine() {
        TestCase tc = newCase(new FakeLibhegel(), true);
        Generator<Integer> noisy = new Generator<>() {
            @Override
            public Integer doDraw(TestCase inner) {
                inner.note("inside");
                return 7;
            }
        };
        tc.note("before");
        tc.draw(noisy.printAsRepr(), "v");
        tc.note("after");
        assertEquals(List.of("before", "var v = 7;", "inside", "after"), tc.output());
    }

    @Test
    void aFailingDrawPrintsNothingButKeepsItsNotes() {
        TestCase tc = newCase(new FakeLibhegel(), true);
        Generator<Integer> failing = new Generator<>() {
            @Override
            public Integer doDraw(TestCase inner) {
                inner.note("about to fail");
                throw new IllegalStateException("inside");
            }
        };
        assertThrows(IllegalStateException.class, () -> tc.draw(failing.printAsRepr(), "v"));
        assertEquals(List.of("about to fail"), tc.output());
        assertTrue(tc.draws().isEmpty());
    }

    @Test
    void verboseCasesPrintWithoutRecording() {
        TestCase tc = new TestCase(new LiveDataSource(new FakeLibhegel(), FakeLibhegel.TC), false, true);
        assertFalse(tc.isFinal());
        tc.draw(constant(1), "x");
        tc.note("n");
        assertEquals(List.of("var x = 1;", "n"), tc.output());
        assertTrue(tc.draws().isEmpty());
    }

    @Test
    void anEmptyDocumentHasNoLines() {
        assertEquals(List.of(), newCase(new FakeLibhegel(), true).output());
    }

    @Test
    void anUnreadableDocumentIsABindingError() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.printerValueRc = Abi.E_INVALID_ARG;
        TestCase tc = newCase(fake, true);
        HegelException e = assertThrows(HegelException.class, tc::output);
        assertTrue(e.getMessage().contains("hegel_printer_value failed"), e.getMessage());
    }

    @Test
    void blocksIndentTheirOutputAndNameTheirDrawsInAFreshScope() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, true);
        tc.draw(constant(1), "x");
        tc.note("Step 1: rule {");
        TestCase block = tc.block(2);
        block.draw(constant(2), "x");
        block.draw(constant(3), "x");
        block.note("inner");
        block.endBlock();
        tc.note("}");
        tc.draw(constant(4), "x");
        assertEquals(
                List.of(
                        "var x = 1;",
                        "Step 1: rule {",
                        "  var x = 2;",
                        "  var x_2 = 3;",
                        "  inner",
                        "}",
                        "var x_2 = 4;"),
                tc.output());
        assertEquals(Map.of("x", 1, "x_2", 2, "x_2_2", 3, "x_2_3", 4), tc.draws());
        assertEquals(List.of(FakeLibhegel.BLOCK_BASE), fake.freedBlocks);
        tc.releaseAll();
        assertEquals(List.of(FakeLibhegel.BLOCK_BASE), fake.freedBlocks);
        assertEquals(fake.printerOwners.size(), fake.freedPrinters.size());
    }

    @Test
    void aPinnedBlockLivesUntilTheCaseIsOver() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, true);
        TestCase block = tc.block(2);
        block.pin();
        block.pin();
        block.endBlock();
        assertTrue(fake.freedBlocks.isEmpty());
        tc.pin();
        tc.releaseAll();
        assertEquals(List.of(FakeLibhegel.BLOCK_BASE), fake.freedBlocks);
    }

    @Test
    void anEndedBlockCannotBeUsed() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, true);
        TestCase block = tc.block(2);
        block.endBlock();
        assertThrows(InvalidArgument.class, () -> block.draw(constant(1)));
        assertThrows(InvalidArgument.class, () -> block.draw(Generators.integers()));
        assertThrows(InvalidArgument.class, () -> block.note("late"));
    }

    @Test
    void blocksShareTheStreamsAbortState() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, false);
        TestCase block = tc.block(2);
        fake.generateIntegerRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> block.draw(Generators.integers()));
        assertTrue(tc.isAborted());
        block.endBlock();
    }

    @Test
    void aFailedBlockOrPrinterIsABindingError() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, true);
        fake.testCaseBlockRc = Abi.E_INVALID_HANDLE;
        assertThrows(HegelException.class, () -> tc.block(2));
        fake.testCasePrinterRc = Abi.E_INVALID_HANDLE;
        assertThrows(HegelException.class, () -> tc.draw(constant(1)));
    }

    @Test
    void spanOpensAndClosesAroundTheBody() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, false);
        assertEquals(7, tc.span(Label.of("test.pair"), () -> 7));
        assertEquals(List.of(Label.of("test.pair")), fake.startedSpans);
        assertEquals(1, fake.stoppedSpans);
        // The span is closed on the exceptional path too.
        assertThrows(
                IllegalStateException.class,
                () -> tc.span(Label.TUPLE, () -> {
                    throw new IllegalStateException("inside");
                }));
        assertEquals(2, fake.stoppedSpans);
    }

    @Test
    void assumeAndTarget() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = newCase(fake, false);
        tc.assume(true);
        assertThrows(AssumeRejected.class, () -> tc.assume(false));
        tc.target(3.5);
        tc.target(9.0, "score");
        fake.targetRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> tc.target(1.0));
    }

    private static PrintableGenerator<Integer> constant(int v) {
        return Generators.just(v);
    }
}
