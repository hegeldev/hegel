using Xunit;

namespace Hegel.Tests;

public class LabelTests
{
    [Fact]
    public void OfIsDeterministicAndDependsOnTheName()
    {
        Assert.Equal(Label.Of("hegel.integers"), Label.Of("hegel.integers"));
        Assert.NotEqual(Label.Of("hegel.integers"), Label.Of("hegel.text"));
        Assert.NotEqual(0UL, Label.Of(""));
    }

    [Fact]
    public void CombineMixesItsArgumentsInOrder()
    {
        var a = Label.Of("a");
        var b = Label.Of("b");
        Assert.Equal(Label.Combine(a, b), Label.Combine(a, b));
        Assert.NotEqual(Label.Combine(a, b), Label.Combine(b, a));
        Assert.NotEqual(a, Label.Combine(a));
        Assert.NotEqual(Label.Combine(a), Label.Combine(a, a));
    }

    [Fact]
    public void ConstantsAreDistinct()
    {
        ulong[] constants =
        [
            Label.List, Label.Set, Label.Map, Label.Tuple, Label.OneOf, Label.Optional, Label.FixedDict, Label.FlatMap,
            Label.Filter, Label.FilterAttempt, Label.Mapped, Label.SampledFrom, Label.StatefulRule, Label.Recursive,
            Label.Permutation, Label.Subsequence, Label.Sample, Label.Array, Label.Composite, Label.Deferred, Label.FromLark,
            Label.PoolValuesReusable, Label.PoolValuesConsumed, Label.Generator,
        ];
        Assert.Equal(constants.Length, constants.Distinct().Count());
        Assert.DoesNotContain(0UL, constants);
    }
}
