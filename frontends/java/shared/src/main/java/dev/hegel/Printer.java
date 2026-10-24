package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.reflect.Array;
import java.lang.reflect.Method;
import java.lang.reflect.RecordComponent;
import java.util.Arrays;
import java.util.Collection;
import java.util.Iterator;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalDouble;
import java.util.OptionalInt;
import java.util.OptionalLong;

/**
 * A handle onto a region of an engine pretty-printer document ({@code hegel_printer_t}).
 *
 * <p>The engine lays the document out; this class renders Java values into it in Java syntax,
 * giving the engine their structure: strings and characters are quoted and escaped, arrays and
 * collections print as {@code [a, b]}, maps as {@code {k: v}}, records as {@code Name[a=x, b=y]}
 * (as their default {@code toString()} does) and optionals as {@code Optional[x]} or {@code
 * Optional.empty}, each a group that breaks one element per line when it does not fit, with every
 * element rendered the same way. Anything else is its {@code toString()} handed to the engine's
 * reflow, which recovers the structure from its brackets; so is a record whose components cannot
 * be read, such as one whose accessor throws.
 *
 * <p>A write to a region that is already dead (the document was read while a straggling thread
 * was still printing) is a harmless no-op. Any other error the engine reports for a write is
 * raised: an {@link IllegalArgumentException} for an invalid argument, such as text containing a
 * newline, and a {@link HegelException} otherwise.
 */
final class Printer implements AutoCloseable {
    private final Libhegel lib;
    private final long handle;

    Printer(Libhegel lib, long handle) {
        this.lib = lib;
        this.handle = handle;
    }

    void text(String text) {
        check(lib.printerText(handle, text), "text");
    }

    void hardBreak() {
        check(lib.printerHardBreak(handle), "hard_break");
    }

    void beginSpeculative() {
        check(lib.printerBeginSpeculative(handle), "begin_speculative");
    }

    void commitSpeculative() {
        check(lib.printerCommitSpeculative(handle), "commit_speculative");
    }

    void abortSpeculative() {
        check(lib.printerAbortSpeculative(handle), "abort_speculative");
    }

    private void beginGroup(String open) {
        check(lib.printerBeginGroup(handle, open.length(), open), "begin_group");
    }

    private void endGroup(String close) {
        check(lib.printerEndGroup(handle, close), "end_group");
    }

    private void check(int rc, String op) {
        if (rc == Abi.OK || rc == Abi.E_INVALID_HANDLE) {
            return;
        }
        String message = "hegel_printer_" + op + " failed (rc=" + rc + "): " + lib.lastErrorMessage();
        if (rc == Abi.E_INVALID_ARG) {
            throw new IllegalArgumentException(message);
        }
        throw new HegelException(message);
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
            beginGroup("{");
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
            endGroup("}");
        } else if (value instanceof Record record) {
            record(record);
        } else if (value instanceof Optional<?> optional) {
            optional("Optional", optional.isPresent(), optional.orElse(null));
        } else if (value instanceof OptionalInt optional) {
            optional("OptionalInt", optional.isPresent(), optional.isPresent() ? optional.getAsInt() : null);
        } else if (value instanceof OptionalLong optional) {
            optional("OptionalLong", optional.isPresent(), optional.isPresent() ? optional.getAsLong() : null);
        } else if (value instanceof OptionalDouble optional) {
            optional("OptionalDouble", optional.isPresent(), optional.isPresent() ? optional.getAsDouble() : null);
        } else {
            reflow(String.valueOf(value));
        }
    }

    private void reflow(String text) {
        check(lib.printerReflow(handle, text), "reflow");
    }

    private void record(Record record) {
        RecordComponent[] components = record.getClass().getRecordComponents();
        Object[] values = new Object[components.length];
        try {
            for (int i = 0; i < components.length; i++) {
                Method accessor = components[i].getAccessor();
                accessor.setAccessible(true);
                values[i] = accessor.invoke(record);
            }
        } catch (ReflectiveOperationException | RuntimeException e) {
            reflow(String.valueOf(record));
            return;
        }
        beginGroup(record.getClass().getSimpleName() + "[");
        for (int i = 0; i < components.length; i++) {
            if (i > 0) {
                separator();
            }
            text(components[i].getName() + "=");
            value(values[i]);
        }
        endGroup("]");
    }

    private void optional(String name, boolean present, Object contents) {
        if (present) {
            beginGroup(name + "[");
            value(contents);
            endGroup("]");
        } else {
            text(name + ".empty");
        }
    }

    private void sequence(Iterator<?> elements) {
        beginGroup("[");
        boolean first = true;
        while (elements.hasNext()) {
            if (!first) {
                separator();
            }
            first = false;
            value(elements.next());
        }
        endGroup("]");
    }

    private void separator() {
        text(",");
        check(lib.printerBreakable(handle, " "), "breakable");
    }

    /**
     * Read the whole document, with every clone's region spliced in at its anchor. Only meaningful
     * on a document's root handle (a test case's root printer or a standalone one); reading seals
     * the document. Resolving reports an invalid argument both for a document with no clone regions
     * to resolve, which is not an error, and for misuse that reading the value reports again, so
     * only its other errors are raised here.
     */
    String read() {
        int resolved = lib.printerResolve(handle);
        if (resolved != Abi.OK && resolved != Abi.E_INVALID_ARG) {
            throw new HegelException("hegel_printer_resolve failed (rc=" + resolved + "): " + lib.lastErrorMessage());
        }
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
