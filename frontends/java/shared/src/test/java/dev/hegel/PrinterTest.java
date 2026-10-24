package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
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
import org.junit.jupiter.api.Test;

class PrinterTest {
    private static String render(Object value) {
        Libhegel lib = Engine.get();
        try (Printer printer = new Printer(lib, lib.printerNew())) {
            printer.value(value);
            return printer.read();
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
        assertEquals("[1, 2]", render(new byte[] {1, 2}));
        assertEquals("[1, 2, 3]", render(new int[] {1, 2, 3}));
        assertEquals("[\"a\", null]", render(new String[] {"a", null}));
        assertEquals("[1, \"x\"]", render(List.of(1, "x")));
        assertEquals("[]", render(List.of()));
        assertEquals("[\"s\"]", render(new LinkedHashSet<>(List.of("s"))));
        assertEquals("{1: 2}", render(Map.of(1, 2)));
        assertEquals("{}", render(Map.of()));
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("a", List.of(1));
        m.put("b", 'c');
        assertEquals("{\"a\": [1], \"b\": 'c'}", render(m));
    }

    @Test
    void recordsRenderStructurallyWithTheirComponentsAsLiterals() {
        assertEquals("Point[x=1, y=2]", render(new Point(1, 2)));
        assertEquals("Empty[]", render(new Empty()));
        assertEquals("Tuple2[value1=\"a b\", value2=\"\"]", render(new Tuple2<>("a b", "")));
        assertEquals("Named[name=\"x\\ny\", tags=[\"p, q\", \"]\"]]", render(new Named("x\ny", List.of("p, q", "]"))));
        assertEquals(
                "Tuple3[value1='c', value2=null, value3=Point[x=0, y=0]]",
                render(new Tuple3<>('c', null, new Point(0, 0))));
    }

    @Test
    void longRecordsBreakOneComponentPerLineAlignedPastTheirName() {
        String out = render(new Tuple2<>("a".repeat(50), "b".repeat(50)));
        assertEquals("Tuple2[value1=\"" + "a".repeat(50) + "\",\n       value2=\"" + "b".repeat(50) + "\"]", out);
    }

    @Test
    void aRecordWhoseComponentsCannotBeReadIsReflowedFromToString() {
        assertEquals("Broken[value=v]", render(new Broken("v")));
    }

    @Test
    void optionalsRenderStructurally() {
        assertEquals("Optional[\"a b\"]", render(Optional.of("a b")));
        assertEquals("Optional[\"\\\"x\\\"\"]", render(Optional.of("\"x\"")));
        assertEquals("Optional.empty", render(Optional.empty()));
        assertEquals("OptionalInt[3]", render(OptionalInt.of(3)));
        assertEquals("OptionalInt.empty", render(OptionalInt.empty()));
        assertEquals("OptionalLong[4]", render(OptionalLong.of(4)));
        assertEquals("OptionalLong.empty", render(OptionalLong.empty()));
        assertEquals("OptionalDouble[0.5]", render(OptionalDouble.of(0.5)));
        assertEquals("OptionalDouble.empty", render(OptionalDouble.empty()));
        assertEquals("[Optional[Point[x=1, y=2]]]", render(List.of(Optional.of(new Point(1, 2)))));
    }

    @Test
    void otherObjectsAreReflowedFromToString() {
        assertEquals("Thing(a b, c)", render(new Opaque("Thing(a b, c)")));
        assertEquals("first\nsecond", render(new Opaque("first\nsecond")));
    }

    @Test
    void writesToADeadRegionAreIgnored() {
        Libhegel lib = Engine.get();
        try (Printer printer = new Printer(lib, lib.printerNew())) {
            printer.text("x");
            assertEquals("x", printer.read());
            printer.text("late");
            printer.hardBreak();
            printer.value(List.of(new Point(1, 2), Optional.of("s")));
            printer.beginSpeculative();
            printer.commitSpeculative();
            printer.abortSpeculative();
            assertEquals("x", printer.read());
        }
    }

    @Test
    void invalidWritesAreRaised() {
        Libhegel lib = Engine.get();
        try (Printer printer = new Printer(lib, lib.printerNew())) {
            IllegalArgumentException newline = assertThrows(IllegalArgumentException.class, () -> printer.text("a\nb"));
            assertTrue(newline.getMessage().contains("hegel_printer_text"), newline.getMessage());
            assertThrows(IllegalArgumentException.class, printer::commitSpeculative);
            assertThrows(IllegalArgumentException.class, printer::abortSpeculative);
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
        try (Printer printer = new Printer(failing, failing.printerNew())) {
            HegelException e = assertThrows(HegelException.class, () -> printer.value(List.of(1, 2)));
            assertTrue(e.getMessage().contains("hegel_printer_breakable failed (rc=-8)"), e.getMessage());
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
        assertEquals("[\"element0\",", lines[0]);
        assertEquals(" \"element1\",", lines[1]);
        assertEquals(" \"element11\"]", lines[11]);
        for (String line : lines) {
            assertTrue(line.length() <= 79, line);
        }
    }

    @Test
    void readingAStandaloneDocumentTwiceGivesTheSameText() {
        Libhegel lib = Engine.get();
        try (Printer printer = new Printer(lib, lib.printerNew())) {
            printer.text("x");
            printer.hardBreak();
            assertEquals("x\n", printer.read());
            assertEquals("x\n", printer.read());
        }
    }

    @Test
    void speculativeOutputCanBeAbandoned() {
        Libhegel lib = Engine.get();
        try (Printer printer = new Printer(lib, lib.printerNew())) {
            printer.beginSpeculative();
            printer.text("dropped");
            printer.abortSpeculative();
            printer.beginSpeculative();
            printer.text("kept");
            printer.commitSpeculative();
            assertEquals("kept", printer.read());
        }
    }

    @Test
    void aNullDocumentCannotBeRead() {
        Libhegel lib = Engine.get();
        try (Printer printer = new Printer(lib, 0)) {
            assertThrows(HegelException.class, printer::read);
        }
    }
}
