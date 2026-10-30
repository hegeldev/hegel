using Xunit;

namespace Hegel.Tests;

public class RunReportTests
{
    private static readonly RunStatistics Stats = new(1, 2, 3, 4);

    private static Failure NewFailure(Exception e) =>
        new("o", null, null, e, new Dictionary<string, object?>(), []);

    [Fact]
    public void PassedReportDoesNotThrow()
    {
        var report = new RunReport(RunStatus.Passed, Stats, []);
        report.ThrowIfFailed();
        Assert.True(report.Passed);
        Assert.Null(report.Error);
        Assert.Null(report.ErrorKind);
        Assert.False(report.HealthCheckFailed);
        Assert.Null(report.FailedHealthCheck);
        Assert.Empty(report.Failures);
        Assert.Same(Stats, report.Statistics);
    }

    [Fact]
    public void ASingleFailureIsRethrownWithItsStack()
    {
        Exception thrown;
        try
        {
            throw new InvalidOperationException("one");
        }
        catch (InvalidOperationException caught)
        {
            thrown = caught;
        }
        var report = new RunReport(RunStatus.Failed, Stats, [NewFailure(thrown)]);
        var e = Assert.Throws<InvalidOperationException>(report.ThrowIfFailed);
        Assert.Same(thrown, e);
        Assert.Contains(nameof(ASingleFailureIsRethrownWithItsStack), e.StackTrace);
        Assert.False(report.Passed);
    }

    [Fact]
    public void SeveralFailuresBecomeAMultipleFailuresException()
    {
        var a = new InvalidOperationException("one");
        var b = new ArgumentException("");
        var report = new RunReport(RunStatus.Failed, Stats, [NewFailure(a), NewFailure(b)]);
        var e = Assert.Throws<MultipleFailuresException>(report.ThrowIfFailed);
        Assert.Equal([a, b], e.Failures);
        Assert.Equal(
            "Property-based test failed with 2 distinct failures.\n\nSystem.InvalidOperationException: one\n\nSystem.ArgumentException",
            e.Message);
    }

    [Theory]
    [InlineData(RunErrorKind.HealthCheck, typeof(HealthCheckFailure))]
    [InlineData(RunErrorKind.Flaky, typeof(FlakyTestException))]
    [InlineData(RunErrorKind.Nondeterministic, typeof(FlakyTestException))]
    [InlineData(RunErrorKind.Unsatisfiable, typeof(UnsatisfiableException))]
    [InlineData(RunErrorKind.Usage, typeof(HegelRunException))]
    [InlineData(RunErrorKind.Internal, typeof(HegelRunException))]
    [InlineData(null, typeof(HegelRunException))]
    public void ErrorsThrowTheMatchingRunException(RunErrorKind? kind, Type expected)
    {
        var report = new RunReport(RunStatus.Error, Stats, "went wrong", kind, HealthCheck.TooSlow, []);
        var e = Assert.Throws(expected, report.ThrowIfFailed);
        Assert.Equal("went wrong", e.Message);
        Assert.Equal(kind == RunErrorKind.HealthCheck, report.HealthCheckFailed);
        if (e is HealthCheckFailure hc)
        {
            Assert.Equal(HealthCheck.TooSlow, hc.HealthCheck);
        }
    }

    [Fact]
    public void AMissingErrorMessageBecomesEmpty()
    {
        var report = new RunReport(RunStatus.Error, Stats, null, RunErrorKind.Internal, null, []);
        Assert.Equal("", Assert.Throws<HegelRunException>(report.ThrowIfFailed).Message);
    }

    [Fact]
    public void StatisticsSumAndPrint()
    {
        Assert.Equal(10, Stats.Total);
        Assert.Equal(1, Stats.Valid);
        Assert.Equal(2, Stats.Invalid);
        Assert.Equal(3, Stats.Overrun);
        Assert.Equal(4, Stats.Interesting);
        Assert.Equal("RunStatistics { Total = 10, Valid = 1, Invalid = 2, Overrun = 3, Interesting = 4 }", Stats.ToString());
    }

    [Fact]
    public void CounterRecordsEachOutcome()
    {
        var counter = new RunStatistics.Counter();
        counter.Record(CaseOutcome.Valid);
        counter.Record(CaseOutcome.Interesting);
        counter.Record(CaseOutcome.Interesting);
        var snapshot = counter.Snapshot();
        Assert.Equal((1L, 0L, 0L, 2L), (snapshot.Valid, snapshot.Invalid, snapshot.Overrun, snapshot.Interesting));
    }

    [Fact]
    public void FailureExposesItsParts()
    {
        var e = new InvalidOperationException();
        var draws = new Dictionary<string, object?> { ["a_1"] = 1 };
        var failure = new Failure("Origin", "blob", "caveat", e, draws, ["out"]);
        Assert.Equal("Origin", failure.Origin);
        Assert.Equal("blob", failure.ReproduceBlob);
        Assert.Equal("caveat", failure.Caveat);
        Assert.True(failure.Nondeterministic);
        Assert.Same(e, failure.Exception);
        Assert.Same(draws, failure.Draws);
        Assert.Equal(["out"], failure.Output);
        Assert.False(new Failure("o", null, null, e, draws, []).Nondeterministic);
    }
}
