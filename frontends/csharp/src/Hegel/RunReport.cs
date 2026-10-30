using System.Runtime.ExceptionServices;

namespace Hegel;

/// <summary>What a run found: its status, statistics, failures and any error that stopped it.</summary>
public sealed class RunReport
{
    internal RunReport(RunStatus status, RunStatistics statistics, IReadOnlyList<Failure> failures)
        : this(status, statistics, null, null, null, failures)
    {
    }

    internal RunReport(RunStatus status, RunStatistics statistics, string? error, RunErrorKind? errorKind, HealthCheck? healthCheck, IReadOnlyList<Failure> failures)
    {
        Status = status;
        Statistics = statistics;
        Error = error;
        ErrorKind = errorKind;
        FailedHealthCheck = healthCheck;
        Failures = failures;
    }

    /// <summary>How the run ended.</summary>
    public RunStatus Status { get; }

    /// <summary>Whether no test case failed.</summary>
    public bool Passed => Status == RunStatus.Passed;

    /// <summary>How many test cases ran, by outcome.</summary>
    public RunStatistics Statistics { get; }

    /// <summary>The engine's message when the run stopped early.</summary>
    public string? Error { get; }

    /// <summary>Why the run stopped early.</summary>
    public RunErrorKind? ErrorKind { get; }

    /// <summary>Whether the run stopped because a health check failed.</summary>
    public bool HealthCheckFailed => ErrorKind == RunErrorKind.HealthCheck;

    /// <summary>The health check that failed, when one did.</summary>
    public HealthCheck? FailedHealthCheck { get; }

    /// <summary>The distinct failures found, each with its minimal test case.</summary>
    public IReadOnlyList<Failure> Failures { get; }

    /// <summary>
    /// Throw unless the run passed: a single failure's own exception, with its stack;
    /// <see cref="MultipleFailuresException"/> for several; a <see cref="HegelRunException"/> for a
    /// run that stopped early.
    /// </summary>
    public void ThrowIfFailed()
    {
        if (Status == RunStatus.Passed)
        {
            return;
        }
        if (Status == RunStatus.Error)
        {
            throw RunError();
        }
        if (Failures.Count == 1)
        {
            ExceptionDispatchInfo.Capture(Failures[0].Exception).Throw();
        }
        var failures = Failures.Select(f => f.Exception).ToList();
        var message = $"Property-based test failed with {failures.Count} distinct failures." + string.Concat(failures.Select(e => "\n\n" + Describe(e)));
        throw new MultipleFailuresException(message, failures);
    }

    private static string Describe(Exception e) =>
        string.IsNullOrEmpty(e.Message) ? e.GetType().FullName! : $"{e.GetType().FullName}: {e.Message}";

    private HegelRunException RunError()
    {
        var error = Error ?? "";
        return ErrorKind switch
        {
            RunErrorKind.HealthCheck => new HealthCheckFailure(error, FailedHealthCheck),
            RunErrorKind.Flaky or RunErrorKind.Nondeterministic => new FlakyTestException(error),
            RunErrorKind.Unsatisfiable => new UnsatisfiableException(error),
            _ => new HegelRunException(error),
        };
    }
}

/// <summary>How many test cases a run executed, by outcome.</summary>
public sealed class RunStatistics
{
    internal RunStatistics(long valid, long invalid, long overrun, long interesting)
    {
        Valid = valid;
        Invalid = invalid;
        Overrun = overrun;
        Interesting = interesting;
    }

    /// <summary>Every test case.</summary>
    public long Total => Valid + Invalid + Overrun + Interesting;

    /// <summary>Test cases that ran to completion.</summary>
    public long Valid { get; }

    /// <summary>Test cases rejected by an assumption.</summary>
    public long Invalid { get; }

    /// <summary>Test cases that ran out of data.</summary>
    public long Overrun { get; }

    /// <summary>Test cases that failed.</summary>
    public long Interesting { get; }

    /// <inheritdoc />
    public override string ToString() =>
        $"RunStatistics {{ Total = {Total}, Valid = {Valid}, Invalid = {Invalid}, Overrun = {Overrun}, Interesting = {Interesting} }}";

    internal sealed class Counter
    {
        private readonly long[] _counts = new long[4];

        internal void Record(CaseOutcome outcome) => _counts[(int)outcome]++;

        internal RunStatistics Snapshot() => new(_counts[0], _counts[1], _counts[2], _counts[3]);
    }
}

/// <summary>One distinct failure a run found, with the minimal test case that shows it.</summary>
public sealed class Failure
{
    internal Failure(string origin, string? reproduceBlob, string? caveat, Exception exception, IReadOnlyDictionary<string, object?> draws, IReadOnlyList<string> output)
    {
        Origin = origin;
        ReproduceBlob = reproduceBlob;
        Caveat = caveat;
        Exception = exception;
        Draws = draws;
        Output = output;
    }

    /// <summary>What distinguishes this failure from the others: the exception type and where it was thrown.</summary>
    public string Origin { get; }

    /// <summary>The blob that replays this failure through <see cref="Settings.ReproduceFailure"/>, when the engine has one.</summary>
    public string? ReproduceBlob { get; }

    /// <summary>The engine's warning about this failure (for example that the test is nondeterministic).</summary>
    public string? Caveat { get; }

    /// <summary>Whether the engine flagged this failure as nondeterministic.</summary>
    public bool Nondeterministic => Caveat != null;

    /// <summary>The exception the minimal test case threw.</summary>
    public Exception Exception { get; }

    /// <summary>The minimal test case's draws, by name, in order.</summary>
    public IReadOnlyDictionary<string, object?> Draws { get; }

    /// <summary>The minimal test case's printed output, line by line.</summary>
    public IReadOnlyList<string> Output { get; }
}
