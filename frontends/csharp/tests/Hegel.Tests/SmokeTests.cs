using Xunit;

namespace Hegel.Tests;

public class SmokeTests
{
    [Fact]
    public void EngineVersionIsReported()
    {
        Assert.Matches(@"^\d+\.\d+\.\d+", Property.EngineVersion);
    }

    [Fact]
    public void PassingPropertyPasses()
    {
        var report = Property.Test(tc =>
        {
            var a = tc.Draw(Generators.Integers());
            var b = tc.Draw(Generators.Integers());
            Assert.Equal(a + b, b + a);
        }, new Settings { Database = Database.Disabled, Verbosity = Verbosity.Quiet });
        Assert.True(report.Passed);
    }

    [Fact]
    public void FailingPropertyShrinksAndReports()
    {
        var output = new StringWriter();
        var report = Property.Run(tc =>
        {
            var xs = tc.Draw(Generators.Lists(Generators.Integers()), "xs");
            Assert.True(xs.Sum(x => (long)x) < 1000);
        }, new Settings { Database = Database.Disabled, PrintBlob = false }, Reporter.Printing(output));
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.StartsWith("TrueException at SmokeTests.cs:", failure.Origin);
        Assert.Equal("var xs_1 = new List<int> {1000};", Assert.Single(failure.Output));
        Assert.Equal(["xs_1"], failure.Draws.Keys);
    }
}
