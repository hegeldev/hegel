using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

internal sealed class Boom : Exception
{
    public Boom()
        : base("boom")
    {
    }
}

internal sealed class RawIntegers : Generator<int>
{
    public override int DoDraw(TestCase tc) => (int)tc.GenerateInteger(0, 3);
}

internal static class Gen
{
    internal static Settings Quiet(int cases) => Engines.Quiet with { TestCases = cases };

    internal static List<T> Sample<T>(Generator<T> g, int cases = 50)
    {
        var values = new List<T>();
        var report = Property.Run(tc => values.Add(tc.DrawSilent(g)), Quiet(cases), Reporter.Silent);
        Assert.True(report.Passed, Describe(report));
        return values;
    }

    internal static void All<T>(Generator<T> g, Func<T, bool> ok, int cases = 50)
    {
        var values = Sample(g, cases);
        Assert.NotEmpty(values);
        foreach (var value in values)
        {
            Assert.True(ok(value), $"unexpected value {Show(value)}");
        }
    }

    internal static List<(T Value, string Text)> Printed<T>(Generator<T> g, int cases = 20)
    {
        var results = new List<(T, string)>();
        var report = Property.Run(tc =>
        {
            var doc = new Document();
            try
            {
                var value = tc.DrawAndPrint(g, doc.Printer);
                results.Add((value, doc.Finish()));
            }
            finally
            {
                doc.Dispose();
            }
        }, Quiet(cases), Reporter.Silent);
        Assert.True(report.Passed, Describe(report));
        Assert.NotEmpty(results);
        return results;
    }

    internal static Failure Minimal<T>(Generator<T> g, Func<T, bool>? fails = null)
    {
        var report = Property.Run(tc =>
        {
            var value = tc.Draw(g, "x");
            if (fails == null || fails(value))
            {
                throw new Boom();
            }
        }, Engines.Quiet, Reporter.Silent);
        Assert.Equal(RunStatus.Failed, report.Status);
        return Assert.Single(report.Failures);
    }

    internal static string MinimalOutput<T>(Generator<T> g, Func<T, bool>? fails = null) =>
        string.Join("\n", Minimal(g, fails).Output);

    private static string Describe(RunReport report) =>
        report.Error ?? string.Join("\n", report.Failures.Select(f => f.Exception.ToString()));

    private static string Show<T>(T value) => value switch
    {
        null => "null",
        System.Collections.IEnumerable e and not string => "[" + string.Join(", ", e.Cast<object?>().Select(Show)) + "]",
        _ => value.ToString() ?? "",
    };
}
