namespace Hegel;

/// <summary>
/// Receives what a run reports as it happens. Override what you want to hear about; the default
/// implementations do nothing. <see cref="Printing"/> writes the standard report to a writer and
/// <see cref="Silent"/> discards it.
/// </summary>
public abstract class Reporter
{
    /// <summary>The run is starting with the effective <paramref name="settings"/>.</summary>
    public virtual void RunStarted(Settings settings) { }

    /// <summary>The engine printed <paramref name="line"/> (a warning, statistics, a replay note).</summary>
    public virtual void EngineOutput(string line) { }

    /// <summary>A test case is starting; <paramref name="finalReplay"/> when it is a failure's final report.</summary>
    public virtual void CaseStarted(bool finalReplay) { }

    /// <summary>A test case printed <paramref name="line"/> (a draw or a note).</summary>
    public virtual void CaseOutput(string line, bool finalReplay) { }

    /// <summary>A test case ended with <paramref name="outcome"/>.</summary>
    public virtual void CaseFinished(CaseOutcome outcome, bool finalReplay) { }

    /// <summary>The run found <paramref name="count"/> distinct failures, reported next.</summary>
    public virtual void FailuresFound(int count) { }

    /// <summary>One failure's report, after its test case's output.</summary>
    public virtual void Failure(Failure failure) { }

    /// <summary>The run ended with <paramref name="report"/>.</summary>
    public virtual void RunFinished(RunReport report) { }

    /// <summary>The standard report, written to <paramref name="output"/>.</summary>
    public static Reporter Printing(TextWriter output) => new PrintingReporter(output);

    /// <summary>A reporter that discards everything.</summary>
    public static Reporter Silent { get; } = new SilentReporter();

    private sealed class SilentReporter : Reporter
    {
    }
}

internal sealed class PrintingReporter : Reporter
{
    private readonly TextWriter _out;
    private bool _printBlob;
    private bool _quiet;
    private bool _verbose;
    private bool _multiple;

    internal PrintingReporter(TextWriter output)
    {
        _out = output;
    }

    public override void RunStarted(Settings settings)
    {
        _printBlob = settings.PrintBlob;
        _quiet = settings.Verbosity == Verbosity.Quiet;
        _verbose = settings.Verbosity >= Verbosity.Verbose;
    }

    public override void EngineOutput(string line) => _out.WriteLine(line);

    public override void CaseStarted(bool finalReplay)
    {
        if (finalReplay && _multiple && !_quiet)
        {
            _out.WriteLine();
        }
    }

    public override void CaseOutput(string line, bool finalReplay)
    {
        if (!_quiet)
        {
            _out.WriteLine(line);
        }
    }

    public override void CaseFinished(CaseOutcome outcome, bool finalReplay)
    {
        if (!_verbose || finalReplay)
        {
            return;
        }
        if (outcome == CaseOutcome.Invalid)
        {
            _out.WriteLine("Test case stopped: failed assumption");
        }
        else if (outcome == CaseOutcome.Overrun)
        {
            _out.WriteLine("Test case stopped: out of data");
        }
    }

    public override void FailuresFound(int count)
    {
        _multiple = count > 1;
        if (_multiple && !_quiet)
        {
            _out.WriteLine($"Property-based test failed with {count} distinct failures.");
        }
    }

    public override void Failure(Failure failure)
    {
        if (!_quiet && failure.Caveat != null)
        {
            _out.WriteLine("note: " + failure.Caveat);
        }
        if (_printBlob && failure.ReproduceBlob != null)
        {
            _out.WriteLine();
            _out.WriteLine("To reproduce this failure, add this to the test's Settings:");
            _out.WriteLine($"    ReproduceFailure = [\"{failure.ReproduceBlob}\"]");
        }
    }
}
