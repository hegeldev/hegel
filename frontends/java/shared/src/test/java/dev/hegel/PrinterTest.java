package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalDouble;
import java.util.OptionalInt;
import java.util.OptionalLong;
import java.util.Set;
import java.util.UUID;
import java.util.function.Consumer;
import org.junit.jupiter.api.Test;

class PrinterTest {
    private static String render(Object value) {
        try (Document doc = new Document()) {
            doc.printer().repr(value);
            return doc.finish();
        }
    }

    private static String render(int maxWidth, Consumer<Printer> print) {
        try (Document doc = new Document().maxWidth(maxWidth)) {
            print.accept(doc.printer());
            return doc.finish();
        }
    }

    record Point(int x, int y) {}

    record Named(String name, List<String> tags) {}

    record Empty() {}

    record Broken(String value) {
        @Override
        public String value() {
            throw new IllegalStateException("unreadable");
        }
    }

    static final class Opaque {
        private final String shown;

        Opaque(String shown) {
            this.shown = shown;
        }

        @Override
        public String toString() {
            return shown;
        }
    }

    @Test
    void scalarsAndStringsRenderAsJavaLiterals() {
        assertEquals("null", render(null));
        assertEquals("42", render(42));
        assertEquals("true", render(true));
        assertEquals("\"a\\\\b\\\"c\"", render("a\\b\"c"));
        assertEquals("\"it's\"", render("it's"));
        assertEquals(
                "\"line\\nbreak\\r\\ttab\\u0000\\u007f\\u0085\\u2028\\u2029\"",
                render("line\nbreak\r\ttab\0\u007f\u0085  "));
        assertEquals("'x'", render('x'));
        assertEquals("'\\''", render('\''));
        assertEquals("'\"'", render('"'));
        assertEquals("\"sb\"", render(new StringBuilder("sb")));
    }

    @Test
    void containersRenderStructurally() {
        assertEquals("new byte[] {1, 2}", render(new byte[] {1, 2}));
        assertEquals("new int[] {1, 2, 3}", render(new int[] {1, 2, 3}));
        assertEquals("new String[] {\"a\", null}", render(new String[] {"a", null}));
        assertEquals("new int[][] {new int[] {1}, new int[] {}}", render(new int[][] {{1}, {}}));
        assertEquals("List.of(1, \"x\")", render(List.of(1, "x")));
        assertEquals("List.of()", render(List.of()));
        assertEquals("Set.of(\"s\")", render(new LinkedHashSet<>(List.of("s"))));
        assertEquals("Set.of()", render(Set.of()));
        assertEquals(
                "UUID.fromString(\"f47ac10b-58cc-4372-a567-0e02b2c3d479\")",
                render(UUID.fromString("f47ac10b-58cc-4372-a567-0e02b2c3d479")));
        assertEquals("Map.of(1, 2)", render(Map.of(1, 2)));
        assertEquals("Map.of()", render(Map.of()));
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("a", List.of(1));
        m.put("b", 'c');
        assertEquals("Map.of(\"a\", List.of(1), \"b\", 'c')", render(m));
    }

    @Test
    void recordsRenderAsTheirConstructorCallWithTheirComponentsAsLiterals() {
        assertEquals("new Point(1, 2)", render(new Point(1, 2)));
        assertEquals("new Empty()", render(new Empty()));
        assertEquals("new Tuple2(\"a b\", \"\")", render(new Tuple2<>("a b", "")));
        assertEquals("new Named(\"x\\ny\", List.of(\"p, q\", \")\"))", render(new Named("x\ny", List.of("p, q", ")"))));
        assertEquals("new Tuple3('c', null, new Point(0, 0))", render(new Tuple3<>('c', null, new Point(0, 0))));
    }

    @Test
    void longRecordsBreakOneComponentPerLineAlignedInsideTheParenthesis() {
        String out = render(new Tuple2<>("a".repeat(50), "b".repeat(50)));
        assertEquals("new Tuple2(\"" + "a".repeat(50) + "\",\n           \"" + "b".repeat(50) + "\")", out);
    }

    @Test
    void aRecordWhoseComponentsCannotBeReadIsReflowedFromToString() {
        assertEquals("Broken[value=v]", render(new Broken("v")));
    }

    @Test
    void optionalsRenderAsTheirFactoryCalls() {
        assertEquals("Optional.of(\"a b\")", render(Optional.of("a b")));
        assertEquals("Optional.of(\"\\\"x\\\"\")", render(Optional.of("\"x\"")));
        assertEquals("Optional.empty()", render(Optional.empty()));
        assertEquals("OptionalInt.of(3)", render(OptionalInt.of(3)));
        assertEquals("OptionalInt.empty()", render(OptionalInt.empty()));
        assertEquals("OptionalLong.of(4)", render(OptionalLong.of(4)));
        assertEquals("OptionalLong.empty()", render(OptionalLong.empty()));
        assertEquals("OptionalDouble.of(0.5)", render(OptionalDouble.of(0.5)));
        assertEquals("OptionalDouble.empty()", render(OptionalDouble.empty()));
        assertEquals("List.of(Optional.of(new Point(1, 2)))", render(List.of(Optional.of(new Point(1, 2)))));
    }

    @Test
    void otherObjectsAreReflowedFromToString() {
        assertEquals("Thing(a b, c)", render(new Opaque("Thing(a b, c)")));
        assertEquals("first\nsecond", render(new Opaque("first\nsecond")));
    }

    @Test
    void writesToADeadRegionAreIgnored() {
        Libhegel lib = Engine.get();
        Printer printer = new Printer(lib, lib.printerNew(), new Printer.Regions());
        try {
            printer.text("x");
            Printer slot = printer.deferred();
            assertEquals("x", printer.read());
            assertFalse(slot.shouldPrint());
            slot.text("late");
            slot.hardBreak();
            slot.repr(List.of(new Point(1, 2), Optional.of("s")));
            slot.reflow("late");
            slot.speculate().commit();
            slot.speculate().abort();
            printer.text("late");
            assertEquals("x", printer.read());
        } finally {
            printer.free();
        }
    }

    @Test
    void invalidWritesAreRaised() {
        try (Document doc = new Document()) {
            Printer printer = doc.printer();
            IllegalArgumentException newline =
                    assertThrows(IllegalArgumentException.class, () -> printer.ifBreak("a\nb"));
            assertTrue(newline.getMessage().contains("hegel_printer_if_break"), newline.getMessage());
            assertThrows(IllegalArgumentException.class, () -> printer.comment("a\nb"));
            assertThrows(IllegalArgumentException.class, () -> printer.breakable("a\nb"));
            assertThrows(IllegalArgumentException.class, () -> printer.beginGroup(1, "a\nb"));
            Printer.Speculation speculation = printer.speculate();
            speculation.commit();
            assertThrows(IllegalArgumentException.class, speculation::commit);
            assertThrows(IllegalArgumentException.class, speculation::abort);
        }
    }

    @Test
    void otherEngineErrorsFromAWriteAreRaised() {
        Libhegel real = Engine.get();
        Libhegel failing = (Libhegel) Proxy.newProxyInstance(
                Libhegel.class.getClassLoader(), new Class<?>[] {Libhegel.class}, (proxy, method, args) -> {
                    if (method.getName().equals("printerBreakable")) {
                        return Abi.E_INTERNAL;
                    }
                    try {
                        return method.invoke(real, args);
                    } catch (InvocationTargetException e) {
                        throw e.getCause();
                    }
                });
        Printer printer = new Printer(failing, failing.printerNew(), new Printer.Regions());
        try {
            HegelException e = assertThrows(HegelException.class, () -> printer.repr(List.of(1, 2)));
            assertTrue(e.getMessage().contains("hegel_printer_breakable failed (rc=-8)"), e.getMessage());
        } finally {
            printer.free();
        }
    }

    @Test
    void longValuesBreakOneElementPerLine() {
        List<String> words = new ArrayList<>();
        for (int i = 0; i < 12; i++) {
            words.add("element" + i);
        }
        String out = render(words);
        String[] lines = out.split("\n");
        assertEquals(12, lines.length, out);
        assertEquals("List.of(\"element0\",", lines[0]);
        assertEquals("        \"element1\",", lines[1]);
        assertEquals("        \"element11\")", lines[11]);
        for (String line : lines) {
            assertTrue(line.length() <= 79, line);
        }
    }

    @Test
    void longMapsBreakOneEntryPerLine() {
        Map<String, String> entries = new LinkedHashMap<>();
        for (int i = 0; i < 12; i++) {
            entries.put("key" + i, "value" + i);
        }
        String out = render(entries);
        String[] lines = out.split("\n");
        assertEquals(12, lines.length, out);
        assertEquals("Map.of(\"key0\", \"value0\",", lines[0]);
        assertEquals("       \"key1\", \"value1\",", lines[1]);
        assertEquals("       \"key11\", \"value11\")", lines[11]);
    }

    @Test
    void readingAStandaloneDocumentTwiceGivesTheSameText() {
        Libhegel lib = Engine.get();
        Printer printer = new Printer(lib, lib.printerNew(), new Printer.Regions());
        try {
            printer.text("x");
            printer.hardBreak();
            assertEquals("x\n", printer.read());
            assertEquals("x\n", printer.read());
        } finally {
            printer.free();
        }
    }

    @Test
    void aNullDocumentCannotBeRead() {
        Printer printer = new Printer(Engine.get(), 0, new Printer.Regions());
        assertThrows(HegelException.class, printer::read);
    }

    @Test
    void textHonoursNewlinesAsHardBreaks() {
        assertEquals("a\n\nb\n", render(79, p -> p.text("a\n\nb\n")));
        assertEquals("[x,\n y]", render(79, p -> p.group(1, "[", "]", () -> p.text("x,\ny"))));
    }

    @Test
    void ifBreakPrintsOnlyWhenItsGroupBreaks() {
        Consumer<Printer> print = p -> p.group(1, "[", "]", () -> {
            p.text("1000,");
            p.breakable(" ");
            p.text("2000");
            p.ifBreak(",");
        });
        assertEquals("[1000, 2000]", render(79, print));
        assertEquals("[1000,\n 2000,]", render(8, print));
    }

    @Test
    void groupAndSeqLayOutTogether() {
        String out =
                render(12, p -> p.group(5, "Some(", ")", () -> p.seq("[", "]", List.of("first", "second"), p::text)));
        assertEquals("Some([first,\n      second])", out);
    }

    @Test
    void seqSeparatesItsItems() {
        assertEquals("{1,\n 2,\n 3}", render(8, p -> p.seq("{", "}", List.of(1, 2, 3), p::repr)));
        assertEquals("{1, 2, 3}", render(79, p -> p.seq("{", "}", List.of(1, 2, 3), p::repr)));
        assertEquals("{}", render(79, p -> p.seq("{", "}", List.of(), p::repr)));
    }

    @Test
    void aDocumentLaysItsGroupsOutToItsWidth() {
        Consumer<Printer> print = p -> {
            p.beginGroup(1, "[");
            p.text("first");
            p.separator();
            p.text("second");
            p.endGroup("]");
        };
        assertEquals("[first,\n second]", render(10, print));
        assertEquals("[first, second]", render(79, print));
    }

    @Test
    void theSupplierFormOfGroupReturnsItsBodysResult() {
        try (Document doc = new Document().maxWidth(4)) {
            Printer p = doc.printer();
            int result = p.group(1, "(", ")", () -> {
                p.text("ab,");
                p.breakable(" ");
                p.text("cd");
                return 7;
            });
            assertEquals(7, result);
            assertEquals("(ab,\n cd)", doc.finish());
        }
    }

    @Test
    void beginGroupWithoutAnIndentAlignsPastTheOpening() {
        assertEquals("é(a,\n  b)", render(3, p -> {
            p.beginGroup("é(");
            p.text("a");
            p.separator();
            p.text("b");
            p.endGroup(")");
        }));
    }

    @Test
    void shiftIndentMovesLaterBreakPoints() {
        assertEquals("[a,\n   b]", render(3, p -> {
            p.beginGroup("[");
            p.text("a,");
            p.shiftIndent(2);
            p.breakable(" ");
            p.shiftIndent(-2);
            p.text("b");
            p.endGroup("]");
        }));
    }

    @Test
    void aCommentEndsItsLineAndBreaksItsGroups() {
        assertEquals("[1,  // one\n 2\n]", render(79, p -> {
            p.beginGroup("[");
            p.text("1,");
            p.comment("one");
            p.breakable(" ");
            p.text("2");
            p.endGroup("]");
        }));
    }

    @Test
    void aSpeculationKeepsOnlyWhatIsCommitted() {
        assertEquals("kept", render(79, p -> {
            Printer.Speculation dropped = p.speculate();
            p.text("dropped");
            dropped.abort();
            Printer.Speculation kept = p.speculate();
            p.text("kept");
            kept.commit();
            kept.close();
            try (Printer.Speculation closed = p.speculate()) {
                p.text("closed");
            }
        }));
    }

    @Test
    void aSpeculationIsAbortedWhenItsBlockThrows() {
        try (Document doc = new Document()) {
            Printer p = doc.printer();
            assertThrows(IllegalStateException.class, () -> {
                try (Printer.Speculation speculation = p.speculate()) {
                    p.text("partial");
                    throw new IllegalStateException("rejected");
                }
            });
            p.text("after");
            assertEquals("after", doc.finish());
        }
    }

    @Test
    void aDeferredRegionIsFilledInWhereItWasOpened() {
        try (Document doc = new Document()) {
            Printer p = doc.printer();
            p.text("[");
            Printer slot = p.deferred();
            p.text("]");
            Printer inner = slot.deferred();
            slot.text(" second");
            inner.text("first");
            assertTrue(slot.shouldPrint());
            assertEquals("[first second]", doc.finish());
            assertFalse(slot.shouldPrint());
            slot.text("late");
        }
    }

    @Test
    void aDeferredRegionOpenedInAnAbortedSpeculationIsDead() {
        try (Document doc = new Document()) {
            Printer p = doc.printer();
            Printer.Speculation speculation = p.speculate();
            Printer slot = p.deferred();
            speculation.abort();
            assertFalse(slot.shouldPrint());
            slot.text("ignored");
            slot.repr(List.of(1));
            Printer orphan = slot.deferred();
            assertFalse(orphan.shouldPrint());
            orphan.text("ignored");
            p.text("kept");
            assertEquals("kept", doc.finish());
        }
    }

    @Test
    void theNoopPrinterIgnoresEverything() {
        Printer p = Printer.noop();
        assertFalse(p.shouldPrint());
        p.text("a\nb");
        p.repr(List.of(1));
        p.reflow("x");
        p.breakable(" ");
        p.ifBreak(",");
        p.hardBreak();
        p.beginGroup("[");
        p.beginGroup(1, "[");
        p.endGroup("]");
        p.endGroup("]");
        assertEquals(3, p.group(1, "(", ")", () -> 3));
        p.seq("[", "]", List.of(1, 2), p::repr);
        p.shiftIndent(1);
        p.comment("c");
        p.speculate().commit();
        p.speculate().abort();
        assertSame(Printer.noop(), p.deferred());
    }

    @Test
    void aPrinterOfAClosedDocumentIgnoresEverything() {
        Printer p;
        try (Document doc = new Document()) {
            p = doc.printer();
            p.text("x");
        }
        assertFalse(p.shouldPrint());
        p.text("late");
        p.repr("late");
        assertSame(Printer.noop(), p.deferred());
    }

    @Test
    void aDocumentChoosesItsWidthBeforeItIsPrintedTo() {
        assertThrows(IllegalArgumentException.class, () -> new Document().maxWidth(0));
        try (Document doc = new Document()) {
            doc.printer();
            assertThrows(IllegalStateException.class, () -> doc.maxWidth(10));
        }
    }

    @Test
    void aFinishedDocumentCannotBePrintedToAgain() {
        Document doc = new Document();
        assertEquals("", doc.finish());
        assertThrows(IllegalStateException.class, doc::printer);
        doc.close();
        Document unused = new Document();
        unused.close();
        assertThrows(IllegalStateException.class, unused::finish);
    }
}
