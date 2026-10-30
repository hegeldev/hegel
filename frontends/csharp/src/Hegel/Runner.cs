using System.Diagnostics;
using Hegel.Native;

namespace Hegel;

internal static class Runner
{
    private static readonly string[] InfraPrefixes = ["System.", "Microsoft.", "Xunit.", "NUnit.", "TUnit.", "MSTest."];

    internal const string StaleBlob =
        "ReproduceFailure: no supplied failure blob reproduced a failure. The failure may have been fixed — or, for a nondeterministic blob, it may not have recurred within the replay budget. Re-run to retry, or remove the blob once the failure is fixed.";

    private sealed record CaseRun(bool Captured, IReadOnlyList<string> Output, IReadOnlyDictionary<string, object?> Draws, Exception? Interesting, string? Origin)
    {
        internal int Rank => Captured ? 1 : 0;
    }

    private sealed record Verdict(IReadOnlyList<Failure> Failures, RunReport? Error);

    internal static RunReport Run(ILibhegel lib, Settings settings, Action<TestCase> body, Reporter reporter)
    {
        var report = WithSettings(lib, settings, s =>
        {
            var effective = settings.Effective(
                checked((long)lib.SettingsGetTestCases(s)),
                lib.SettingsGetPrintBlob(s),
                EnumCodes.StrictnessFrom(lib.SettingsGetNondeterminismStrictness(s)),
                EnumCodes.VerbosityFrom(lib.SettingsGetVerbosity(s)));
            reporter.RunStarted(effective);
            var counts = new RunStatistics.Counter();
            if (effective.ReproduceFailure.Count > 0)
            {
                return Replay(lib, s, effective, body, reporter, counts);
            }
            IReadOnlyList<Failure> explicitFailures = [];
            if (effective.Examples.Count > 0 && (lib.SettingsGetPhases(s) & Abi.PhaseExplicit) != 0)
            {
                var reportMultiple = lib.SettingsGetReportMultipleFailures(s);
                explicitFailures = RunExamples(lib, effective, body, reporter, reportMultiple);
                if (explicitFailures.Count > 0 && !reportMultiple)
                {
                    return ReportFailures(reporter, counts, explicitFailures, null);
                }
            }
            var verdict = Drive(lib, lib.RunStart(s, reporter.EngineOutput), effective, body, reporter, counts, explicitFailures);
            var failures = explicitFailures.Concat(verdict.Failures).ToList();
            if (failures.Count == 0)
            {
                return verdict.Error ?? new RunReport(RunStatus.Passed, counts.Snapshot(), []);
            }
            return ReportFailures(reporter, counts, failures, verdict.Error);
        });
        reporter.RunFinished(report);
        return report;
    }

    private static RunReport Replay(ILibhegel lib, nint s, Settings settings, Action<TestCase> body, Reporter reporter, RunStatistics.Counter counts)
    {
        var failures = new List<Failure>();
        RunReport? error = null;
        foreach (var blob in settings.ReproduceFailure)
        {
            var verdict = Drive(lib, lib.RunStartBlob(s, blob, reporter.EngineOutput), settings, body, reporter, counts, failures);
            failures.AddRange(verdict.Failures);
            error = verdict.Error;
            if (error != null)
            {
                break;
            }
        }
        if (failures.Count == 0)
        {
            if (error != null)
            {
                return error;
            }
            throw new DidNotReproduceException(StaleBlob);
        }
        return ReportFailures(reporter, counts, failures, error);
    }

    private static IReadOnlyList<Failure> RunExamples(ILibhegel lib, Settings settings, Action<TestCase> body, Reporter reporter, bool reportMultiple)
    {
        var failures = new List<Failure>();
        var origins = new HashSet<string>();
        foreach (var example in settings.Examples)
        {
            var failure = RunExample(lib, example, settings, body, reporter);
            if (failure != null && origins.Add(failure.Origin))
            {
                failures.Add(failure);
                if (!reportMultiple)
                {
                    break;
                }
            }
        }
        return failures;
    }

    private static Failure? RunExample(ILibhegel lib, Example example, Settings settings, Action<TestCase> body, Reporter reporter)
    {
        var verbose = settings.Verbosity >= Verbosity.Verbose;
        var quiet = settings.Verbosity == Verbosity.Quiet;
        using var draws = new ExplicitDraws(lib, example);
        var testCase = TestCase.Explicit(draws);
        if (verbose)
        {
            reporter.CaseStarted(false);
        }
        Exception? failure = null;
        try
        {
            body(testCase);
        }
        catch (HegelUsageException)
        {
            throw;
        }
        catch (AssumeRejected)
        {
            throw new InvalidExampleException($"Explicit test cases must not be rejected, but explicit {example} was rejected (Assume or Reject)");
        }
        catch (LoopDone)
        {
        }
        catch (Exception e)
        {
            failure = e;
        }
        IReadOnlyList<string> output = quiet ? [] : draws.Output();
        if (verbose)
        {
            foreach (var line in output)
            {
                reporter.CaseOutput(line, false);
            }
            reporter.CaseFinished(failure == null ? CaseOutcome.Valid : CaseOutcome.Interesting, false);
        }
        if (failure == null)
        {
            draws.CheckConsumed();
            return null;
        }
        return new Failure(OriginOf(failure, settings.InfrastructureNamespaces), null, null, failure, testCase.Draws(), output);
    }

    private static T WithSettings<T>(ILibhegel lib, Settings settings, Func<nint, T> body)
    {
        var s = NewSettings(lib, null);
        try
        {
            ApplySettings(lib, s, settings);
            return body(s);
        }
        finally
        {
            lib.SettingsFree(s);
        }
    }

    internal static nint NewSettings(ILibhegel lib, string? profile)
    {
        nint s;
        if (profile == null)
        {
            CheckUserCall(lib, lib.SettingsNew(out s), "hegel_settings_new");
        }
        else
        {
            CheckUserCall(lib, lib.SettingsNewForProfile(profile, out s), "hegel_settings_new_for_profile");
        }
        return s;
    }

    internal static void CheckStringSetting(ILibhegel lib, string key, string value)
    {
        var s = NewSettings(lib, null);
        try
        {
            SetFromString(lib, s, key, value);
        }
        finally
        {
            lib.SettingsFree(s);
        }
    }

    private static void SetFromString(ILibhegel lib, nint s, string key, string value) =>
        CheckUserCall(lib, lib.SettingsSetFromString(s, key, value), "hegel_settings_set_from_string");

    internal static void RegisterProfile(ILibhegel lib, string name, Settings settings) =>
        WithSettings<object?>(lib, settings, s =>
        {
            CheckUserCall(lib, lib.SettingsRegisterProfile(name, s), "hegel_settings_register_profile");
            return null;
        });

    internal static void SetDefaultProfile(ILibhegel lib, string name) =>
        CheckUserCall(lib, lib.SetDefaultProfile(name), "hegel_set_default_profile");

    private static void CheckUserCall(ILibhegel lib, int rc, string op)
    {
        if (rc == Abi.EInvalidArg)
        {
            throw new ArgumentException(lib.LastErrorMessage());
        }
        if (rc != Abi.Ok)
        {
            throw new HegelException($"{op} failed (rc={rc}): {lib.LastErrorMessage()}");
        }
    }

    private static Verdict Drive(ILibhegel lib, nint run, Settings settings, Action<TestCase> body, Reporter reporter, RunStatistics.Counter counts, IReadOnlyList<Failure> reported)
    {
        try
        {
            var captures = new Dictionary<string, CaseRun>();
            while (true)
            {
                var tc = lib.NextTestCase(run);
                if (tc == 0)
                {
                    break;
                }
                var caseRun = DriveOneCase(lib, tc, body, settings, reporter, counts);
                if (caseRun.Interesting != null)
                {
                    if (!captures.TryGetValue(caseRun.Origin!, out var stored) || caseRun.Rank >= stored.Rank)
                    {
                        captures[caseRun.Origin!] = caseRun;
                    }
                }
            }
            var result = lib.RunResult(run);
            try
            {
                return VerdictOf(lib, result, counts, captures, reported);
            }
            finally
            {
                lib.RunResultFree(result);
            }
        }
        finally
        {
            lib.RunFree(run);
        }
    }

    private static Verdict VerdictOf(ILibhegel lib, nint result, RunStatistics.Counter counts, Dictionary<string, CaseRun> captures, IReadOnlyList<Failure> reported)
    {
        switch (lib.RunResultStatus(result))
        {
            case Abi.RunStatusPassed:
                return new Verdict([], null);
            case Abi.RunStatusError:
                var kind = EnumCodes.ErrorKindFrom(lib.RunResultErrorKind(result, out var healthCheck));
                return new Verdict([], new RunReport(RunStatus.Error, counts.Snapshot(), lib.RunResultError(result), kind, EnumCodes.HealthCheckFrom(healthCheck), []));
            default:
                return new Verdict(EngineFailures(lib, result, captures, reported), null);
        }
    }

    private static IReadOnlyList<Failure> EngineFailures(ILibhegel lib, nint result, Dictionary<string, CaseRun> captures, IReadOnlyList<Failure> alreadyReported)
    {
        var reported = alreadyReported.Select(f => f.Origin).ToHashSet();
        var count = lib.RunResultFailureCount(result);
        var failures = new List<Failure>();
        for (ulong i = 0; i < count; i++)
        {
            var origin = lib.FailureOrigin(result, i);
            if (!captures.Remove(origin, out var capture))
            {
                throw new HegelException($"internal error: failure {i} ({origin}) has no captured failing execution");
            }
            if (reported.Contains(origin))
            {
                continue;
            }
            failures.Add(new Failure(origin, lib.FailureBlob(result, i), lib.FailureCaveat(result, i), capture.Interesting!, capture.Draws, capture.Output));
        }
        return failures;
    }

    private static RunReport ReportFailures(Reporter reporter, RunStatistics.Counter counts, IReadOnlyList<Failure> failures, RunReport? error)
    {
        reporter.FailuresFound(failures.Count);
        foreach (var failure in failures)
        {
            reporter.CaseStarted(true);
            foreach (var line in failure.Output)
            {
                reporter.CaseOutput(line, true);
            }
            reporter.CaseFinished(CaseOutcome.Interesting, true);
            reporter.Failure(failure);
        }
        if (error == null)
        {
            return new RunReport(RunStatus.Failed, counts.Snapshot(), failures);
        }
        return new RunReport(RunStatus.Failed, counts.Snapshot(), error.Error, error.ErrorKind, error.FailedHealthCheck, failures);
    }

    private static CaseRun DriveOneCase(ILibhegel lib, nint tc, Action<TestCase> body, Settings settings, Reporter reporter, RunStatistics.Counter counts)
    {
        var source = new LiveDataSource(lib, tc);
        try
        {
            var testCase = new TestCase(source, lib.TestCaseShouldCapture(tc), settings.Verbosity >= Verbosity.Verbose);
            try
            {
                return RunBody(lib, tc, testCase, body, settings, reporter, counts);
            }
            finally
            {
                testCase.ReleaseAll();
            }
        }
        finally
        {
            try
            {
                source.ReleasePools();
            }
            finally
            {
                lib.TestCaseFree(tc);
            }
        }
    }

    private static CaseRun RunBody(ILibhegel lib, nint tc, TestCase testCase, Action<TestCase> body, Settings settings, Reporter reporter, RunStatistics.Counter counts)
    {
        reporter.CaseStarted(false);
        CaseOutcome outcome;
        string? origin = null;
        Exception? interesting = null;
        try
        {
            body(testCase);
            outcome = CaseOutcome.Valid;
        }
        catch (LoopDone)
        {
            outcome = CaseOutcome.Valid;
        }
        catch (AssumeRejected)
        {
            outcome = CaseOutcome.Invalid;
        }
        catch (StopTest)
        {
            outcome = CaseOutcome.Overrun;
        }
        catch (Exception e) when (e is HegelException or HegelUsageException)
        {
            throw;
        }
        catch (Exception e)
        {
            outcome = CaseOutcome.Interesting;
            origin = OriginOf(e, settings.InfrastructureNamespaces);
            interesting = e;
        }
        var output = Output(testCase, interesting);
        if (settings.Verbosity >= Verbosity.Verbose)
        {
            foreach (var line in output)
            {
                reporter.CaseOutput(line, false);
            }
        }
        var rc = lib.MarkComplete(tc, (int)outcome, origin);
        if (rc != Abi.Ok)
        {
            throw new HegelException($"hegel_mark_complete failed (rc={rc}): {lib.LastErrorMessage()}");
        }
        counts.Record(outcome);
        reporter.CaseFinished(outcome, false);
        return testCase.IsFinal
            ? new CaseRun(true, output, testCase.Draws(), interesting, origin)
            : new CaseRun(false, [], new Dictionary<string, object?>(), interesting, origin);
    }

    private static IReadOnlyList<string> Output(TestCase testCase, Exception? interesting)
    {
        try
        {
            return testCase.Output();
        }
        catch (Exception e) when (interesting != null)
        {
            interesting.Data["HegelOutputError"] = e;
            return [];
        }
    }

    internal static void ApplySettings(ILibhegel lib, nint s, Settings st)
    {
        lib.SettingsTestCases(s, (ulong)st.TestCases);
        lib.SettingsVerbosity(s, (int)st.Verbosity);
        lib.SettingsSeed(s, st.Seed ?? 0, st.Seed.HasValue);
        lib.SettingsDerandomize(s, st.Derandomize);
        lib.SettingsReportMultipleFailures(s, st.ReportMultipleFailures);
        lib.SettingsStopOnFirstFailure(s, st.StopOnFirstFailure);
        lib.SettingsShowStatistics(s, st.ShowStatistics);
        lib.SettingsUnboundedChoices(s, st.UnboundedChoices);
        lib.SettingsPrintBlob(s, st.PrintBlob);
        lib.SettingsNondeterminismStrictness(s, (int)st.NondeterminismStrictness);
        lib.SettingsBackend(s, (int)st.Backend);
        lib.SettingsSuppressHealthCheck(s, (int)st.SuppressHealthCheck);
        lib.SettingsPhases(s, (int)st.Phases);
        lib.SettingsDatabase(s, st.Database.ForEngine());
        if (st.Name != null)
        {
            lib.SettingsDatabaseKey(s, st.Name);
        }
        if (st.TestLocation is { } l)
        {
            lib.SettingsTestLocation(s, l.File, l.BeginLine, l.ClassName, l.Function);
        }
        foreach (var setting in st.StringSettings)
        {
            SetFromString(lib, s, setting.Key, setting.Value);
        }
    }

    internal static string OriginOf(Exception e, IReadOnlyList<string> infrastructureNamespaces)
    {
        foreach (var frame in new StackTrace(e, true).GetFrames())
        {
            var type = frame.GetMethod()?.DeclaringType;
            var file = frame.GetFileName();
            if (type == null || file == null || !IsUserFrame(type, infrastructureNamespaces))
            {
                continue;
            }
            return $"{e.GetType().Name} at {Path.GetFileName(file)}:{frame.GetFileLineNumber()}";
        }
        return e.GetType().FullName ?? e.GetType().Name;
    }

    internal static bool IsUserFrame(Type type, IReadOnlyList<string> infrastructureNamespaces)
    {
        if (HegelInfrastructureAttribute.Marks(type.Assembly))
        {
            return false;
        }
        var name = type.FullName ?? "";
        return !InfraPrefixes.Concat(infrastructureNamespaces).Any(prefix => name.StartsWith(prefix, StringComparison.Ordinal));
    }
}
