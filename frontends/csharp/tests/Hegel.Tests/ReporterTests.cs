using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class ReporterTests
{
    private static Failure NewFailure(string? blob, string? caveat) =>
        new("Origin", blob, caveat, new InvalidOperationException("x"), new Dictionary<string, object?>(), ["var a_1 = 1;"]);

    private static (PrintingReporter Reporter, StringWriter Out) Printing(Settings settings)
    {
        var output = new StringWriter { NewLine = "\n" };
        var reporter = (PrintingReporter)Reporter.Printing(output);
        reporter.RunStarted(settings);
        return (reporter, output);
    }

    [Fact]
    public void BaseReporterHooksDoNothing()
    {
        var reporter = Reporter.Silent;
        reporter.RunStarted(Engines.Quiet);
        reporter.EngineOutput("x");
        reporter.CaseStarted(true);
        reporter.CaseOutput("x", true);
        reporter.CaseFinished(CaseOutcome.Valid, false);
        reporter.FailuresFound(2);
        reporter.Failure(NewFailure("b", "c"));
        reporter.RunFinished(new RunReport(RunStatus.Passed, new RunStatistics(0, 0, 0, 0), []));
        Assert.Same(Reporter.Silent, Reporter.Silent);
    }

    [Fact]
    public void EngineOutputIsAlwaysPrinted()
    {
        var (reporter, output) = Printing(Engines.Quiet);
        reporter.EngineOutput("engine says");
        Assert.Equal("engine says\n", output.ToString());
    }

    [Fact]
    public void CaseOutputIsSuppressedWhenQuiet()
    {
        var (reporter, output) = Printing(Engines.Quiet);
        reporter.CaseOutput("line", false);
        reporter.CaseOutput("line", true);
        Assert.Equal("", output.ToString());
    }

    [Fact]
    public void CaseOutputIsPrintedWhenNotQuiet()
    {
        var (reporter, output) = Printing(Engines.Quiet with { Verbosity = Verbosity.Normal });
        reporter.CaseOutput("line", false);
        Assert.Equal("line\n", output.ToString());
    }

    [Fact]
    public void CaseFinishedExplainsStoppedCasesOnlyWhenVerbose()
    {
        var (quiet, quietOut) = Printing(Engines.Quiet with { Verbosity = Verbosity.Normal });
        quiet.CaseFinished(CaseOutcome.Invalid, false);
        quiet.CaseFinished(CaseOutcome.Overrun, false);
        Assert.Equal("", quietOut.ToString());

        var (verbose, verboseOut) = Printing(Engines.Quiet with { Verbosity = Verbosity.Verbose });
        verbose.CaseFinished(CaseOutcome.Valid, false);
        verbose.CaseFinished(CaseOutcome.Interesting, false);
        verbose.CaseFinished(CaseOutcome.Invalid, true);
        verbose.CaseFinished(CaseOutcome.Invalid, false);
        verbose.CaseFinished(CaseOutcome.Overrun, false);
        Assert.Equal("Test case stopped: failed assumption\nTest case stopped: out of data\n", verboseOut.ToString());
    }

    [Fact]
    public void MultipleFailuresAreAnnouncedAndSeparated()
    {
        var (reporter, output) = Printing(Engines.Quiet with { Verbosity = Verbosity.Normal });
        reporter.FailuresFound(1);
        reporter.CaseStarted(true);
        reporter.FailuresFound(2);
        reporter.CaseStarted(false);
        reporter.CaseStarted(true);
        Assert.Equal("Property-based test failed with 2 distinct failures.\n\n", output.ToString());
    }

    [Fact]
    public void MultipleFailuresAreSilentWhenQuiet()
    {
        var (reporter, output) = Printing(Engines.Quiet);
        reporter.FailuresFound(2);
        reporter.CaseStarted(true);
        Assert.Equal("", output.ToString());
    }

    [Fact]
    public void FailurePrintsTheCaveatUnlessQuiet()
    {
        var (reporter, output) = Printing(Engines.Quiet with { Verbosity = Verbosity.Normal });
        reporter.Failure(NewFailure("blob", "flaky"));
        reporter.Failure(NewFailure(null, null));
        Assert.Equal("note: flaky\n", output.ToString());

        var (quiet, quietOut) = Printing(Engines.Quiet);
        quiet.Failure(NewFailure("blob", "flaky"));
        Assert.Equal("", quietOut.ToString());
    }

    [Fact]
    public void FailurePrintsTheBlobWhenAsked()
    {
        var (reporter, output) = Printing(Engines.Quiet with { PrintBlob = true });
        reporter.Failure(NewFailure("abc", null));
        reporter.Failure(NewFailure(null, null));
        Assert.Equal("\nTo reproduce this failure, add this to the test's Settings:\n    ReproduceFailure = [\"abc\"]\n", output.ToString());
    }
}
