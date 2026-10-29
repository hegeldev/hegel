namespace Hegel;

internal abstract class PoolGenerator<T> : PrintableGenerator<T>
{
    public abstract override T DoDraw(TestCase tc);

    public sealed override T DoDrawAndPrint(TestCase tc, Printer printer)
    {
        var value = DoDraw(tc);
        printer.Repr(value, typeof(T));
        return value;
    }
}

internal static class PoolDraws
{
    internal static T Draw<T>(TestCase tc, nint poolId, Dictionary<long, T> values, bool consume)
    {
        tc.Assume(values.Count > 0);
        var variableId = tc.PoolGenerate(poolId, consume);
        if (!consume)
        {
            return values[variableId];
        }
        values.Remove(variableId, out var value);
        return value!;
    }

    private static long Draw(TestCase tc, nint poolId)
    {
        tc.StartSpan(Label.PoolValuesConsumed);
        try
        {
            return tc.PoolGenerate(poolId, true);
        }
        finally
        {
            tc.StopSpan(false);
        }
    }

    internal static T DrawFiltered<T>(TestCase tc, nint poolId, Dictionary<long, T> values, Func<T, bool> predicate)
    {
        var rejected = new List<long>();
        try
        {
            for (var attempt = 0; attempt < FilteredGenerator<T>.Attempts && rejected.Count < values.Count; attempt++)
            {
                tc.StartSpan(Label.FilterAttempt);
                var discard = true;
                try
                {
                    var variableId = Draw(tc, poolId);
                    if (predicate(values[variableId]))
                    {
                        discard = false;
                        values.Remove(variableId, out var value);
                        return value!;
                    }
                    rejected.Add(variableId);
                }
                finally
                {
                    tc.StopSpan(discard);
                }
            }
        }
        finally
        {
            foreach (var variableId in rejected)
            {
                tc.PoolRestore(poolId, variableId);
            }
        }
        throw new AssumeRejected();
    }
}

/// <summary>
/// Values a stateful test has created, for later rules to draw from: the engine chooses which, and
/// shrinks towards fewer and earlier values. One pool belongs to one test case.
/// </summary>
/// <typeparam name="T">The type of value pooled.</typeparam>
public sealed class Pool<T>
{
    private readonly TestCase _tc;
    private readonly nint _poolId;
    private readonly Dictionary<long, T> _values = [];

    /// <summary>A pool for the test case <paramref name="tc"/>.</summary>
    public Pool(TestCase tc)
    {
        ArgumentNullException.ThrowIfNull(tc);
        _tc = tc;
        _poolId = tc.NewPool();
        tc.Pin();
    }

    /// <summary>Whether the pool holds no values.</summary>
    public bool IsEmpty => _values.Count == 0;

    /// <summary>How many values the pool holds.</summary>
    public int Count => _values.Count;

    /// <summary>Add <paramref name="value"/>.</summary>
    public void Add(T value) => _values[_tc.PoolAdd(_poolId)] = value;

    /// <summary>A generator drawing a value that stays in the pool. Rejects the test case while the pool is empty.</summary>
    public PrintableGenerator<T> Reusable() => new ReusableGenerator(this);

    /// <summary>A generator drawing a value and removing it from the pool. Rejects the test case while the pool is empty.</summary>
    public PrintableGenerator<T> Consuming() => new ConsumingGenerator(this);

    private sealed class ReusableGenerator(Pool<T> pool) : PoolGenerator<T>
    {
        public override ulong Label => Hegel.Label.PoolValuesReusable;

        public override T DoDraw(TestCase tc) => PoolDraws.Draw(tc, pool._poolId, pool._values, false);
    }

    private sealed class ConsumingGenerator(Pool<T> pool) : PoolGenerator<T>
    {
        public override ulong Label => Hegel.Label.PoolValuesConsumed;

        public override T DoDraw(TestCase tc) => PoolDraws.Draw(tc, pool._poolId, pool._values, true);

        public override PrintableGenerator<T> Filter(Func<T, bool> predicate) => new FilteredConsuming(pool, predicate);
    }

    private sealed class FilteredConsuming(Pool<T> pool, Func<T, bool> predicate) : PoolGenerator<T>
    {
        public override ulong Label => Hegel.Label.Combine(Hegel.Label.Filter, Hegel.Label.PoolValuesConsumed);

        public override T DoDraw(TestCase tc) => PoolDraws.DrawFiltered(tc, pool._poolId, pool._values, predicate);
    }
}

/// <summary>A <see cref="Pool{T}"/> shared by the workers of a concurrent stateful test.</summary>
/// <typeparam name="T">The type of value pooled.</typeparam>
public sealed class ConcurrentPool<T>
{
    private readonly nint _poolId;
    private readonly Dictionary<long, T> _values = [];
    private readonly object _gate = new();

    /// <summary>A pool for the test case <paramref name="tc"/>.</summary>
    public ConcurrentPool(TestCase tc)
    {
        ArgumentNullException.ThrowIfNull(tc);
        _poolId = tc.NewPool();
    }

    /// <summary>Whether the pool holds no values.</summary>
    public bool IsEmpty
    {
        get
        {
            lock (_gate)
            {
                return _values.Count == 0;
            }
        }
    }

    /// <summary>How many values the pool holds.</summary>
    public int Count
    {
        get
        {
            lock (_gate)
            {
                return _values.Count;
            }
        }
    }

    /// <summary>Add <paramref name="value"/> through the worker's test case <paramref name="tc"/>.</summary>
    public void Add(TestCase tc, T value)
    {
        ArgumentNullException.ThrowIfNull(tc);
        lock (_gate)
        {
            _values[tc.PoolAdd(_poolId)] = value;
        }
    }

    /// <summary>A generator drawing a value that stays in the pool.</summary>
    public PrintableGenerator<T> Reusable() => new ReusableGenerator(this);

    /// <summary>A generator drawing a value and removing it from the pool.</summary>
    public PrintableGenerator<T> Consuming() => new ConsumingGenerator(this);

    private sealed class ReusableGenerator(ConcurrentPool<T> pool) : PoolGenerator<T>
    {
        public override ulong Label => Hegel.Label.PoolValuesReusable;

        public override T DoDraw(TestCase tc)
        {
            lock (pool._gate)
            {
                return PoolDraws.Draw(tc, pool._poolId, pool._values, false);
            }
        }
    }

    private sealed class ConsumingGenerator(ConcurrentPool<T> pool) : PoolGenerator<T>
    {
        public override ulong Label => Hegel.Label.PoolValuesConsumed;

        public override T DoDraw(TestCase tc)
        {
            lock (pool._gate)
            {
                return PoolDraws.Draw(tc, pool._poolId, pool._values, true);
            }
        }

        public override PrintableGenerator<T> Filter(Func<T, bool> predicate) => new FilteredConsuming(pool, predicate);
    }

    private sealed class FilteredConsuming(ConcurrentPool<T> pool, Func<T, bool> predicate) : PoolGenerator<T>
    {
        public override ulong Label => Hegel.Label.Combine(Hegel.Label.Filter, Hegel.Label.PoolValuesConsumed);

        public override T DoDraw(TestCase tc)
        {
            lock (pool._gate)
            {
                return PoolDraws.DrawFiltered(tc, pool._poolId, pool._values, predicate);
            }
        }
    }
}
