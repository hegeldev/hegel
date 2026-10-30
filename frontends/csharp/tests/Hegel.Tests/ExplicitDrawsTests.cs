using System.Reflection;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class ExplicitDrawsTests
{
    [Fact]
    public void TakesNamedValuesFirstThenPositionalOnes()
    {
        using var draws = new ExplicitDraws(Engines.Real, Example.Of(1, "two").With("n_1", 3.5));
        Assert.Equal(3.5, draws.Take("n_1"));
        Assert.Equal(1, draws.Take("a_1"));
        Assert.Equal("two", draws.Take("b_1"));
        draws.Note("first\nsecond");
        draws.CheckConsumed();
        Assert.Equal(["var n_1 = 3.5;", "var a_1 = 1;", "var b_1 = \"two\";", "first", "second"], draws.Output());
    }

    [Fact]
    public void ANamedValueCanBeTakenRepeatedly()
    {
        using var draws = new ExplicitDraws(Engines.Real, Example.Of().With("n_1", 3));
        Assert.Equal(3, draws.Take("n_1"));
        Assert.Equal(3, draws.Take("n_1"));
        draws.CheckConsumed();
    }

    [Fact]
    public void RunningOutOfValuesIsAnInvalidExample()
    {
        using var draws = new ExplicitDraws(Engines.Real, Example.Of(1));
        draws.Take("a_1");
        var e = Assert.Throws<InvalidExampleException>(() => draws.Take("b_1"));
        Assert.Equal("Explicit example Example(1) has no value for draw b_1: give it by name with Example.With, or add a positional value", e.Message);
    }

    [Fact]
    public void UnusedValuesAreReported()
    {
        using var draws = new ExplicitDraws(Engines.Real, Example.Of(1, 2, 3).With("x_1", 0).With("y_1", 0));
        draws.Take("a_1");
        draws.Take("x_1");
        var e = Assert.Throws<InvalidExampleException>(draws.CheckConsumed);
        Assert.Equal("Explicit example Example(1, 2, 3, x_1=0, y_1=0) was not fully used: no draw is named y_1; 2 positional value(s) were not drawn", e.Message);
    }

    [Fact]
    public void OnlyLeftoverPositionalValuesAreReported()
    {
        using var draws = new ExplicitDraws(Engines.Real, Example.Of(1));
        var e = Assert.Throws<InvalidExampleException>(draws.CheckConsumed);
        Assert.EndsWith("was not fully used: 1 positional value(s) were not drawn", e.Message);
    }

    [Fact]
    public void TargetAndEventsAreIgnored()
    {
        ExplicitDataSource.Instance.Target(1.0, "label");
        ExplicitDataSource.Instance.Event("label");
        ExplicitDataSource.Instance.EventValue("label", 2.0);
    }

    [Fact]
    public void IsAbortedIsUnsupported()
    {
        var e = Assert.Throws<InvalidExampleException>(() => ExplicitDataSource.Instance.IsAborted);
        Assert.Equal("IsAborted is not supported in explicit test cases", e.Message);
    }

    public static IEnumerable<object[]> UnsupportedMethods() =>
        typeof(ExplicitDataSource)
            .GetMethods(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly)
            .Where(m => !m.IsSpecialName && m.Name is not ("Target" or "Event" or "EventValue"))
            .Select(m => new object[] { m.Name });

    [Theory]
    [MemberData(nameof(UnsupportedMethods))]
    public void EveryOtherPrimitiveIsUnsupported(string name)
    {
        var method = typeof(ExplicitDataSource).GetMethod(name)!;
        var args = method.GetParameters().Select(p => p.ParameterType.IsValueType ? Activator.CreateInstance(p.ParameterType) : null).ToArray();
        var e = Assert.Throws<TargetInvocationException>(() => method.Invoke(ExplicitDataSource.Instance, args));
        var inner = Assert.IsType<InvalidExampleException>(e.InnerException);
        Assert.Equal($"{name} is not supported in explicit test cases", inner.Message);
    }
}
