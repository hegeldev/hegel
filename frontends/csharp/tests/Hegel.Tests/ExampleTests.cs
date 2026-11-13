using Xunit;

namespace Hegel.Tests;

public class ExampleTests
{
    [Fact]
    public void OfCopiesThePositionalValues()
    {
        var values = new object?[] { 1, "two" };
        var example = Example.Of(values);
        values[0] = 99;
        Assert.Equal([1, "two"], example.Positional);
        Assert.Empty(example.Named);
    }

    [Fact]
    public void WithAddsANamedValueWithoutChangingTheOriginal()
    {
        var original = Example.Of(1);
        var named = original.With("n", 2).With("m", null);
        Assert.Empty(original.Named);
        Assert.Equal(2, named.Named["n"]);
        Assert.Null(named.Named["m"]);
        Assert.Equal([1], named.Positional);
    }

    [Fact]
    public void WithReplacesAnExistingName()
    {
        var example = Example.Of().With("n", 1).With("n", 2);
        Assert.Equal(2, example.Named["n"]);
    }

    [Fact]
    public void ToStringRendersPositionalThenNamedValues()
    {
        Assert.Equal("Example()", Example.Of().ToString());
        Assert.Equal("Example(1, \"a\\n\", n=2.5)", Example.Of(1, "a\n").With("n", 2.5).ToString());
        Assert.Equal("Example(null)", Example.Of(new object?[] { null }).ToString());
    }
}
