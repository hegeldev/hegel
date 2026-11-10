package dev.hegel;

import java.io.PrintStream;

/**
 * The default {@link Reporter}: prints the classic report to a stream, exactly as the runner did
 * before reporters existed.
 */
final class PrintingReporter implements Reporter {
    private final PrintStream out;
    private final boolean standalone;
    private boolean printBlob;
    private boolean quiet;
    private boolean multiple;

    PrintingReporter(PrintStream out) {
        this(out, false);
    }

    /**
     * A reporter for a program run by {@link Hegel#main}. It prints each failure's stack trace,
     * since no test framework rethrows the exception, and gives {@code --failure-blob} as the way
     * to replay a failure.
     */
    static PrintingReporter forMain(PrintStream out) {
        return new PrintingReporter(out, true);
    }

    private PrintingReporter(PrintStream out, boolean standalone) {
        this.out = out;
        this.standalone = standalone;
    }

    @Override
    public void runStarted(Settings settings) {
        printBlob = Boolean.TRUE.equals(settings.printBlob);
        quiet = settings.verbosity == Verbosity.QUIET;
    }

    @Override
    public void engineOutput(String line) {
        out.println(line);
    }

    @Override
    public void caseStarted(boolean finalReplay) {
        // With several failures, a blank line separates one counterexample's report from the next.
        if (finalReplay && multiple) {
            out.println();
        }
    }

    @Override
    public void draw(String label, Object value, boolean finalReplay) {
        if (!quiet) {
            out.println(label + " = " + TestCase.repr(value) + ";");
        }
    }

    @Override
    public void note(String message, boolean finalReplay) {
        if (!quiet) {
            out.println(message);
        }
    }

    @Override
    public void failuresFound(int count) {
        multiple = count > 1;
        if (multiple) {
            out.println("Property-based test failed with " + count + " distinct failures.");
        }
    }

    @Override
    public void failure(Failure failure) {
        if (!quiet) {
            failure.caveat().ifPresent(caveat -> out.println("note: " + caveat));
            if (standalone) {
                failure.exception().printStackTrace(out);
            }
        }
        if (printBlob) {
            failure.reproduceBlob().ifPresent(blob -> {
                out.println();
                if (standalone) {
                    out.println("rerun with: --failure-blob " + blob);
                } else {
                    out.println("To reproduce this failure, replay it with:");
                    out.println("    @HegelTest(reproduceFailure = \"" + blob + "\")");
                }
            });
        }
    }
}
