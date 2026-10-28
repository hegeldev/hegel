package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.reflect.Array;
import java.lang.reflect.Method;
import java.lang.reflect.RecordComponent;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalDouble;
import java.util.OptionalInt;
import java.util.OptionalLong;
import java.util.Set;
import java.util.UUID;
import java.util.function.Consumer;
import java.util.function.Supplier;

/**
 * The write surface of a pretty-printed document: how a {@link PrintableGenerator} describes the
 * values it draws, and how a {@link Generator#printWith printWith} callback prints a custom
 * representation. See the {@linkplain dev.hegel package documentation} for the printing model.
 *
 * <p>The engine lays the document out, so a printer never measures or wraps anything itself. Output
 * is built from three primitives: {@link #text} emits literal text, {@link #breakable} marks a
 * point that renders as a separator when the enclosing group fits on one line and as a newline plus
 * indentation when it does not, and {@link #beginGroup} / {@link #endGroup} delimit the groups those
 * decisions are made over. A group either fits, and every break point in it renders as its
 * separator, or breaks as a whole, outermost groups first. On top of those sit {@link #repr}, which
 * prints any Java value in Java syntax with its structure recovered, and the {@link #group}, {@link
 * #seq} and {@link #separator} conveniences.
 *
 * <p>Because printing happens while a value is drawn, a printer is more than an append-only stream:
 * a generator that may reject an attempt prints it inside a {@link #speculate() speculation} and
 * commits only an accepted one, and {@link #deferred()} opens a hole that can be filled in after the
 * draw, from any thread that owns the returned printer.
 *
 * <p>A printer obtained from a test case or a {@link Document} is used by one thread at a time.
 * Writing to a region that is already dead (the document was read, or a speculation the region was
 * anchored inside was aborted) is a harmless no-op, as is everything on the {@link #noop()}
 * printer. Any other error the engine reports for a write is raised: an {@link
 * IllegalArgumentException} for an invalid argument, such as an {@link #endGroup} with no group
 * open, and a {@link HegelException} otherwise.
 */
public final class Printer {
    private static final Printer NOOP = new Printer(null, 0, null);

    private final Libhegel lib;
    private final long handle;
    private final Regions regions;
    private boolean freed;

    /**
     * The deferred regions opened from a document's printers, freed together once the document has
     * been read (by the test case or {@link Document} that owns them).
     */
    static final class Regions {
        private final List<Printer> open = new ArrayList<>();

        synchronized void add(Printer printer) {
            open.add(printer);
        }

        void freeAll() {
            List<Printer> taken;
            synchronized (this) {
                taken = new ArrayList<>(open);
                open.clear();
            }
            for (Printer printer : taken) {
                printer.free();
            }
        }
    }

    Printer(Libhegel lib, long handle, Regions regions) {
        this.lib = lib;
        this.handle = handle;
        this.regions = regions;
    }

    /**
     * A printer that discards everything printed to it. A {@link PrintableGenerator} draws silently
     * by drawing and printing into it.
     *
     * @return the discarding printer
     */
    public static Printer noop() {
        return NOOP;
    }

    private boolean inactive() {
        return handle == 0 || freed;
    }

    /**
     * Whether printing here produces output: {@code false} for the {@link #noop()} printer and for
     * a region that has died, whose writes are discarded. Use it to skip work, such as formatting a
     * value, whose only purpose is to be printed.
     *
     * @return whether writes to this printer are kept
     */
    public synchronized boolean shouldPrint() {
        if (inactive()) {
            return false;
        }
        boolean[] live = new boolean[1];
        check(lib, lib.printerIsLive(handle, live), "is_live");
        return live[0];
    }

    /**
     * Emit literal, unbreakable text. Each newline in {@code text} is a {@link #hardBreak()}.
     *
     * @param text the text
     */
    public synchronized void text(String text) {
        if (inactive()) {
            return;
        }
        String[] lines = text.split("\n", -1);
        for (int i = 0; i < lines.length; i++) {
            if (i > 0) {
                check(lib, lib.printerHardBreak(handle), "hard_break");
            }
            if (!lines[i].isEmpty()) {
                check(lib, lib.printerText(handle, lines[i]), "text");
            }
        }
    }

    /**
     * Print {@code value} as a Java expression: strings and characters as escaped literals,
     * records as a call of their canonical constructor, {@code new Name(x, y)}, optionals as
     * {@code Optional.of(x)} or {@code Optional.empty()}, arrays as {@code new int[] {a, b}}
     * (named for their component type), sets as {@code Set.of(a, b)}, other collections as {@code
     * List.of(a, b)}, maps as {@code Map.of(k, v)} and UUIDs as {@code UUID.fromString("...")}.
     * Each is a group that breaks one element (one entry, for a map) per line when it does not fit,
     * with every element printed the same way. Anything else is its {@code toString()}, laid out by
     * {@link #reflow}; so is a record whose components cannot be read, such as one whose accessor
     * throws.
     * Nothing is formatted when {@link #shouldPrint()} is false.
     *
     * <p>This is how the value-producing generators ({@code map}, {@code just}, {@code
     * sampledFrom}, {@code composite}) and the leaf generators print what they draw, and what {@link
     * Generator#printAsRepr()} prints.
     *
     * @param value the value to print
     */
    public synchronized void repr(Object value) {
        if (shouldPrint()) {
            render(value);
        }
    }

    /**
     * Lay out an already-formatted flat representation, such as a {@code toString()}, through
     * groups and break points recovered from its brackets: {@code Name(a, b)}, {@code [a, b]}, {@code
     * {k: v}}, with quoted literals kept whole. Text that does not follow that grammar is emitted
     * verbatim, with each newline a hard break.
     *
     * @param representation the representation
     */
    public synchronized void reflow(String representation) {
        if (!inactive()) {
            check(lib, lib.printerReflow(handle, representation), "reflow");
        }
    }

    /**
     * Emit a potential break point: {@code separator} when the enclosing group fits on the current
     * line, and a newline plus the current indentation when it breaks.
     *
     * @param separator the flat form, which must not contain a newline
     */
    public synchronized void breakable(String separator) {
        if (!inactive()) {
            check(lib, lib.printerBreakable(handle, separator), "breakable");
        }
    }

    /**
     * Emit {@code text} only if the innermost open group breaks across lines, such as a trailing
     * comma only the multi-line form needs. It never counts toward the line width.
     *
     * @param text the text, which must not contain a newline
     */
    public synchronized void ifBreak(String text) {
        if (!inactive()) {
            check(lib, lib.printerIfBreak(handle, text), "if_break");
        }
    }

    /** Emit an unconditional newline followed by the current indentation. */
    public synchronized void hardBreak() {
        if (!inactive()) {
            check(lib, lib.printerHardBreak(handle), "hard_break");
        }
    }

    /**
     * Open a group: emit {@code open}, then indent later break points by {@code indent} more
     * columns (conventionally the width of {@code open}, so continuation lines align just inside
     * it). Close it with {@link #endGroup}.
     *
     * @param indent the extra indentation of the group's break points
     * @param open the opening text, which must not contain a newline
     */
    public synchronized void beginGroup(int indent, String open) {
        if (!inactive()) {
            check(lib, lib.printerBeginGroup(handle, indent, open), "begin_group");
        }
    }

    /**
     * Open a group indented by the width of {@code open}, so that continuation lines align just
     * inside it.
     *
     * @param open the opening text, which must not contain a newline
     */
    public void beginGroup(String open) {
        beginGroup(open.codePointCount(0, open.length()), open);
    }

    /**
     * Close the innermost group: undo the indentation its {@link #beginGroup} added, then emit
     * {@code close}.
     *
     * @param close the closing text, which must not contain a newline
     */
    public synchronized void endGroup(String close) {
        if (!inactive()) {
            check(lib, lib.printerEndGroup(handle, close), "end_group");
        }
    }

    /**
     * Print a group: {@code open}, whatever {@code body} prints with later break points indented by
     * {@code indent}, then {@code close}. The block form of {@link #beginGroup} / {@link #endGroup},
     * which cannot be left unbalanced by a body that returns.
     *
     * @param indent the extra indentation of the group's break points
     * @param open the opening text
     * @param close the closing text
     * @param body prints the group's contents
     */
    public void group(int indent, String open, String close, Runnable body) {
        beginGroup(indent, open);
        body.run();
        endGroup(close);
    }

    /**
     * Print a group around {@code body}, as {@link #group(int, String, String, Runnable)} does, and
     * return what {@code body} returns, such as a value drawn while printing it.
     *
     * @param indent the extra indentation of the group's break points
     * @param open the opening text
     * @param close the closing text
     * @param body prints the group's contents and returns a result
     * @param <R> the result type
     * @return the body's result
     */
    public <R> R group(int indent, String open, String close, Supplier<R> body) {
        beginGroup(indent, open);
        R result = body.get();
        endGroup(close);
        return result;
    }

    /**
     * Print {@code items} as a delimited, comma-separated sequence: {@code open}, each item as
     * {@code print} prints it with a {@link #separator()} between consecutive items, then {@code
     * close}; inline when it fits, and one item per line aligned just inside {@code open} when it
     * does not.
     *
     * @param open the opening text
     * @param close the closing text
     * @param items the items
     * @param print prints one item to this printer
     * @param <E> the item type
     */
    public <E> void seq(String open, String close, Iterable<E> items, Consumer<? super E> print) {
        beginGroup(open);
        boolean first = true;
        for (E item : items) {
            if (!first) {
                separator();
            }
            first = false;
            print.accept(item);
        }
        endGroup(close);
    }

    /**
     * Emit the separator between two items of a group: a {@code ,} followed by a break point that
     * renders as a space when the group fits on one line.
     */
    public void separator() {
        text(",");
        breakable(" ");
    }

    /**
     * Adjust the indentation applied by later break points by {@code delta}, which may be negative
     * to undo an earlier shift.
     *
     * @param delta the change in indentation
     */
    public synchronized void shiftIndent(int delta) {
        if (!inactive()) {
            check(lib, lib.printerShiftIndent(handle, delta), "shift_indent");
        }
    }

    /**
     * Attach a comment to the line being written: {@code text} is rendered as {@code // text} at
     * the end of that line, every group open here is forced to break (nothing else may share a
     * line with a comment), and the comment does not count toward the line width.
     *
     * @param text the comment, which must not contain a newline
     */
    public synchronized void comment(String text) {
        if (!inactive()) {
            check(lib, lib.printerComment(handle, "  // " + text), "comment");
        }
    }

    /**
     * Open a speculative region: what is printed from here on is held back until {@link
     * Speculation#commit()} keeps it or {@link Speculation#abort()} discards it. Closing the
     * speculation without committing it, as a {@code try}-with-resources block does when its body
     * throws, discards it. This is how printing survives a rejected draw: print each attempt
     * speculatively and commit only the accepted one.
     *
     * @return the open speculation
     */
    public synchronized Speculation speculate() {
        if (!inactive()) {
            check(lib, lib.printerBeginSpeculative(handle), "begin_speculative");
        }
        return new Speculation(this);
    }

    private synchronized void endSpeculation(boolean keep) {
        if (inactive()) {
            return;
        }
        if (keep) {
            check(lib, lib.printerCommitSpeculative(handle), "commit_speculative");
        } else {
            check(lib, lib.printerAbortSpeculative(handle), "abort_speculative");
        }
    }

    /**
     * Open a deferred region: a hole in the document at the current position, written through the
     * returned printer. Whatever is printed to it, at any later point and from whichever single
     * thread owns it, appears at the hole when the document is laid out, broken into lines as if it
     * had been printed inline. This is how a generator whose value can only be described once it
     * is used prints: it opens a region while drawing and writes to it as the value is used.
     *
     * <p>The region dies when the document is read or when a speculation it was opened inside is
     * aborted, and its writes are then discarded. The deferred region of the {@link #noop()}
     * printer, or of a dead region, discards everything.
     *
     * @return a printer onto the new region
     */
    public synchronized Printer deferred() {
        if (inactive()) {
            return NOOP;
        }
        long[] child = new long[1];
        int rc = lib.printerDeferred(handle, child);
        check(lib, rc, "deferred");
        if (rc != Abi.OK) {
            return NOOP;
        }
        Printer printer = new Printer(lib, child[0], regions);
        regions.add(printer);
        return printer;
    }

    /**
     * Read the whole document, with every deferred region spliced in at its anchor. Only meaningful
     * on a document's root handle (a test case's root printer or a standalone one); reading seals
     * the document. Resolving reports an invalid argument both for a document with no deferred
     * regions to resolve, which is not an error, and for misuse that reading the value reports
     * again, so only its other errors are raised here.
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

    /** Release the handle; later writes through this printer are discarded. */
    synchronized void free() {
        freed = true;
        lib.printerFree(handle);
    }

    static void check(Libhegel lib, int rc, String op) {
        if (rc == Abi.OK || rc == Abi.E_INVALID_HANDLE) {
            return;
        }
        String message = "hegel_printer_" + op + " failed (rc=" + rc + "): " + lib.lastErrorMessage();
        if (rc == Abi.E_INVALID_ARG) {
            throw new InvalidArgument(message);
        }
        throw new HegelException(message);
    }

    private void render(Object value) {
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
            String component = value.getClass().getComponentType().getSimpleName();
            sequence("new " + component + "[] {", Arrays.asList(elements).iterator(), "}");
        } else if (value instanceof Set<?> set) {
            sequence("Set.of(", set.iterator(), ")");
        } else if (value instanceof Collection<?> collection) {
            sequence("List.of(", collection.iterator(), ")");
        } else if (value instanceof UUID uuid) {
            text("UUID.fromString(" + quote(uuid.toString(), '"') + ")");
        } else if (value instanceof Map<?, ?> map) {
            beginGroup("Map.of(");
            boolean first = true;
            for (Map.Entry<?, ?> entry : map.entrySet()) {
                if (!first) {
                    separator();
                }
                first = false;
                render(entry.getKey());
                text(", ");
                render(entry.getValue());
            }
            endGroup(")");
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
        beginGroup("new " + record.getClass().getSimpleName() + "(");
        for (int i = 0; i < components.length; i++) {
            if (i > 0) {
                separator();
            }
            render(values[i]);
        }
        endGroup(")");
    }

    private void optional(String name, boolean present, Object contents) {
        if (present) {
            beginGroup(name + ".of(");
            render(contents);
            endGroup(")");
        } else {
            text(name + ".empty()");
        }
    }

    private void sequence(String open, Iterator<?> elements, String close) {
        beginGroup(open);
        boolean first = true;
        while (elements.hasNext()) {
            if (!first) {
                separator();
            }
            first = false;
            render(elements.next());
        }
        endGroup(close);
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
                    } else if (Character.isISOControl(c) || c == ' ' || c == ' ') {
                        sb.append(String.format(Locale.ROOT, "\\u%04x", (int) c));
                    } else {
                        sb.append(c);
                    }
                }
            }
        }
        return sb.append(delimiter).toString();
    }

    /**
     * An open speculative region of a {@link Printer}; see {@link Printer#speculate()}. Close it
     * exactly once, with {@link #commit()} or {@link #abort()}; {@link #close()} aborts it unless
     * it was already closed.
     */
    public static final class Speculation implements AutoCloseable {
        private final Printer printer;
        private boolean open = true;

        private Speculation(Printer printer) {
            this.printer = printer;
        }

        /** Close the region, keeping what was printed in it. */
        public void commit() {
            open = false;
            printer.endSpeculation(true);
        }

        /** Close the region, discarding what was printed in it. */
        public void abort() {
            open = false;
            printer.endSpeculation(false);
        }

        /** Abort the region unless it was already committed or aborted. */
        @Override
        public void close() {
            if (open) {
                abort();
            }
        }
    }
}
