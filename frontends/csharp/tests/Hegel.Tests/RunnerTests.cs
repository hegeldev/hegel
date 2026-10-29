using System.Reflection;
using System.Reflection.Emit;
using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class RunnerTests
{
    [Fact]
    public void ExplicitExamplesMustSupplyValuesOfTheDrawnType()
    {
        var wrong = Assert.Throws<InvalidExampleException>(() => Property.Run(tc => tc.Draw(Generators.Longs()), Engines.Quiet with { Examples = [Example.Of(1)] }, Reporter.Silent));
        Assert.Contains("gives draw draw_1 a System.Int32, but the draw expects a System.Int64", wrong.Message);
        var nullForStruct = Assert.Throws<InvalidExampleException>(() => Property.Run(tc => tc.Draw(Generators.Integers()), Engines.Quiet with { Examples = [Example.Of((object?)null)] }, Reporter.Silent));
        Assert.Contains("gives draw draw_1 a null", nullForStruct.Message);
        string? drawn = "unset";
        Property.Run(tc => drawn = tc.Draw(Generators.Text()), Engines.Quiet with { Examples = [Example.Of((object?)null)], Phases = Phase.Explicit }, Reporter.Silent);
        Assert.Null(drawn);
    }

    private static readonly Settings Quiet = Engines.Quiet;

    private sealed class Recording : Reporter
    {
        internal readonly List<string> Events = [];
        internal Settings? Started;
        internal RunReport? Finished;

        public override void RunStarted(Settings settings) => Started = settings;
        public override void EngineOutput(string line) => Events.Add($"engine:{line}");
        public override void CaseStarted(bool finalReplay) => Events.Add($"start:{finalReplay}");
        public override void CaseOutput(string line, bool finalReplay) => Events.Add($"out:{line}:{finalReplay}");
        public override void CaseFinished(CaseOutcome outcome, bool finalReplay) => Events.Add($"end:{outcome}:{finalReplay}");
        public override void FailuresFound(int count) => Events.Add($"failures:{count}");
        public override void Failure(Failure failure) => Events.Add($"failure:{failure.Origin}");
        public override void RunFinished(RunReport report) => Finished = report;
    }

    private static void FailAbove(TestCase tc, int limit)
    {
        var x = tc.Draw(Generators.Integers().Min(0), "x");
        if (x >= limit)
        {
            throw new InvalidOperationException($"too big: {x}");
        }
    }

    private static string Blob()
    {
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet, Reporter.Silent);
        return Assert.Single(report.Failures).ReproduceBlob!;
    }

    [Fact]
    public void APassingRunReportsItsStatistics()
    {
        var reporter = new Recording();
        var report = Property.Run(tc => tc.Draw(Generators.Integers()), Quiet with { TestCases = 5 }, reporter);
        Assert.True(report.Passed);
        Assert.Equal(5, report.Statistics.Valid);
        Assert.Equal(5, report.Statistics.Total);
        Assert.Same(report, reporter.Finished);
        Assert.Equal(5, reporter.Started!.TestCases);
        Assert.Equal(["start:False", "end:Valid:False"], reporter.Events.Take(2));
    }

    [Fact]
    public void AFailingRunShrinksAndReportsTheFailure()
    {
        var reporter = new Recording();
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet, reporter);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.StartsWith("InvalidOperationException at RunnerTests.cs:", failure.Origin);
        Assert.Equal("too big: 10", failure.Exception.Message);
        Assert.Equal(10, failure.Draws["x_1"]);
        Assert.Equal(["var x_1 = 10;"], failure.Output);
        Assert.NotNull(failure.ReproduceBlob);
        Assert.Null(failure.Caveat);
        Assert.True(report.Statistics.Interesting > 0);
        Assert.Equal(["failures:1", "start:True", "out:var x_1 = 10;:True", "end:Interesting:True", $"failure:{failure.Origin}"], reporter.Events.TakeLast(5));
    }

    [Fact]
    public void TestThrowsTheFailure()
    {
        var e = Assert.Throws<InvalidOperationException>(() => Property.Test(tc => FailAbove(tc, 10), Quiet, Reporter.Silent));
        Assert.Equal("too big: 10", e.Message);
    }

    [Fact]
    public void RunRequiresABody()
    {
        Assert.Throws<ArgumentNullException>(() => Property.Run(null!, Quiet, Reporter.Silent));
    }

    [Fact]
    public void TheDefaultReporterPrintsToStandardError()
    {
        var original = Console.Error;
        var captured = new StringWriter();
        Console.SetError(captured);
        try
        {
            Assert.Throws<InvalidOperationException>(() => Property.Test(tc => FailAbove(tc, 10), Quiet with { Verbosity = Verbosity.Normal }));
        }
        finally
        {
            Console.SetError(original);
        }
        Assert.Contains("var x_1 = 10;", captured.ToString());
    }

    [Fact]
    public void TheCallerFillsInTheNameAndLocation()
    {
        var reporter = new Recording();
        Property.Run(tc => { }, Quiet, reporter);
        var started = reporter.Started!;
        Assert.EndsWith("RunnerTests.cs::TheCallerFillsInTheNameAndLocation", started.Name);
        Assert.EndsWith("RunnerTests.cs", started.TestLocation!.File);
        Assert.DoesNotContain('\\', started.TestLocation.File);
        Assert.Equal(typeof(RunnerTests).FullName, started.TestLocation.ClassName);
        Assert.Equal(nameof(TheCallerFillsInTheNameAndLocation), started.TestLocation.Function);
        Assert.True(started.TestLocation.BeginLine > 0);
    }

    [Fact]
    public void GivenNameAndLocationAreKept()
    {
        var reporter = new Recording();
        var location = new TestLocation("f.cs", 1, "C", "M");
        Property.Run(tc => { }, Quiet with { Name = "given", TestLocation = location }, reporter);
        Assert.Equal("given", reporter.Started!.Name);
        Assert.Equal(location, reporter.Started.TestLocation);
        Property.Run(tc => { }, Quiet with { Name = "only-name" }, reporter);
        Assert.Equal("only-name", reporter.Started!.Name);
        Assert.Equal(nameof(GivenNameAndLocationAreKept), reporter.Started.TestLocation!.Function);
    }

    [Fact]
    public void RunnerAcceptsSettingsWithoutANameOrLocation()
    {
        var report = Runner.Run(Engines.Real, Quiet with { TestCases = 2 }, tc => tc.Draw(Generators.Integers()), Reporter.Silent);
        Assert.True(report.Passed);
        Assert.Equal(2, report.Statistics.Total);
    }

    [Fact]
    public void StringSettingsAreApplied()
    {
        var report = Property.Run(tc => tc.Draw(Generators.Integers()), Quiet.Set("test_cases", "3"), Reporter.Silent);
        Assert.Equal(3, report.Statistics.Total);
    }

    [Fact]
    public void EngineOutputReachesTheReporter()
    {
        var reporter = new Recording();
        Property.Run(tc => tc.Draw(Generators.Integers()), Quiet with { ShowStatistics = true, Verbosity = Verbosity.Normal, TestCases = 3 }, reporter);
        Assert.Contains(reporter.Events, e => e.StartsWith("engine:Statistics (over 3 test cases)"));
    }

    [Fact]
    public void RejectedCasesAreCounted()
    {
        var reporter = new Recording();
        var report = Property.Run(tc =>
        {
            var x = tc.Draw(Generators.Integers(), "x");
            tc.Assume(x % 2 == 0);
        }, Quiet with { TestCases = 20, Verbosity = Verbosity.Verbose }, reporter);
        Assert.True(report.Passed);
        Assert.True(report.Statistics.Invalid > 0);
        Assert.Contains("end:Invalid:False", reporter.Events);
        Assert.Contains(reporter.Events, e => e.StartsWith("out:var x_1 = "));
    }

    [Fact]
    public void AnOverrunIsCountedAndALargeInitialTestCaseFailsAHealthCheck()
    {
        var reporter = new Recording();
        var report = Property.Run(tc =>
        {
            while (true)
            {
                tc.GenerateInteger(0, long.MaxValue);
            }
        }, Quiet with { TestCases = 2 }, reporter);
        Assert.Equal(1, report.Statistics.Overrun);
        Assert.Contains("end:Overrun:False", reporter.Events);
        Assert.Equal(HealthCheck.LargeInitialTestCase, report.FailedHealthCheck);
        Assert.Throws<HealthCheckFailure>(report.ThrowIfFailed);
    }

    [Fact]
    public void MultipleFailuresAreReportedWhenAsked()
    {
        var report = Property.Run(tc =>
        {
            var x = tc.Draw(Generators.Integers());
            if (x % 2 == 0)
            {
                throw new InvalidOperationException("even");
            }
            throw new ArgumentException("odd");
        }, Quiet with { ReportMultipleFailures = true }, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        Assert.Equal(2, report.Failures.Count);
        var e = Assert.Throws<MultipleFailuresException>(report.ThrowIfFailed);
        Assert.StartsWith("Property-based test failed with 2 distinct failures.", e.Message);
    }

    [Fact]
    public void StopOnFirstFailureDoesNotShrink()
    {
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet with { StopOnFirstFailure = true, Seed = 1 }, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        Assert.Single(report.Failures);
    }

    [Fact]
    public void AnUnsatisfiableTestIsAnError()
    {
        var report = Property.Run(tc => tc.Reject(), Quiet, Reporter.Silent);
        Assert.Equal(RunStatus.Error, report.Status);
        Assert.Equal(RunErrorKind.Unsatisfiable, report.ErrorKind);
        Assert.Throws<UnsatisfiableException>(report.ThrowIfFailed);
        Assert.Throws<UnsatisfiableException>(() => Property.Test(tc => tc.Reject(), Quiet, Reporter.Silent));
    }

    [Fact]
    public void FilteringTooMuchFailsAHealthCheckUnlessSuppressed()
    {
        var report = Property.Run(tc =>
        {
            tc.Draw(Generators.Integers());
            tc.Assume(false);
        }, Quiet, Reporter.Silent);
        Assert.Equal(RunStatus.Error, report.Status);
        Assert.Equal(RunErrorKind.HealthCheck, report.ErrorKind);
        Assert.True(report.HealthCheckFailed);
        Assert.Equal(HealthCheck.FilterTooMuch, report.FailedHealthCheck);
        var e = Assert.Throws<HealthCheckFailure>(report.ThrowIfFailed);
        Assert.Equal(HealthCheck.FilterTooMuch, e.HealthCheck);

        var suppressed = Property.Run(tc =>
        {
            tc.Draw(Generators.Integers());
            tc.Assume(false);
        }, Quiet with { SuppressHealthCheck = HealthCheck.FilterTooMuch }, Reporter.Silent);
        Assert.Equal(RunErrorKind.Unsatisfiable, suppressed.ErrorKind);
    }

    [Fact]
    public void AFlakyFailureIsAnErrorUnderStrictNondeterminism()
    {
        var n = 0;
        var report = Property.Run(tc =>
        {
            tc.Draw(Generators.Integers());
            if (n++ == 0)
            {
                throw new InvalidOperationException("once");
            }
        }, Quiet with { NondeterminismStrictness = NondeterminismStrictness.Error }, Reporter.Silent);
        Assert.Equal(RunStatus.Error, report.Status);
        Assert.Equal(RunErrorKind.Flaky, report.ErrorKind);
        Assert.Empty(report.Failures);
        Assert.Throws<FlakyTestException>(report.ThrowIfFailed);
    }

    [Fact]
    public void AFlakyFailureCarriesACaveatWhenOnlyWarned()
    {
        var n = 0;
        var reporter = new Recording();
        var report = Property.Run(tc =>
        {
            tc.Draw(Generators.Integers());
            if (n++ == 0)
            {
                throw new InvalidOperationException("once");
            }
        }, Quiet with { NondeterminismStrictness = NondeterminismStrictness.Warn }, reporter);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.True(failure.Nondeterministic);
        Assert.StartsWith("unconfirmed failure", failure.Caveat);
        Assert.Equal("once", failure.Exception.Message);
    }

    [Fact]
    public void NondeterministicDrawsAreAnErrorUnderStrictNondeterminism()
    {
        var n = 0;
        var report = Property.Run(tc =>
        {
            if (n++ % 2 == 0)
            {
                tc.Draw(Generators.Integers());
            }
            else
            {
                tc.Draw(Generators.Booleans());
            }
            throw new InvalidOperationException("always");
        }, Quiet with { NondeterminismStrictness = NondeterminismStrictness.Error }, Reporter.Silent);
        Assert.Equal(RunErrorKind.Nondeterministic, report.ErrorKind);
        Assert.Throws<FlakyTestException>(report.ThrowIfFailed);
    }

    [Fact]
    public void ABlobReplaysTheFailure()
    {
        var blob = Blob();
        var reporter = new Recording();
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet with { ReproduceFailure = [blob] }, reporter);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.Equal(["var x_1 = 10;"], failure.Output);
        Assert.Equal(1, report.Statistics.Total);
        Assert.Contains("failures:1", reporter.Events);
    }

    [Fact]
    public void BlobsReplayingTheSameFailureReportItOnce()
    {
        var blob = Blob();
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet with { ReproduceFailure = [blob, blob] }, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        Assert.Equal(2, report.Statistics.Total);
        Assert.Single(report.Failures);
    }

    [Fact]
    public void AStaleBlobThrows()
    {
        var blob = Blob();
        var e = Assert.Throws<DidNotReproduceException>(() => Property.Run(tc => tc.Draw(Generators.Integers().Min(0)), Quiet with { ReproduceFailure = [blob] }, Reporter.Silent));
        Assert.Equal(Runner.StaleBlob, e.Message);
    }

    [Fact]
    public void AnUndecodableBlobIsAnEngineError()
    {
        var e = Assert.Throws<HegelException>(() => Property.Run(tc => { }, Quiet with { ReproduceFailure = ["not a blob"] }, Reporter.Silent));
        Assert.StartsWith("hegel_run_start_blob failed (rc=-5)", e.Message);
    }

    [Fact]
    public void AnErrorWhileReplayingIsReturned()
    {
        var blob = Blob();
        using var engine = Engines.Override(new ErroringResults(Engines.Real, failFrom: 1));
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet with { ReproduceFailure = [blob] }, Reporter.Silent);
        Assert.Equal(RunStatus.Error, report.Status);
        Assert.Equal(RunErrorKind.Internal, report.ErrorKind);
        Assert.Equal("fake error", report.Error);
        Assert.Empty(report.Failures);
    }

    [Fact]
    public void AnErrorAfterAReplayedFailureStopsTheReplayButKeepsTheFailure()
    {
        var blob = Blob();
        using var engine = Engines.Override(new ErroringResults(Engines.Real, failFrom: 2));
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet with { ReproduceFailure = [blob, blob, blob] }, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        Assert.Equal("fake error", report.Error);
        Assert.Equal(RunErrorKind.Internal, report.ErrorKind);
        Assert.Single(report.Failures);
        Assert.Equal(2, report.Statistics.Total);
    }

    [Fact]
    public void ExplicitExamplesRunBeforeGeneration()
    {
        var seen = new List<int>();
        var report = Property.Run(tc => seen.Add(tc.Draw(Generators.Integers(), "x")), Quiet with { Examples = [Example.Of(42), Example.Of().With("x_1", 43)], TestCases = 1 }, Reporter.Silent);
        Assert.True(report.Passed);
        Assert.Equal([42, 43], seen.Take(2));
        Assert.Equal(3, seen.Count);
        Assert.Equal(1, report.Statistics.Total);
    }

    [Fact]
    public void ExplicitExamplesAreSkippedWithoutTheExplicitPhase()
    {
        var report = Property.Run(tc => Assert.NotEqual(42, tc.Draw(Generators.Integers().Max(10))), Quiet with { Examples = [Example.Of(42)], Phases = Phase.All & ~Phase.Explicit }, Reporter.Silent);
        Assert.True(report.Passed);
    }

    [Fact]
    public void AFailingExampleIsReportedWithoutGenerating()
    {
        var generated = 0;
        var reporter = new Recording();
        var report = Property.Run(tc =>
        {
            var x = tc.Draw(Generators.Integers(), "x");
            if (x == 42)
            {
                throw new InvalidOperationException("explicit");
            }
            generated++;
        }, Quiet with { Examples = [Example.Of(42)], Verbosity = Verbosity.Normal }, reporter);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.StartsWith("InvalidOperationException at RunnerTests.cs:", failure.Origin);
        Assert.Null(failure.ReproduceBlob);
        Assert.Equal(["var x_1 = 42;"], failure.Output);
        Assert.Equal(42, failure.Draws["x_1"]);
        Assert.Equal(0, generated);
        Assert.Equal(0, report.Statistics.Total);
        Assert.Equal(["failures:1", "start:True", "out:var x_1 = 42;:True", "end:Interesting:True", $"failure:{failure.Origin}"], reporter.Events);
    }

    [Fact]
    public void FailingExamplesWithTheSameOriginAreReportedOnce()
    {
        var report = Property.Run(tc =>
        {
            var x = tc.Draw(Generators.Integers(), "x");
            if (x >= 40)
            {
                throw new InvalidOperationException("big");
            }
        }, Quiet with { Examples = [Example.Of(42), Example.Of(43)], ReportMultipleFailures = true }, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.Equal(42, failure.Draws["x_1"]);
        Assert.True(report.Statistics.Total > 0);
    }

    [Fact]
    public void DistinctFailingExamplesAreAllReported()
    {
        var report = Property.Run(tc =>
        {
            var x = tc.Draw(Generators.Integers().Max(10), "x");
            if (x == 42)
            {
                throw new InvalidOperationException("a");
            }
            if (x == 43)
            {
                throw new ArgumentException("b");
            }
        }, Quiet with { Examples = [Example.Of(42), Example.Of(43)], ReportMultipleFailures = true }, Reporter.Silent);
        Assert.Equal(2, report.Failures.Count);
    }

    [Fact]
    public void ExamplesAreRunVerbosely()
    {
        var reporter = new Recording();
        Property.Run(tc =>
        {
            tc.Draw(Generators.Integers(), "x");
            tc.Note("noted");
        }, Quiet with { Examples = [Example.Of(1)], TestCases = 1, Verbosity = Verbosity.Verbose }, reporter);
        Assert.Equal(["start:False", "out:var x_1 = 1;:False", "out:noted:False", "end:Valid:False"], reporter.Events.Take(4));
    }

    [Fact]
    public void AFailingExampleRunVerboselyIsInteresting()
    {
        var reporter = new Recording();
        Property.Run(tc =>
        {
            tc.Draw(Generators.Integers(), "x");
            throw new InvalidOperationException("x");
        }, Quiet with { Examples = [Example.Of(1)], Verbosity = Verbosity.Verbose }, reporter);
        Assert.Equal(["start:False", "out:var x_1 = 1;:False", "end:Interesting:False", "failures:1"], reporter.Events.Take(4));
    }

    [Fact]
    public void AQuietFailingExampleHasNoOutput()
    {
        var report = Property.Run(tc =>
        {
            tc.Draw(Generators.Integers(), "x");
            throw new InvalidOperationException("x");
        }, Quiet with { Examples = [Example.Of(1)] }, Reporter.Silent);
        Assert.Empty(Assert.Single(report.Failures).Output);
    }

    [Fact]
    public void AnExampleMayRepeat()
    {
        var runs = 0;
        var report = Property.Run(tc => tc.Repeat(() => runs++), Quiet with { Examples = [Example.Of()], Phases = Phase.Explicit }, Reporter.Silent);
        Assert.True(report.Passed);
        Assert.Equal(1, runs);
    }

    [Fact]
    public void ARejectedExampleIsInvalid()
    {
        var e = Assert.Throws<InvalidExampleException>(() => Property.Run(tc => tc.Assume(false), Quiet with { Examples = [Example.Of(1)] }, Reporter.Silent));
        Assert.Equal("Explicit test cases must not be rejected, but explicit Example(1) was rejected (Assume or Reject)", e.Message);
    }

    [Fact]
    public void AnExampleThatIsNotFullyUsedIsInvalid()
    {
        var e = Assert.Throws<InvalidExampleException>(() => Property.Run(tc => { }, Quiet with { Examples = [Example.Of(1)] }, Reporter.Silent));
        Assert.Equal("Explicit example Example(1) was not fully used: 1 positional value(s) were not drawn", e.Message);
    }

    [Fact]
    public void AnExampleWithTooFewValuesIsInvalid()
    {
        var e = Assert.Throws<InvalidExampleException>(() => Property.Run(tc => tc.Draw(Generators.Integers(), "x"), Quiet with { Examples = [Example.Of()] }, Reporter.Silent));
        Assert.StartsWith("Explicit example Example() has no value for draw x_1", e.Message);
    }

    [Fact]
    public void UsageErrorsInAnExampleAbortTheRun()
    {
        var e = Assert.Throws<HegelUsageException>(() => Property.Run(tc => tc.Draw(new Unprintable()), Quiet with { Examples = [Example.Of(1)] }, Reporter.Silent));
        Assert.Contains("cannot print the values it draws", e.Message);
    }

    [Fact]
    public void UsageErrorsInABodyAbortTheRun()
    {
        var usage = Assert.Throws<HegelUsageException>(() => Property.Run(tc => throw new HegelUsageException("bad"), Quiet, Reporter.Silent));
        Assert.Equal("bad", usage.Message);
        var engine = Assert.Throws<HegelException>(() => Property.Run(tc => throw new HegelException("worse"), Quiet, Reporter.Silent));
        Assert.Equal("worse", engine.Message);
    }

    [Fact]
    public void OriginsSkipInfrastructureFrames()
    {
        var report = Property.Run(tc => Thrower.Throw(), Quiet, Reporter.Silent);
        var origin = Assert.Single(report.Failures).Origin;
        Assert.StartsWith("InvalidOperationException at RunnerTests.cs:", origin);
        var line = int.Parse(origin[(origin.LastIndexOf(':') + 1)..]);

        var skipping = Property.Run(tc => Thrower.Throw(), Quiet with { InfrastructureNamespaces = [typeof(Thrower).FullName!] }, Reporter.Silent);
        var skipped = Assert.Single(skipping.Failures).Origin;
        Assert.StartsWith("InvalidOperationException at RunnerTests.cs:", skipped);
        Assert.NotEqual(line, int.Parse(skipped[(skipped.LastIndexOf(':') + 1)..]));
    }

    [Fact]
    public void OriginOfAnExceptionWithoutAUserFrameIsItsTypeName()
    {
        Assert.Equal("System.InvalidOperationException", Runner.OriginOf(new InvalidOperationException(), []));
        var report = Property.Run(tc => int.Parse("x"), Quiet with { InfrastructureNamespaces = ["Hegel.Tests."] }, Reporter.Silent);
        Assert.Equal("System.FormatException", Assert.Single(report.Failures).Origin);
    }

    [Fact]
    public void FramesOfThisLibraryAreNeverUserFrames()
    {
        Assert.False(Runner.IsUserFrame(typeof(Runner), []));
        Assert.True(Runner.IsUserFrame(typeof(RunnerTests), []));
        Assert.False(Runner.IsUserFrame(typeof(RunnerTests), ["Hegel.Tests"]));
        Assert.False(Runner.IsUserFrame(typeof(string), []));
    }

    [Fact]
    public void FramesOfAnAssemblyMarkedAsInfrastructureAreNeverUserFrames()
    {
        var marker = typeof(HegelInfrastructureAttribute).GetConstructor([])!;
        var infrastructure = AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("Hegel.Tests.Infrastructure"), AssemblyBuilderAccess.Run, [new CustomAttributeBuilder(marker, [])]);
        var infrastructureType = infrastructure.DefineDynamicModule("m").DefineType("Frame").CreateType();
        Assert.False(Runner.IsUserFrame(infrastructureType, []));
        var plain = AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("Hegel.Tests.Plain"), AssemblyBuilderAccess.Run);
        var plainType = plain.DefineDynamicModule("m").DefineType("Frame").CreateType();
        Assert.True(Runner.IsUserFrame(plainType, []));
    }

    [Fact]
    public void AnOriginWithoutACaptureIsAnInternalError()
    {
        using var engine = Engines.Override(new BogusOrigins(Engines.Real));
        var e = Assert.Throws<HegelException>(() => Property.Run(tc => FailAbove(tc, 10), Quiet, Reporter.Silent));
        Assert.Equal("internal error: failure 0 (bogus) has no captured failing execution", e.Message);
    }

    [Fact]
    public void AFailedMarkCompleteIsAnEngineError()
    {
        using var engine = Engines.Override(new FailingMarkComplete(Engines.Real));
        var e = Assert.Throws<HegelException>(() => Property.Run(tc => { }, Quiet, Reporter.Silent));
        Assert.StartsWith("hegel_mark_complete failed (rc=-8): ", e.Message);
    }

    [Fact]
    public void AnOutputErrorIsAttachedToTheFailure()
    {
        using var engine = Engines.Override(new FailingPrinterValue(Engines.Real));
        var report = Property.Run(tc => FailAbove(tc, 10), Quiet, Reporter.Silent);
        var failure = Assert.Single(report.Failures);
        Assert.Empty(failure.Output);
        var attached = Assert.IsType<HegelException>(failure.Exception.Data["HegelOutputError"]);
        Assert.StartsWith("hegel_printer_value failed (rc=-8)", attached.Message);
    }

    [Fact]
    public void AnOutputErrorInAPassingCaseAbortsTheRun()
    {
        using var engine = Engines.Override(new FailingPrinterValue(Engines.Real));
        var e = Assert.Throws<HegelException>(() => Property.Run(tc => tc.Draw(Generators.Integers()), Quiet with { Verbosity = Verbosity.Verbose }, Reporter.Silent));
        Assert.StartsWith("hegel_printer_value failed (rc=-8)", e.Message);
    }

    [Fact]
    public void AnUnknownErrorKindIsReportedAsNull()
    {
        using var engine = Engines.Override(new ErroringResults(Engines.Real, failFrom: 1, kind: 99));
        var report = Property.Run(tc => { }, Quiet, Reporter.Silent);
        Assert.Equal(RunStatus.Error, report.Status);
        Assert.Null(report.ErrorKind);
        Assert.Null(report.FailedHealthCheck);
        Assert.Throws<HegelRunException>(report.ThrowIfFailed);
    }

    [Fact]
    public void AnErrorAfterAFailingExampleKeepsBoth()
    {
        using var engine = Engines.Override(new ErroringResults(Engines.Real, failFrom: 1));
        var report = Property.Run(tc =>
        {
            var x = tc.Draw(Generators.Integers().Max(10), "x");
            if (x == 42)
            {
                throw new InvalidOperationException("a");
            }
        }, Quiet with { Examples = [Example.Of(42)], ReportMultipleFailures = true }, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        Assert.Single(report.Failures);
        Assert.Equal("fake error", report.Error);
    }

    private static class Thrower
    {
        internal static void Throw() => throw new InvalidOperationException("thrown");
    }

    private sealed class Unprintable : Generator<int>
    {
        public override int DoDraw(TestCase tc) => 1;
    }

    private sealed class ErroringResults(ILibhegel inner, int failFrom, int kind = Abi.RunErrorInternal) : DelegatingLibhegel(inner)
    {
        private int _results;

        private bool Failing => _results >= failFrom;

        public override nint RunResult(nint run)
        {
            _results++;
            return base.RunResult(run);
        }

        public override int RunResultStatus(nint result) => Failing ? Abi.RunStatusError : base.RunResultStatus(result);

        public override int RunResultErrorKind(nint result, out int healthCheck)
        {
            if (!Failing)
            {
                return base.RunResultErrorKind(result, out healthCheck);
            }
            healthCheck = 0;
            return kind;
        }

        public override string RunResultError(nint result) => Failing ? "fake error" : base.RunResultError(result);
    }

    private sealed class BogusOrigins(ILibhegel inner) : DelegatingLibhegel(inner)
    {
        public override string FailureOrigin(nint result, ulong index) => "bogus";
    }

    private sealed class FailingMarkComplete(ILibhegel inner) : DelegatingLibhegel(inner)
    {
        public override int MarkComplete(nint testCase, int status, string? origin) => Abi.EInternal;
    }

    private sealed class FailingPrinterValue(ILibhegel inner) : DelegatingLibhegel(inner)
    {
        public override int PrinterValue(nint printer, out string value)
        {
            value = "";
            return Abi.EInternal;
        }
    }
}
