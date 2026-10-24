package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.reflect.Array;
import java.util.Arrays;
import java.util.Collection;
import java.util.Iterator;
import java.util.Locale;
import java.util.Map;

/**
 * A handle onto a region of an engine pretty-printer document ({@code hegel_printer_t}).
 *
 * <p>The engine lays the document out; this class renders Java values into it in Java syntax,
 * giving the engine their structure: strings and characters are quoted and escaped, arrays and
 * collections print as {@code [a, b]} and maps as {@code {k: v}}, each a group that breaks one
 * element per line when it does not fit, and anything else is its {@code toString()} handed to the
 * engine's reflow, which recovers the structure from its brackets.
 *
 * <p>A write to a region that is already dead (the document was read while a straggling thread
 * was still printing) is a harmless no-op, so writes ignore the engine's return code.
 */
final class Printer implements AutoCloseable {
    private final Libhegel lib;
    private final long handle;

    Printer(Libhegel lib, long handle) {
        this.lib = lib;
        this.handle = handle;
    }

    void text(String text) {
        lib.printerText(handle, text);
    }

    void hardBreak() {
        lib.printerHardBreak(handle);
    }

    void beginSpeculative() {
        lib.printerBeginSpeculative(handle);
    }

    void commitSpeculative() {
        lib.printerCommitSpeculative(handle);
    }

    void abortSpeculative() {
        lib.printerAbortSpeculative(handle);
    }

    /** Render {@code value} in Java syntax at the current position. */
    void value(Object value) {
        if (value == null) {
            text("null");
        } else if (value instanceof CharSequence s) {
            text(quote(s.toString(), '"'));
        } else if (value instanceof Character c) {
            text(quote(c.toString(), '\''));
        } else if (value.getClass().isArray()) {
            int length = Array.getLength(value);
            Object[] elements = new Object[length];
            for (int i = 0; i < length; i++) {
                elements[i] = Array.get(value, i);
            }
            sequence(Arrays.asList(elements).iterator());
        } else if (value instanceof Collection<?> collection) {
            sequence(collection.iterator());
        } else if (value instanceof Map<?, ?> map) {
            lib.printerBeginGroup(handle, 1, "{");
            boolean first = true;
            for (Map.Entry<?, ?> entry : map.entrySet()) {
                if (!first) {
                    separator();
                }
                first = false;
                value(entry.getKey());
                text(": ");
                value(entry.getValue());
            }
            lib.printerEndGroup(handle, "}");
        } else {
            lib.printerReflow(handle, String.valueOf(value));
        }
    }

    private void sequence(Iterator<?> elements) {
        lib.printerBeginGroup(handle, 1, "[");
        boolean first = true;
        while (elements.hasNext()) {
            if (!first) {
                separator();
            }
            first = false;
            value(elements.next());
        }
        lib.printerEndGroup(handle, "]");
    }

    private void separator() {
        text(",");
        lib.printerBreakable(handle, " ");
    }

    /**
     * Read the whole document, with every clone's region spliced in at its anchor. Only meaningful
     * on a document's root handle (a test case's root printer or a standalone one); reading seals
     * the document. A document with no clone regions has nothing to resolve, which is not an error.
     */
    String read() {
        lib.printerResolve(handle);
        String[] out = new String[1];
        int rc = lib.printerValue(handle, out);
        if (rc != Abi.OK) {
            throw new HegelException("hegel_printer_value failed (rc=" + rc + "): " + lib.lastErrorMessage());
        }
        return out[0];
    }

    @Override
    public void close() {
        lib.printerFree(handle);
    }

    /**
     * {@code s} as a Java literal between {@code delimiter}s: backslashes, the delimiter and
     * control characters escaped, so the result is one line.
     */
    static String quote(String s, char delimiter) {
        StringBuilder sb = new StringBuilder(s.length() + 2).append(delimiter);
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '\\' -> sb.append("\\\\");
                case '\n' -> sb.append("\\n");
                case '\r' -> sb.append("\\r");
                case '\t' -> sb.append("\\t");
                default -> {
                    if (c == delimiter) {
                        sb.append('\\').append(c);
                    } else if (Character.isISOControl(c) || c == '\u2028' || c == '\u2029') {
                        sb.append(String.format(Locale.ROOT, "\\u%04x", (int) c));
                    } else {
                        sb.append(c);
                    }
                }
            }
        }
        return sb.append(delimiter).toString();
    }
}
