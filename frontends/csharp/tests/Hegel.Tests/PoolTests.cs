using System.Collections.Concurrent;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class PoolTests
{
    private static Settings Quick => Engines.Quiet with { TestCases = 20 };

    [Fact]
    public void PoolsNeedATestCase()
    {
        Assert.Throws<ArgumentNullException>(() => new Pool<int>(null!));
        Assert.Throws<ArgumentNullException>(() => new ConcurrentPool<int>(null!));
        Property.Test(tc => Assert.Throws<ArgumentNullException>(() => new ConcurrentPool<int>(tc).Add(null!, 1)), Quick with { TestCases = 1 });
    }

    [Fact]
    public void ReusableDrawsKeepValuesAndConsumingDrawsRemoveThem()
    {
        var report = Property.Test(tc =>
        {
            var pool = new Pool<int>(tc);
            Assert.True(pool.IsEmpty);
            pool.Add(1);
            pool.Add(2);
            Assert.False(pool.IsEmpty);
            Assert.Equal(2, pool.Count);
            var a = tc.Draw(pool.Reusable(), "a");
            Assert.Contains(a, new[] { 1, 2 });
            Assert.Equal(2, pool.Count);
            var b = tc.Draw(pool.Consuming());
            var c = tc.Draw(pool.Consuming());
            Assert.True(pool.IsEmpty);
            Assert.Equal([1, 2], new[] { b, c }.Order());
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void ConcurrentPoolsBehaveTheSameOnOneThread()
    {
        var report = Property.Test(tc =>
        {
            var pool = new ConcurrentPool<string>(tc);
            Assert.True(pool.IsEmpty);
            pool.Add(tc, "x");
            pool.Add(tc, "y");
            Assert.False(pool.IsEmpty);
            Assert.Equal(2, pool.Count);
            var a = tc.Draw(pool.Reusable());
            Assert.Contains(a, new[] { "x", "y" });
            Assert.Equal(2, pool.Count);
            var b = tc.Draw(pool.Consuming().Filter(s => s == "y"));
            Assert.Equal("y", b);
            var c = tc.Draw(pool.Consuming());
            Assert.Equal("x", c);
            Assert.True(pool.IsEmpty);
            Assert.Throws<AssumeRejected>(() => tc.Draw(pool.Reusable()));
            Assert.Throws<AssumeRejected>(() => tc.Draw(pool.Consuming()));
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void DrawingFromAnEmptyPoolRejectsTheCase()
    {
        var report = Property.Test(tc =>
        {
            var pool = new Pool<int>(tc);
            Assert.Throws<AssumeRejected>(() => tc.Draw(pool.Reusable()));
            Assert.Throws<AssumeRejected>(() => tc.Draw(pool.Consuming()));
            Assert.Throws<AssumeRejected>(() => tc.Draw(pool.Consuming().Filter(_ => true)));
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void FilteredConsumingDrawsTakeOnlyMatchingValues()
    {
        var report = Property.Test(tc =>
        {
            var pool = new Pool<int>(tc);
            foreach (var i in Enumerable.Range(1, 6))
            {
                pool.Add(i);
            }
            var even = pool.Consuming().Filter(x => x % 2 == 0);
            var first = tc.Draw(even, "first");
            var second = tc.Draw(even, "second");
            var third = tc.Draw(even, "third");
            Assert.Equal([2, 4, 6], new[] { first, second, third }.Order());
            Assert.Equal(3, pool.Count);
            Assert.Throws<AssumeRejected>(() => tc.Draw(even));
            Assert.Equal(3, pool.Count);
            Assert.Equal([1, 3, 5], new[] { tc.Draw(pool.Consuming()), tc.Draw(pool.Consuming()), tc.Draw(pool.Consuming()) }.Order());
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void QueryWhereOnAConsumingPoolTakesOnlyMatchingValues()
    {
        var report = Property.Test(tc =>
        {
            var pool = new Pool<int>(tc);
            foreach (var i in Enumerable.Range(1, 4))
            {
                pool.Add(i);
            }
            var even = from x in pool.Consuming() where x % 2 == 0 select x;
            Assert.Equal([2, 4], new[] { tc.Draw(even), tc.Draw(even) }.Order());
            Assert.Equal(2, pool.Count);
            Assert.Throws<AssumeRejected>(() => tc.Draw(even));
        }, Quick);
        Assert.True(report.Passed);
    }

    private static Pool<int> Filled(TestCase tc, int count)
    {
        var pool = new Pool<int>(tc);
        foreach (var i in Enumerable.Range(1, count))
        {
            pool.Add(i);
        }
        return pool;
    }

    private static List<int> Drain(TestCase tc, Pool<int> pool)
    {
        var values = new List<int>();
        while (!pool.IsEmpty)
        {
            values.Add(tc.Draw(pool.Consuming()));
        }
        values.Sort();
        return values;
    }

    [Fact]
    public void AFilterAfterAMapPutsBackTheValuesItRejects()
    {
        var report = Property.Test(tc =>
        {
            var pool = Filled(tc, 4);
            var forty = pool.Consuming().Map(x => x * 10).Filter(v => v == 40);
            try
            {
                Assert.Equal(40, tc.Draw(forty));
                Assert.Equal([1, 2, 3], Drain(tc, pool));
            }
            catch (AssumeRejected)
            {
                Assert.Equal([1, 2, 3, 4], Drain(tc, pool));
            }
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void AQueryThatFiltersAfterALetPutsBackTheValuesItRejects()
    {
        var report = Property.Test(tc =>
        {
            var pool = Filled(tc, 4);
            var none = from x in pool.Consuming() let y = x * 10 where y > 40 select y;
            Assert.Throws<AssumeRejected>(() => tc.Draw(none));
            Assert.Equal([1, 2, 3, 4], Drain(tc, pool));
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void AnOuterRejectionPutsBackWhatAnInnerFilterAccepted()
    {
        var report = Property.Test(tc =>
        {
            var pool = Filled(tc, 4);
            var none = pool.Consuming().Filter(_ => true).Map(x => x).Filter(_ => true).Filter(_ => false);
            Assert.Throws<AssumeRejected>(() => tc.Draw(none));
            Assert.Equal([1, 2, 3, 4], Drain(tc, pool));
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void AUniqueListPutsBackTheDuplicatesItRejects()
    {
        var report = Property.Test(tc =>
        {
            var pool = Filled(tc, 4);
            var parities = tc.Draw(Generators.Lists(pool.Consuming().Map(x => x % 2)).MaxSize(2).Unique());
            Assert.Equal(4 - parities.Count, pool.Count);
            Assert.Equal(4 - parities.Count, Drain(tc, pool).Count);
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void APredicateThatThrowsLeavesThePoolWhole()
    {
        var report = Property.Test(tc =>
        {
            var pool = Filled(tc, 3);
            Assert.Throws<InvalidOperationException>(() => tc.Draw(pool.Consuming().Filter(_ => throw new InvalidOperationException("no"))));
            Assert.Equal([1, 2, 3], Drain(tc, pool));
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void AConcurrentPoolPutsBackWhatALaterFilterRejects()
    {
        var report = Property.Test(tc =>
        {
            var pool = new ConcurrentPool<int>(tc);
            pool.Add(tc, 1);
            pool.Add(tc, 2);
            Assert.Throws<AssumeRejected>(() => tc.Draw(pool.Consuming().Map(x => x).Filter(_ => false)));
            Assert.Equal(2, pool.Count);
            Assert.Equal([1, 2], new[] { tc.Draw(pool.Consuming()), tc.Draw(pool.Consuming()) }.Order());
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void FilteredConsumingDrawsGiveUpAfterAFewAttempts()
    {
        var report = Property.Test(tc =>
        {
            var pool = new Pool<int>(tc);
            foreach (var i in Enumerable.Range(0, 10))
            {
                pool.Add(i);
            }
            var tried = 0;
            Assert.Throws<AssumeRejected>(() => tc.Draw(pool.Consuming().Filter(_ =>
            {
                tried++;
                return false;
            })));
            Assert.Equal(FilteredGenerator<int>.Attempts, tried);
            Assert.Equal(10, pool.Count);
        }, Quick);
        Assert.True(report.Passed);
    }

    [Fact]
    public void PoolDrawsArePrintedAsTheirValue()
    {
        var report = Property.Run(tc =>
        {
            var pool = new Pool<string>(tc);
            pool.Add("seven");
            var a = tc.Draw(pool.Reusable(), "a");
            var b = tc.Draw(pool.Consuming().Filter(s => s.Length > 0), "b");
            throw new InvalidOperationException(a + b);
        }, Quick);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.Equal(["var a_1 = \"seven\";", "var b_1 = \"seven\";"], failure.Output);
        Assert.Equal("seven", failure.Draws["a_1"]);
    }

    [Fact]
    public void PoolGeneratorsCarryTheirLabels()
    {
        Property.Test(tc =>
        {
            var pool = new Pool<int>(tc);
            var shared = new ConcurrentPool<int>(tc);
            Assert.Equal(Label.PoolValuesReusable, pool.Reusable().Label);
            Assert.Equal(Label.PoolValuesConsumed, pool.Consuming().Label);
            Assert.Equal(Label.Combine(Label.Filter, Label.PoolValuesConsumed), pool.Consuming().Filter(_ => true).Label);
            Assert.Equal(Label.PoolValuesReusable, shared.Reusable().Label);
            Assert.Equal(Label.PoolValuesConsumed, shared.Consuming().Label);
            Assert.Equal(Label.Combine(Label.Filter, Label.PoolValuesConsumed), shared.Consuming().Filter(_ => true).Label);
        }, Quick with { TestCases = 1 });
    }

    private sealed class PoolMachine
    {
        private readonly HashSet<int> _live = [];
        private readonly Pool<int> _pool;
        private int _next;

        internal PoolMachine(TestCase tc)
        {
            _pool = new Pool<int>(tc);
        }

        [Rule]
        internal void Create(TestCase tc)
        {
            var v = _next++;
            _pool.Add(v);
            _live.Add(v);
        }

        [Rule]
        internal void Reuse(TestCase tc)
        {
            var v = tc.Draw(_pool.Reusable(), "v");
            Assert.Contains(v, _live);
        }

        [Rule]
        internal void Consume(TestCase tc)
        {
            var before = _pool.Count;
            var v = tc.Draw(_pool.Consuming(), "v");
            Assert.True(_live.Remove(v), $"consumed a value never created: {v}");
            Assert.Equal(before - 1, _pool.Count);
        }

        [Rule]
        internal void ConsumeEven(TestCase tc)
        {
            var v = tc.Draw(_pool.Consuming().Filter(x => x % 2 == 0), "v");
            Assert.Equal(0, v % 2);
            Assert.True(_live.Remove(v), $"consumed a value never created: {v}");
        }

        [Invariant]
        internal void Agree(TestCase tc) => Assert.Equal(_live.Count, _pool.Count);
    }

    [Fact]
    public void PoolsHandOutOnlyLiveValues()
    {
        var report = Property.Test(tc => Stateful.Run(new PoolMachine(tc), tc), Quick);
        Assert.True(report.Passed);
    }

    private sealed class LateMachine
    {
        private Pool<int>? _pool;

        [Rule]
        internal void Create(TestCase tc)
        {
            _pool ??= new Pool<int>(tc);
            _pool.Add(tc.Draw(Generators.Integers(), "n"));
        }

        [Rule]
        internal void Use(TestCase tc)
        {
            tc.Assume(_pool != null);
            tc.Draw(_pool!.Reusable(), "v");
        }
    }

    [Fact]
    public void APoolMadeInsideARuleOutlivesIt()
    {
        var report = Property.Test(tc => Stateful.Run(new LateMachine(), tc), Quick);
        Assert.True(report.Passed);
    }

    private sealed class SharedPoolMachine
    {
        private readonly ConcurrentDictionary<int, bool> _everLive = new();
        private readonly ConcurrentDictionary<int, bool> _live = new();
        private readonly ConcurrentPool<int> _pool;
        private int _next;

        internal SharedPoolMachine(TestCase tc)
        {
            _pool = new ConcurrentPool<int>(tc);
        }

        [Rule]
        internal void Create(TestCase tc)
        {
            var v = Interlocked.Increment(ref _next);
            _everLive[v] = true;
            _live[v] = true;
            _pool.Add(tc, v);
        }

        [Rule]
        internal void Reuse(TestCase tc)
        {
            var v = tc.Draw(_pool.Reusable(), "v");
            Assert.True(_everLive.ContainsKey(v));
        }

        [Rule]
        internal void Consume(TestCase tc)
        {
            var v = tc.Draw(_pool.Consuming(), "v");
            Assert.True(_live.TryRemove(v, out _), $"consumed a value never created: {v}");
        }

        [Rule]
        internal void ConsumeEven(TestCase tc)
        {
            var v = tc.Draw(_pool.Consuming().Filter(x => x % 2 == 0), "v");
            Assert.Equal(0, v % 2);
            Assert.True(_live.TryRemove(v, out _), $"consumed a value never created: {v}");
        }

        [Invariant]
        internal void Agree(TestCase tc)
        {
            Assert.Equal(_live.Count, _pool.Count);
            Assert.Equal(_live.IsEmpty, _pool.IsEmpty);
        }
    }

    [Fact]
    public void ConcurrentPoolsAreSharedBetweenWorkers()
    {
        var options = new StatefulOptions { MinConcurrency = 2, MaxConcurrency = 3, StepCount = 20 };
        var report = Property.Test(tc => Stateful.Run(new SharedPoolMachine(tc), tc, options), Quick);
        Assert.True(report.Passed);
    }
}
