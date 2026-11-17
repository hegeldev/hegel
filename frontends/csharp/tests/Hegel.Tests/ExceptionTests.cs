using Xunit;

namespace Hegel.Tests;

public class ExceptionTests
{
    [Fact]
    public void HegelExceptionCarriesMessageAndCause()
    {
        var cause = new InvalidOperationException("cause");
        var e = new HegelException("boom", cause);
        Assert.Equal("boom", e.Message);
        Assert.Same(cause, e.InnerException);
        Assert.Null(new HegelException("boom").InnerException);
    }

    [Fact]
    public void RunExceptionsAreHegelExceptions()
    {
        Assert.IsAssignableFrom<HegelException>(new HegelRunException("m"));
        Assert.IsAssignableFrom<HegelRunException>(new HealthCheckFailure("m", HealthCheck.TooSlow));
        Assert.IsAssignableFrom<HegelRunException>(new FlakyTestException("m"));
        Assert.IsAssignableFrom<HegelRunException>(new UnsatisfiableException("m"));
        Assert.IsAssignableFrom<HegelException>(new DidNotReproduceException("m"));
        Assert.Equal(HealthCheck.TooSlow, new HealthCheckFailure("m", HealthCheck.TooSlow).HealthCheck);
        Assert.Null(new HealthCheckFailure("m", null).HealthCheck);
    }

    [Fact]
    public void UsageExceptionsAreArgumentExceptions()
    {
        Assert.IsAssignableFrom<ArgumentException>(new HegelUsageException("m"));
        Assert.IsAssignableFrom<HegelUsageException>(new InvalidExampleException("m"));
        Assert.Equal("m", new InvalidExampleException("m").Message);
    }

    [Fact]
    public void MultipleFailuresExceptionKeepsTheFailures()
    {
        var failures = new Exception[] { new InvalidOperationException("a"), new ArgumentException("b") };
        var e = new MultipleFailuresException("two", failures);
        Assert.Equal("two", e.Message);
        Assert.Equal(failures, e.Failures);
    }

    [Fact]
    public void ControlExceptionsHaveMessages()
    {
        Assert.Equal("the engine ended this test case", new StopTest().Message);
        Assert.Equal("the test case was rejected by an assumption", new AssumeRejected().Message);
        Assert.Equal("the repeat loop finished", new LoopDone().Message);
        Assert.True(TestCase.IsControl(new StopTest()));
        Assert.False(TestCase.IsControl(new InvalidOperationException()));
    }

    [Fact]
    public void TestLocationIsARecord()
    {
        var a = new TestLocation("f.cs", 3, "C", "M");
        Assert.Equal(a, new TestLocation("f.cs", 3, "C", "M"));
        Assert.Equal("f.cs", a.File);
        Assert.Equal(3, a.BeginLine);
        Assert.Equal("C", a.ClassName);
        Assert.Equal("M", a.Function);
    }
}
