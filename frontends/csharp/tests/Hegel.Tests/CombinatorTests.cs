using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class CombinatorTests
{
    [Fact]
    public void MapTransformsValuesAndCombinesLabels()
    {
        var g = Generators.Integers().Min(0).Max(3).Map(x => x * 2);
        Gen.All(g, v => v % 2 == 0 && v <= 6, 50);
        Assert.Equal(Label.Combine(Label.Mapped, Label.Of("hegel.integers")), g.Label);
        Assert.Equal(g.Label, g.Label);
        Assert.Equal("var x_1 = 0;", Gen.MinimalOutput(g));
    }

    [Fact]
    public void PrintAsCallPrintsTheSourceInsideTheCall()
    {
        var g = Generators.Integers().Min(1).Max(1).Map(x => x + 0.5).PrintAsCall("Half.Of");
        Assert.Equal("var x_1 = Half.Of(1);", Gen.MinimalOutput(g));
        Gen.All(g, v => v == 1.5, 5);
        Assert.Equal(Label.Combine(Label.Mapped, Label.Of("hegel.integers")), g.Label);
        Assert.Equal(1.5, Gen.Printed(g, 3)[0].Value);
    }

    [Fact]
    public void PrintAsCallValidatesItsArguments()
    {
        var mapped = Generators.Integers().Map(x => x);
        Assert.Contains("PrintAsCall requires a non-empty function name", Assert.Throws<ArgumentException>(() => mapped.PrintAsCall("")).Message);
        var raw = new RawIntegers().Map(x => x);
        var e = Assert.Throws<InvalidOperationException>(() => raw.PrintAsCall("F"));
        Assert.Contains("is not a PrintableGenerator", e.Message);
    }

    [Fact]
    public void FilterRetriesUntilThePredicateHolds()
    {
        var g = Generators.Integers().Min(0).Max(10).Filter(x => x % 2 == 1);
        Gen.All(g, v => v % 2 == 1, 100);
        Assert.Equal(Label.Combine(Label.Filter, Label.Of("hegel.integers")), g.Label);
        Assert.Equal("var x_1 = 1;", Gen.MinimalOutput(g));
        Assert.Equal(3, FilteredGenerator<int>.Attempts);
    }

    [Fact]
    public void FilterThatNeverHoldsRejectsTheTestCase()
    {
        var g = Generators.Integers().Filter(_ => false);
        var report = Property.Run(tc => tc.Draw(g), Engines.Quiet with { TestCases = 5, SuppressHealthCheck = HealthCheck.FilterTooMuch }, Reporter.Silent);
        Assert.NotEqual(RunStatus.Failed, report.Status);
        Assert.Equal(0, report.Statistics.Valid);
    }

    [Fact]
    public void FlatMapChoosesTheSecondGeneratorFromTheFirst()
    {
        var g = Generators.Integers().Min(1).Max(3).FlatMap(n => Generators.Lists(Generators.Just(n)).MinSize(n).MaxSize(n));
        Gen.All(g, xs => xs.Count >= 1 && xs.All(x => x == xs.Count), 50);
        Assert.Equal(Label.Combine(Label.FlatMap, Label.Of("hegel.integers")), g.Label);
        Assert.Equal("var x_1 = new List<int> {1};", Gen.MinimalOutput(g));
    }

    [Fact]
    public void PrintWithUsesTheGivenPrinter()
    {
        var g = Generators.Integers().Min(1).Max(1).PrintWith((v, p) => p.Text($"one({v})"));
        Assert.Equal("var x_1 = one(1);", Gen.MinimalOutput(g));
        Gen.All(g, v => v == 1, 5);
        Assert.Equal(Label.Of("hegel.integers"), g.Label);
        var raw = new RawIntegers().PrintAsRepr();
        Assert.Equal("var x_1 = 0;", Gen.MinimalOutput(raw));
        Assert.Equal(Label.Of(typeof(RawIntegers).FullName!), raw.Label);
    }

    [Fact]
    public void DeferredDelegatesOnceSet()
    {
        var d = Generators.Deferred<int>();
        Assert.Throws<ArgumentNullException>(() => d.Set(null!));
        Assert.Equal(Label.Deferred, d.Label);
        var unset = Assert.Throws<InvalidOperationException>(() => Property.Test(tc => tc.Draw(d), Gen.Quiet(3), Reporter.Silent));
        Assert.Equal("deferred generator has not been set", unset.Message);
        Assert.Throws<InvalidOperationException>(() => Property.Test(tc => tc.DrawSilent(d), Gen.Quiet(3), Reporter.Silent));
        d.Set(Generators.Just(7));
        var twice = Assert.Throws<InvalidOperationException>(() => d.Set(Generators.Just(8)));
        Assert.Equal("deferred generator has already been set", twice.Message);
        Gen.All(d, v => v == 7, 5);
        Assert.Equal("var x_1 = 7;", Gen.MinimalOutput(d));
    }

    [Fact]
    public void CompositeRunsTheBody()
    {
        var g = Generators.Composite(tc => tc.Draw(Generators.Integers().Min(0).Max(2)) + 10);
        Gen.All(g, v => v is >= 10 and <= 12, 50);
        Assert.Equal(Label.Composite, g.Label);
        Assert.Equal("var x_1 = 10;", Gen.MinimalOutput(g));
        var printing = Generators.Composite((tc, p) =>
        {
            var n = tc.DrawAndPrint(Generators.Integers().Min(0).Max(2), p);
            p.Text(" + 10");
            return n + 10;
        });
        Gen.All(printing, v => v is >= 10 and <= 12, 20);
        Assert.Equal(Label.Composite, printing.Label);
        Assert.Equal("var x_1 = 0 + 10;", Gen.MinimalOutput(printing));
    }

    [Fact]
    public void JustAndNullsAreConstant()
    {
        Gen.All(Generators.Just("k"), v => v == "k", 5);
        Gen.All(Generators.Nulls<string>(), v => v == null, 5);
        Assert.Equal(Label.Of("hegel.just"), Generators.Just(1).Label);
        Assert.Equal("var x_1 = \"k\";", Gen.MinimalOutput(Generators.Just("k")));
        Assert.Equal("var x_1 = null;", Gen.MinimalOutput(Generators.Nulls<string>()));
    }

    [Fact]
    public void SampledFromPicksFromTheValues()
    {
        Gen.All(Generators.SampledFrom(["a", "b", "c"]), v => v is "a" or "b" or "c", 50);
        Gen.All(Generators.SampledFrom((IEnumerable<int>)new List<int> { 4, 5 }), v => v is 4 or 5, 20);
        Assert.Equal(Label.SampledFrom, Generators.SampledFrom([1]).Label);
        Assert.Equal("var x_1 = \"a\";", Gen.MinimalOutput(Generators.SampledFrom(["a", "b"])));
        Assert.Throws<ArgumentNullException>(() => Generators.SampledFrom<int>((IEnumerable<int>)null!));
        var e = Assert.Throws<ArgumentException>(() => Generators.SampledFrom<int>([]));
        Assert.Contains("SampledFrom requires a non-empty collection", e.Message);
    }

    [Fact]
    public void OneOfPicksAGenerator()
    {
        var g = Generators.OneOf(Generators.Just(1), Generators.Integers().Min(10).Max(20));
        var values = Gen.Sample(g, 100);
        Assert.Contains(1, values);
        Assert.Contains(values, v => v >= 10);
        Assert.Equal(Label.Combine(Label.OneOf, Label.Of("hegel.just"), Label.Of("hegel.integers")), g.Label);
        Assert.Equal("var x_1 = 1;", Gen.MinimalOutput(g));
        var e = Assert.Throws<ArgumentException>(() => Generators.OneOf<int>());
        Assert.Contains("OneOf requires at least one generator", e.Message);
    }

    [Fact]
    public void OptionalAndNullableMayBeNull()
    {
        var optional = Generators.Optional(Generators.Just("v"));
        var values = Gen.Sample(optional, 100);
        Assert.Contains(null, values);
        Assert.Contains("v", values);
        Assert.Equal(Label.Combine(Label.Optional, Label.Of("hegel.just")), optional.Label);
        Assert.Equal("var x_1 = null;", Gen.MinimalOutput(optional, v => v == null));
        Assert.Equal("var x_1 = \"v\";", Gen.MinimalOutput(optional, v => v != null));
        var nullable = Generators.Nullable(Generators.Integers().Min(1).Max(1));
        var ints = Gen.Sample(nullable, 100);
        Assert.Contains(null, ints);
        Assert.Contains(1, ints);
        Assert.Equal("var x_1 = null;", Gen.MinimalOutput(nullable, v => v == null));
    }

    [Fact]
    public void TuplesDrawEachElement()
    {
        var one = Generators.Just(1);
        Assert.Equal("var x_1 = (1, 1);", Gen.MinimalOutput(Generators.Tuples(one, one)));
        Assert.Equal("var x_1 = (1, 1, 1);", Gen.MinimalOutput(Generators.Tuples(one, one, one)));
        Assert.Equal("var x_1 = (1, 1, 1, 1);", Gen.MinimalOutput(Generators.Tuples(one, one, one, one)));
        Gen.All(Generators.Tuples(one, one, one, one, one), t => t == (1, 1, 1, 1, 1), 3);
        Gen.All(Generators.Tuples(one, one, one, one, one, one), t => t == (1, 1, 1, 1, 1, 1), 3);
        Gen.All(Generators.Tuples(one, one, one, one, one, one, one), t => t == (1, 1, 1, 1, 1, 1, 1), 3);
        Gen.All(Generators.Tuples(one, one, one, one, one, one, one, one), t => t == (1, 1, 1, 1, 1, 1, 1, 1), 3);
        var pair = Generators.Tuples(Generators.Integers().Min(0).Max(1), Generators.Booleans());
        Gen.All(pair, t => t.Item1 is 0 or 1, 20);
        Assert.Equal(Label.Combine(Label.Tuple, Label.Of("hegel.integers"), Label.Of("hegel.booleans")), pair.Label);
    }

    [Fact]
    public void PermutationsReorderTheElements()
    {
        Gen.All(Generators.Permutations([1, 2, 3]), p => p.Count == 3 && p.OrderBy(x => x).SequenceEqual([1, 2, 3]), 50);
        Gen.All(Generators.Permutations((IEnumerable<string>)new List<string> { "a" }), p => p.SequenceEqual(["a"]), 5);
        Gen.All(Generators.Permutations<int>([]), p => p.Count == 0, 3);
        Assert.Equal(Label.Permutation, Generators.Permutations([1]).Label);
        Assert.Equal("var x_1 = new List<int> {1, 2};", Gen.MinimalOutput(Generators.Permutations([1, 2])));
    }

    [Fact]
    public void SubsequencesKeepTheOrder()
    {
        var all = Gen.Sample(Generators.Subsequences([1, 2, 3, 4]), 100);
        Assert.All(all, s => Assert.True(s.SequenceEqual(s.OrderBy(x => x)) && s.All(x => x is >= 1 and <= 4)));
        Assert.Contains(all, s => s.Count == 0);
        Assert.Contains(all, s => s.Count == 4);
        Gen.All(Generators.Subsequences((IEnumerable<int>)new List<int> { 1, 2, 3 }).MinSize(2).MaxSize(2), s => s.Count == 2, 50);
        Gen.All(Generators.Subsequences([1, 2, 3]).MaxSize(10), s => s.Count <= 3, 20);
        Assert.Equal(Label.Subsequence, Generators.Subsequences([1]).Label);
        Assert.Equal("var x_1 = new List<int> {};", Gen.MinimalOutput(Generators.Subsequences([1, 2])));
    }

    [Fact]
    public void SubsequencesRejectImpossibleSizes()
    {
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Subsequences([1, 2]).MinSize(3));
        Assert.Contains("Cannot generate a subsequence: MinSize 3 is larger than the 2 elements in the sequence", e.Message);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Subsequences([1, 2]).MaxSize(-1));
    }

    [Fact]
    public void SamplesDrawWithOrWithoutReplacement()
    {
        var withReplacement = Gen.Sample(Generators.Samples([1, 2]).MinSize(3).MaxSize(3), 50);
        Assert.All(withReplacement, s => Assert.Equal(3, s.Count));
        Gen.All(Generators.Samples((IEnumerable<int>)new List<int> { 1, 2, 3 }).WithoutReplacement(), s => s.Distinct().Count() == s.Count, 50);
        Gen.All(Generators.Samples([1, 2, 3]).WithoutReplacement().MaxSize(10).MinSize(1), s => s.Count is >= 1 and <= 3, 50);
        Gen.All(Generators.Samples([1, 2, 3]).WithoutReplacement().WithReplacement().MaxSize(1), s => s.Count <= 1, 20);
        Gen.All(Generators.Samples<int>([]), s => s.Count == 0, 5);
        Assert.Equal(Label.Sample, Generators.Samples([1]).Label);
        Assert.Equal("var x_1 = new List<int> {};", Gen.MinimalOutput(Generators.Samples([1, 2])));
    }

    [Fact]
    public void SamplesRejectImpossibleSizes()
    {
        Assert.Contains("Cannot generate a non-empty sample from an empty sequence", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Samples<int>([]).MinSize(1)).Message);
        Assert.Contains("Cannot generate a sample without replacement: MinSize 3 is larger than the 2 elements in the sequence", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Samples([1, 2]).WithoutReplacement().MinSize(3)).Message);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Samples([1, 2]).MaxSize(-1));
    }

    [Fact]
    public void ListsRespectSizesAndPrintAsCollectionExpressions()
    {
        var g = Generators.Lists(Generators.Integers().Min(0).Max(9)).MinSize(1).MaxSize(3);
        Gen.All(g, xs => xs.Count is >= 1 and <= 3 && xs.All(x => x is >= 0 and <= 9), 100);
        Assert.Equal(Label.Combine(Label.List, Label.Of("hegel.integers")), g.Label);
        Assert.Equal("var x_1 = new List<int> {0};", Gen.MinimalOutput(g));
        Assert.Equal("var x_1 = new List<int> {1, 1};", Gen.MinimalOutput(Generators.Lists(Generators.Just(1)).MinSize(2).MaxSize(2)));
        Assert.Equal("var x_1 = new List<int> {};", Gen.MinimalOutput(Generators.Lists(Generators.Integers())));
    }

    [Fact]
    public void UniqueListsRejectDuplicates()
    {
        var unique = Generators.Lists(Generators.Integers().Min(0).Max(2)).Unique().MinSize(3);
        Gen.All(unique, xs => xs.Count == 3 && xs.Distinct().Count() == 3, 50);
        Assert.Equal("var x_1 = new List<int> {0, 1, 2};", Gen.MinimalOutput(unique));
        var byKey = Generators.Lists(Generators.Integers().Min(0).Max(5)).UniqueBy(x => x % 3).MinSize(3);
        Gen.All(byKey, xs => xs.Count == 3 && xs.Select(x => x % 3).Distinct().Count() == 3, 50);
        var arrays = Generators.Lists(Generators.Arrays(Generators.Integers().Min(0).Max(1)).MinSize(1).MaxSize(1)).Unique().MinSize(2);
        Gen.All(arrays, xs => xs.Count == 2 && xs[0][0] != xs[1][0], 50);
        var nulls = Generators.Lists(Generators.Nulls<string>()).Unique().MaxSize(5);
        Gen.All(nulls, xs => xs.Count <= 1, 20);
    }

    [Fact]
    public void ContentKeyComparesStructurally()
    {
        var a = new ContentKey(new[] { 1, 2 });
        var b = new ContentKey(new[] { 1, 2 });
        Assert.True(a.Equals((object)b));
        Assert.False(a.Equals(new object()));
        Assert.False(a.Equals(null));
        Assert.Equal(a.GetHashCode(), b.GetHashCode());
        Assert.Equal(new ContentKey(null), new ContentKey(null));
        Assert.Equal(0, new ContentKey(null).GetHashCode());
        Assert.NotEqual(new ContentKey(1), new ContentKey(new[] { 1 }));
        Assert.Equal(new ContentKey((1, "a")), new ContentKey((1, "a")));
    }

    [Fact]
    public void ArraysWrapLists()
    {
        var g = Generators.Arrays(Generators.Integers().Min(0).Max(2)).MinSize(1).MaxSize(2);
        Gen.All(g, xs => xs.Length is 1 or 2, 50);
        Assert.Equal(Label.Combine(Label.Array, Label.Combine(Label.List, Label.Of("hegel.integers"))), g.Label);
        Assert.Equal("var x_1 = new int[] {0};", Gen.MinimalOutput(g));
        Assert.Equal("var x_1 = new string[] {\"a\", \"a\"};", Gen.MinimalOutput(Generators.Arrays(Generators.Just("a")).MinSize(2).MaxSize(2)));
        Gen.All(Generators.Arrays(Generators.Integers().Min(0).Max(1)).Unique().MinSize(2), xs => xs.Length == 2 && xs[0] != xs[1], 20);
        Gen.All(Generators.Arrays(Generators.Integers().Min(0).Max(3)).UniqueBy(x => x % 2).MinSize(2), xs => xs.Length == 2, 20);
    }

    [Fact]
    public void SetsHoldDistinctValues()
    {
        var g = Generators.Sets(Generators.Integers().Min(0).Max(2)).MinSize(2).MaxSize(3);
        Gen.All(g, s => s.Count is 2 or 3, 50);
        Assert.Equal(Label.Combine(Label.Set, Label.Of("hegel.integers")), g.Label);
        Assert.Equal("var x_1 = new HashSet<int> {0, 1};", Gen.MinimalOutput(g));
        Assert.Equal("var x_1 = new HashSet<int> {};", Gen.MinimalOutput(Generators.Sets(Generators.Integers())));
        Gen.All(Generators.Sets(Generators.Just(1)).MaxSize(4), s => s.Count <= 1, 20);
    }

    [Fact]
    public void MapsHoldDistinctKeys()
    {
        var g = Generators.Maps(Generators.Integers().Min(0).Max(1), Generators.Just("v")).MinSize(2).MaxSize(2);
        Gen.All(g, m => m.Count == 2 && m.Keys.OrderBy(k => k).SequenceEqual([0, 1]), 50);
        Assert.Equal(Label.Combine(Label.Map, Label.Of("hegel.integers"), Label.Of("hegel.just")), g.Label);
        Assert.Equal("var x_1 = new Dictionary<int, string> {[0] = \"v\", [1] = \"v\"};", Gen.MinimalOutput(g));
        Assert.Equal("var x_1 = new Dictionary<int, string> {};", Gen.MinimalOutput(Generators.Maps(Generators.Integers(), Generators.Just("v"))));
        Gen.All(Generators.Maps(Generators.Just(1), Generators.Just(1)).MaxSize(3), m => m.Count <= 1, 20);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Maps(Generators.Integers(), Generators.Integers()).MinSize(-1));
    }

    private static RecursiveGenerator<int> Trees() =>
        Generators.Recursive(Generators.Just(1), sub => Generators.Lists(sub).MinSize(2).MaxSize(2).Map(xs => xs.Sum()));

    [Fact]
    public void RecursiveBuildsNestedStructures()
    {
        var values = Gen.Sample(Trees(), 100);
        Assert.Contains(1, values);
        Assert.Contains(values, v => v > 1);
        Assert.All(values, v => Assert.True(v <= RecursiveGenerator<int>.DefaultMaxLeaves));
        Gen.All(Trees().MaxLeaves(3), v => v <= 3, 100);
        Gen.All(Trees().MaxDepth(0), v => v == 1, 20);
        Gen.All(Trees().MaxDepth(1), v => v is 1 or 2, 50);
        Assert.Equal(Label.Combine(Label.Recursive, Label.Of("hegel.just")), Trees().Label);
        Assert.Equal("var x_1 = 1;", Gen.MinimalOutput(Trees()));
        Assert.Equal("var x_1 = 2;", Gen.MinimalOutput(Trees(), v => v > 1));
        Assert.Equal("var x_1 = 1;", Gen.MinimalOutput(Trees().MaxLeaves(2)));
    }

    [Fact]
    public void RecursivePrintsThroughTheSubtrees()
    {
        var mapped = Generators.Recursive<object>(Generators.Just<object>(0), sub => Generators.Lists(sub).MinSize(1).MaxSize(2).Map(xs => (object)xs));
        Assert.Equal("var x_1 = new List<object> {0};", Gen.MinimalOutput(mapped, v => v is List<object>));
        var nested = Generators.Recursive<object>(Generators.Just<object>(0), sub => Generators.Composite((tc, p) => (object)tc.DrawAndPrint(Generators.Lists(sub).MinSize(1).MaxSize(1), p)));
        Assert.Equal("var x_1 = new List<object> {0};", Gen.MinimalOutput(nested, v => v is List<object>));
        Assert.Equal("var x_1 = new List<object> {new List<object> {0}};", Gen.MinimalOutput(nested, v => v is List<object> xs && xs[0] is List<object>));
        Assert.Contains(Gen.Printed(nested, 50), r => r.Text.StartsWith("new List<object> {new List<object> {", StringComparison.Ordinal));
    }

    [Fact]
    public void RecursiveRejectsBadBudgets()
    {
        Assert.Contains("Recursive: MaxDepth must be >= 0", Assert.Throws<ArgumentOutOfRangeException>(() => Trees().MaxDepth(-1)).Message);
        Assert.Contains("Recursive: MaxLeaves must be >= 1", Assert.Throws<ArgumentOutOfRangeException>(() => Trees().MaxLeaves(0)).Message);
    }

    [Fact]
    public void SizesHelpersValidate()
    {
        Assert.Equal(2UL, Sizes.Validate(2, Native.Abi.Unbounded, "x"));
        Assert.Equal(Native.Abi.MaxSizeDefault, Sizes.EngineMax(Native.Abi.Unbounded));
        Assert.Equal(5UL, Sizes.EngineMax(5));
        Assert.Equal(7UL, Sizes.Max(7, "x"));
    }
}
