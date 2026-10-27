package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

/** The built-in reporters and the {@link Label} constants. */
class ReporterTest {
    /** The captured output with platform line endings normalised, so exact comparisons hold on Windows. */
    private static String normalizedOutput(ByteArrayOutputStream buf) {
        return buf.toString(StandardCharsets.UTF_8).replace("\r\n", "\n");
    }

    @Test
    void silentReporterIgnoresEveryCallback() {
        Reporter silent = Reporter.silent();
        RunReport report = new RunReport(RunStatus.PASSED, new RunStatistics.Counter().snapshot(), List.of());
        Failure failure = new Failure("origin", "blob", null, new AssertionError("x"), Map.of(), List.of());
        silent.runStarted(new Settings());
        silent.engineOutput("line");
        silent.caseStarted(false);
        silent.caseOutput("var x = 1;", true);
        silent.caseFinished(CaseOutcome.VALID, false);
        silent.failuresFound(1);
        silent.failure(failure);
        silent.runFinished(report);
    }

    @Test
    void printingReporterPrintsCaseOutputAndEngineOutput() {
        ByteArrayOutputStream buf = new ByteArrayOutputStream();
        Reporter printing = Reporter.printing(new PrintStream(buf, true, StandardCharsets.UTF_8));
        printing.runStarted(new Settings().printBlob(false));
        printing.engineOutput("engine line");
        printing.caseStarted(true); // a lone failure: no separating blank line
        printing.caseOutput("var xs = [1, 2];", true);
        printing.caseOutput("a note", true);
        printing.caseFinished(CaseOutcome.INTERESTING, true);
        printing.failure(new Failure("o", "b64", null, new AssertionError("x"), Map.of(), List.of())); // printBlob off
        assertEquals("engine line\nvar xs = [1, 2];\na note\n", normalizedOutput(buf));
    }

    @Test
    void printingReporterPrintsTheCaveatThenTheReproducer() {
        ByteArrayOutputStream buf = new ByteArrayOutputStream();
        Reporter printing = Reporter.printing(new PrintStream(buf, true, StandardCharsets.UTF_8));
        printing.runStarted(new Settings().printBlob(true));
        printing.failure(new Failure(
                "o", "b64", "nondeterministic failure, confirmed", new AssertionError("x"), Map.of(), List.of()));
        assertEquals(
                "note: nondeterministic failure, confirmed\n\nTo reproduce this failure, call this on the test's"
                        + " Settings:\n    .reproduceFailure(\"b64\")\n",
                normalizedOutput(buf));
    }

    @Test
    void printingReporterHonoursVerbosity() {
        ByteArrayOutputStream quietBuf = new ByteArrayOutputStream();
        Reporter quiet = Reporter.printing(new PrintStream(quietBuf, true, StandardCharsets.UTF_8));
        quiet.runStarted(new Settings().verbosity(Verbosity.QUIET));
        quiet.caseOutput("var x = 1;", true);
        quiet.caseOutput("hidden", true);
        assertEquals("", quietBuf.toString(StandardCharsets.UTF_8));

        // Verbose: non-final output prints like final output (the runner decides what to send).
        ByteArrayOutputStream verboseBuf = new ByteArrayOutputStream();
        Reporter verbose = Reporter.printing(new PrintStream(verboseBuf, true, StandardCharsets.UTF_8));
        verbose.runStarted(new Settings().verbosity(Verbosity.VERBOSE));
        verbose.caseOutput("var x = 1;", false);
        verbose.caseOutput("shown", false);
        assertEquals("var x = 1;\nshown\n", normalizedOutput(verboseBuf));
    }

    @Test
    void labelsMatchTheEngineConstants() {
        assertEquals(Label.of("hegel.lists"), Label.LIST);
        assertEquals(Label.of("hegel.sets"), Label.SET);
        assertEquals(Label.of("hegel.dicts"), Label.MAP);
        assertEquals(Label.of("hegel.tuples"), Label.TUPLE);
        assertEquals(Label.of("hegel.one_of"), Label.ONE_OF);
        assertEquals(Label.of("hegel.optional"), Label.OPTIONAL);
        assertEquals(Label.of("hegel.fixed_dicts"), Label.FIXED_DICT);
        assertEquals(Label.of("hegel.flat_map"), Label.FLAT_MAP);
        assertEquals(Label.of("hegel.filter"), Label.FILTER);
        assertEquals(Label.of("hegel.filter.attempt"), Label.FILTER_ATTEMPT);
        assertEquals(Label.of("hegel.deferred"), Label.DEFERRED);
        assertEquals(Label.of("hegel.from_lark"), Label.FROM_LARK);
        assertEquals(Label.of("hegel.generator"), Label.GENERATOR);
        assertEquals(Label.of("hegel.map"), Label.MAPPED);
        assertEquals(Label.of("hegel.sampled_from"), Label.SAMPLED_FROM);
        assertEquals(Label.of("hegel.stateful.rule"), Label.STATEFUL_RULE);
        // Minted labels are stable and match the hash the composite generator already uses.
        assertEquals(Label.COMPOSITE, Label.of("hegel.composite"));
        assertEquals(Label.of("my.frontend.pair"), Label.of("my.frontend.pair"));
    }
}
