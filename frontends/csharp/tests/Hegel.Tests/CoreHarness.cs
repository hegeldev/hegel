using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

internal sealed class RawRun : IDisposable
{
    private readonly nint _settings;
    private readonly nint _run;
    private readonly List<TestCase> _cases = [];
    private readonly List<string> _lines = [];
    private bool _completed;

    internal RawRun(Settings? settings = null)
    {
        Lib = Engines.Real;
        _settings = Runner.NewSettings(Lib, null);
        Runner.ApplySettings(Lib, _settings, settings ?? Engines.Quiet with { TestCases = 1 });
        _run = Lib.RunStart(_settings, _lines.Add);
        Tc = Lib.NextTestCase(_run);
        _lines.Clear();
    }

    internal static RawRun Verbose() => new(Engines.Quiet with { TestCases = 1, Verbosity = Verbosity.Verbose });

    internal ILibhegel Lib { get; }

    internal nint Tc { get; }

    internal LiveDataSource Source(ILibhegel? lib = null) => new(lib ?? Lib, Tc);

    internal TestCase Case(bool wantsOutput = true, ILibhegel? lib = null)
    {
        var tc = new TestCase(Source(lib), wantsOutput);
        _cases.Add(tc);
        return tc;
    }

    internal IReadOnlyList<string> Complete()
    {
        if (!_completed)
        {
            _completed = true;
            foreach (var tc in _cases)
            {
                tc.ReleaseAll();
            }
            Lib.MarkComplete(Tc, Abi.StatusValid, null, null, null);
        }
        return _lines;
    }

    public void Dispose()
    {
        Complete();
        Lib.TestCaseFree(Tc);
        Lib.RunFree(_run);
        Lib.SettingsFree(_settings);
    }
}

internal static class Runs
{
    internal static Failure Failing(Action<TestCase> body, Settings? settings = null)
    {
        var report = Property.Run(tc =>
        {
            body(tc);
            throw new InvalidOperationException("boom");
        }, settings ?? Engines.Reported, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        return Assert.Single(report.Failures);
    }

    internal static RunReport Passing(Action<TestCase> body, Settings? settings = null)
    {
        var report = Property.Run(body, settings ?? Engines.Quiet, Reporter.Silent);
        Assert.True(report.Passed, report.Error ?? string.Join("\n", report.Failures.Select(f => f.Exception.ToString())));
        return report;
    }

    internal static T Accepted<T>(Func<TestCase, T> draw)
    {
        for (var attempt = 0; attempt < 100; attempt++)
        {
            using var run = new RawRun();
            try
            {
                return draw(run.Case(wantsOutput: false));
            }
            catch (AssumeRejected)
            {
            }
        }
        throw new InvalidOperationException("the engine rejected every attempt");
    }

    internal static string Render(object? value, int width = Document.DefaultMaxWidth)
    {
        using var doc = new Document { MaxWidth = width };
        doc.Printer.Repr(value);
        return doc.Finish();
    }

    internal static string Print(Action<Printer> body, int width = Document.DefaultMaxWidth)
    {
        using var doc = new Document { MaxWidth = width };
        body(doc.Printer);
        return doc.Finish();
    }
}
