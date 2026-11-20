using System.Diagnostics;
using Hegel.Native;

namespace Hegel;

internal static class Runner
{
    private static readonly string[] InfraPrefixes = ["System.", "Microsoft.", "Xunit.", "NUnit.", "TUnit.", "MSTest."];

    internal const string StaleBlob =
        "ReproduceFailure: no supplied failure blob reproduced a failure. The failure may have been fixed — or, for a nondeterministic blob, it may not have recurred within the replay budget. Re-run to retry, or remove the blob once the failure is fixed.";

    private sealed record CaseRun(bool WantedOutput, IReadOnlyDictionary<string, object?> Draws, Exception? Interesting, string? Origin);

    private sealed record Verdict(IReadOnlyList<Failure> Failures, RunReport? Error);

    internal static RunReport Run(ILibhegel lib, Settings settings, Action<TestCase> body, Reporter reporter)
    {
        var report = WithSettings(lib, settings, s =>
        {
            var effective = settings.Effective(
                Settings.TestCasesFrom(lib.SettingsGetTestCases(s)),
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
            var verdict = Drive(lib, lib.RunStart(s, reporter.EngineSink), effective, body, reporter, counts, explicitFailures);
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
            var verdict = Drive(lib, lib.RunStartBlob(s, blob, reporter.EngineSink), settings, body, reporter, counts, failures);
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
            UserCode.Call(body, testCase);
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
        if (rc == Abi.EInvalidArg || rc == Abi.EUnknownSetting)
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
            var failing = new Dictionary<string, CaseRun>();
            while (true)
            {
                var tc = lib.NextTestCase(run);
                if (tc == 0)
                {
                    break;
                }
                var caseRun = DriveOneCase(lib, tc, body, settings, reporter, counts);
                if (caseRun.Interesting != null && (caseRun.WantedOutput || !failing.ContainsKey(caseRun.Origin!)))
                {
                    failing[caseRun.Origin!] = caseRun;
                }
            }
            var result = lib.RunResult(run);
            try
            {
                return VerdictOf(lib, result, counts, failing, reported);
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

    private static Verdict VerdictOf(ILibhegel lib, nint result, RunStatistics.Counter counts, Dictionary<string, CaseRun> failing, IReadOnlyList<Failure> reported)
    {
        switch (lib.RunResultStatus(result))
        {
            case Abi.RunStatusPassed:
                return new Verdict([], null);
            case Abi.RunStatusError:
                var kind = EnumCodes.ErrorKindFrom(lib.RunResultErrorKind(result, out var healthCheck));
                return new Verdict([], new RunReport(RunStatus.Error, counts.Snapshot(), lib.RunResultError(result), kind, EnumCodes.HealthCheckFrom(healthCheck), []));
            default:
                return new Verdict(EngineFailures(lib, result, failing, reported), null);
        }
    }

    private static IReadOnlyList<Failure> EngineFailures(ILibhegel lib, nint result, Dictionary<string, CaseRun> failing, IReadOnlyList<Failure> alreadyReported)
    {
        var reported = alreadyReported.Select(f => f.Origin).ToHashSet();
        var count = lib.RunResultFailureCount(result);
        var failures = new List<Failure>();
        for (ulong i = 0; i < count; i++)
        {
            var origin = lib.FailureOrigin(result, i);
            if (!failing.Remove(origin, out var caseRun))
            {
                throw new HegelException($"internal error: failure {i} ({origin}) has no failing execution");
            }
            if (reported.Contains(origin))
            {
                continue;
            }
            var output = Printer.Lines(lib.FailureOutput(result, i) ?? "");
            failures.Add(new Failure(origin, lib.FailureBlob(result, i), lib.FailureCaveat(result, i), caseRun.Interesting!, caseRun.Draws, output));
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
            var wantsOutput = lib.TestCaseWantsOutput(tc);
            var testCase = new TestCase(source, wantsOutput);
            try
            {
                return RunBody(lib, tc, source, testCase, body, settings, reporter, counts);
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
                source.Release();
            }
        }
    }

    private static CaseRun RunBody(ILibhegel lib, nint tc, LiveDataSource source, TestCase testCase, Action<TestCase> body, Settings settings, Reporter reporter, RunStatistics.Counter counts)
    {
        reporter.CaseStarted(false);
        CaseOutcome outcome;
        string? origin = null;
        Exception? interesting = null;
        try
        {
            UserCode.Call(body, testCase);
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
        if (source.IsAborted)
        {
            outcome = source.IsRejected ? CaseOutcome.Invalid : CaseOutcome.Overrun;
            origin = null;
            interesting = null;
        }
        var summary = interesting != null && testCase.WantsOutput ? SummaryOf(interesting, settings.InfrastructureNamespaces) : null;
        var detail = interesting != null && lib.TestCaseWantsDetail(tc) ? interesting.ToString() : null;
        var rc = lib.MarkComplete(tc, (int)outcome, origin, summary, detail);
        if (rc != Abi.Ok)
        {
            throw new HegelException($"hegel_mark_complete failed (rc={rc}): {lib.LastErrorMessage()}");
        }
        counts.Record(outcome);
        reporter.CaseFinished(outcome, false);
        return new CaseRun(testCase.WantsOutput, testCase.Draws(), interesting, origin);
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

    internal static string OriginOf(Exception e, IReadOnlyList<string> infrastructureNamespaces) =>
        UserFrame(e, infrastructureNamespaces) is { } frame
            ? $"{e.GetType().Name} at {Path.GetFileName(frame.GetFileName())}:{frame.GetFileLineNumber()}"
            : e.GetType().FullName ?? e.GetType().Name;

    internal static string SummaryOf(Exception e, IReadOnlyList<string> infrastructureNamespaces)
    {
        var message = e.Message;
        var newline = message.IndexOfAny(['\n', '\r']);
        if (newline >= 0)
        {
            message = (message[..newline].TrimEnd() + " …").TrimStart();
        }
        var text = message.Length == 0 ? e.GetType().Name : $"{e.GetType().Name}: {message}";
        if (UserFrame(e, infrastructureNamespaces) is not { } frame)
        {
            return text;
        }
        var column = frame.GetFileColumnNumber();
        var location = column > 0 ? $"{frame.GetFileLineNumber()}:{column}" : $"{frame.GetFileLineNumber()}";
        return $"{frame.GetFileName()}:{location}: {text}";
    }

    private static StackFrame? UserFrame(Exception e, IReadOnlyList<string> infrastructureNamespaces) =>
        new StackTrace(e, true).GetFrames().FirstOrDefault(frame =>
            frame.GetMethod()?.DeclaringType is { } type && frame.GetFileName() != null && IsUserFrame(type, infrastructureNamespaces));

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
