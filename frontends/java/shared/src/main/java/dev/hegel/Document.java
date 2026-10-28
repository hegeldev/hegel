package dev.hegel;

import dev.hegel.lowlevel.Libhegel;

/**
 * One standalone pretty-printed document, laid out by the engine: for building and testing printing
 * code outside a test run. Choose the layout with {@link #maxWidth} before printing, write through
 * {@link #printer()}, and render with {@link #finish()}, which also releases the document.
 *
 * <pre>{@code
 * try (Document doc = new Document().maxWidth(8)) {
 *     Printer p = doc.printer();
 *     p.seq("{", "}", List.of(1, 2, 3), p::repr);
 *     assert doc.finish().equals("{1,\n 2,\n 3}");
 * }
 * }</pre>
 *
 * <p>A test run has its own document per test case; printing code is handed a printer onto it and
 * never renders it.
 */
public final class Document implements AutoCloseable {
    /** The line width a document is laid out to unless {@link #maxWidth} says otherwise. */
    public static final int DEFAULT_MAX_WIDTH = 79;

    private final Printer.Regions regions = new Printer.Regions();
    private int maxWidth = DEFAULT_MAX_WIDTH;
    private Printer printer;
    private boolean closed;

    /** An empty document laid out to {@value #DEFAULT_MAX_WIDTH} columns. */
    public Document() {}

    /**
     * Keep lines within {@code maxWidth} columns where the group structure allows it. Layout
     * options describe the whole document, so this must be called before {@link #printer()}.
     *
     * @param maxWidth the line width, at least 1
     * @return this document
     * @throws IllegalArgumentException if {@code maxWidth} is less than 1
     * @throws IllegalStateException if the document has already been printed to
     */
    public Document maxWidth(int maxWidth) {
        if (printer != null) {
            throw new IllegalStateException("maxWidth must be set before the document is printed to");
        }
        if (maxWidth < 1) {
            throw new IllegalArgumentException("maxWidth must be positive, got " + maxWidth);
        }
        this.maxWidth = maxWidth;
        return this;
    }

    /**
     * The printer to write this document's content through.
     *
     * @return the document's printer
     * @throws IllegalStateException if the document has been finished or closed
     */
    public Printer printer() {
        if (closed) {
            throw new IllegalStateException("the document has been finished or closed");
        }
        if (printer == null) {
            Libhegel lib = Engine.real();
            long options = lib.printerOptionsNew();
            try {
                Printer.check(lib, lib.printerOptionsSetMaxWidth(options, maxWidth), "options_set_max_width");
                printer = new Printer(lib, lib.printerNew(options), regions);
            } finally {
                lib.printerOptionsFree(options);
            }
        }
        return printer;
    }

    /**
     * Splice in every deferred region, lay the document out and return it, releasing the document.
     *
     * @return the rendered document
     */
    public String finish() {
        String text = printer().read();
        close();
        return text;
    }

    /** Release the document without rendering it. Closing it again does nothing. */
    @Override
    public void close() {
        closed = true;
        regions.freeAll();
        if (printer != null) {
            printer.free();
            printer = null;
        }
    }
}
