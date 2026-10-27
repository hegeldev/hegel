package dev.hegel;

import java.io.PrintStream;

/**
 * The default {@link Reporter}: prints the classic report to a stream, exactly as the runner did
 * before reporters existed.
 */
final class PrintingReporter implements Reporter {
    private final PrintStream out;
    private boolean printBlob;
    private boolean annotated;
    private boolean quiet;
    private boolean verbose;
    private boolean multiple;

    PrintingReporter(PrintStream out) {
        this.out = out;
    }

    @Override
    public void runStarted(Settings settings) {
        printBlob = settings.printBlob;
        annotated = settings.annotated;
        quiet = settings.verbosity == Verbosity.QUIET;
        verbose = settings.verbosity == Verbosity.VERBOSE || settings.verbosity == Verbosity.DEBUG;
    }

    @Override
    public void engineOutput(String line) {
        out.println(line);
    }

    @Override
    public void caseStarted(boolean finalReplay) {
        // With several failures, a blank line separates one counterexample's report from the next.
        if (finalReplay && multiple && !quiet) {
            out.println();
        }
    }

    @Override
    public void caseOutput(String line, boolean finalReplay) {
        if (!quiet) {
            out.println(line);
        }
    }

    @Override
    public void caseFinished(CaseOutcome outcome, boolean finalReplay) {
        if (!verbose || finalReplay) {
            return;
        }
        if (outcome == CaseOutcome.INVALID) {
            out.println("Test case stopped: failed assumption");
        } else if (outcome == CaseOutcome.OVERRUN) {
            out.println("Test case stopped: out of data");
        }
    }

    @Override
    public void failuresFound(int count) {
        multiple = count > 1;
        if (multiple && !quiet) {
            out.println("Property-based test failed with " + count + " distinct failures.");
        }
    }

    @Override
    public void failure(Failure failure) {
        if (!quiet) {
            failure.caveat().ifPresent(caveat -> out.println("note: " + caveat));
        }
        if (printBlob) {
            failure.reproduceBlob().ifPresent(blob -> {
                out.println();
                if (annotated) {
                    out.println("To reproduce this failure, add this to the test's @HegelTest annotation:");
                    out.println("    reproduceFailure = \"" + blob + "\"");
                } else {
                    out.println("To reproduce this failure, call this on the test's Settings:");
                    out.println("    .reproduceFailure(\"" + blob + "\")");
                }
            });
        }
    }
}
