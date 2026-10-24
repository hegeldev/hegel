package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Libhegel;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
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
    void otherObjectsAreReflowedFromToString() {
        assertEquals("Point[x=1, y=2]", render(new Point(1, 2)));
        assertEquals("Optional[\"x\"]", render(Optional.of("\"x\"")));
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
