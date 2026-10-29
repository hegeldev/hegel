using Hegel.Native;

namespace Hegel;

/// <summary>A generator whose values are another's, transformed by a function.</summary>
public sealed class MappedGenerator<TSource, TResult> : ValueGenerator<TResult>
{
    private readonly Generator<TSource> _source;
    private readonly Func<TSource, TResult> _f;
    private readonly CachedLabel _label = new();

    internal MappedGenerator(Generator<TSource> source, Func<TSource, TResult> f)
    {
        _source = source;
        _f = f;
    }

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.Mapped, _source.Label));

    /// <inheritdoc />
    public override TResult DoDraw(TestCase tc) => _f(tc.DrawSilent(_source));

    /// <summary>
    /// This generator, printing each value as a call of <paramref name="function"/> on the source's
    /// own printing of its value: <c>Point.FromPolar(1.5, 0.25)</c> rather than the result's repr.
    /// </summary>
    public PrintableGenerator<TResult> PrintAsCall(string function)
    {
        if (string.IsNullOrEmpty(function))
        {
            throw new ArgumentException("PrintAsCall requires a non-empty function name", nameof(function));
        }
        if (_source is not PrintableGenerator<TSource>)
        {
            throw new InvalidOperationException($"PrintAsCall prints the source's own printing of its value, but {_source.GetType().FullName} is not a PrintableGenerator");
        }
        return new CallPrinting(this, function + "(");
    }

    private sealed class CallPrinting : PrintableGenerator<TResult>
    {
        private readonly MappedGenerator<TSource, TResult> _outer;
        private readonly string _open;

        internal CallPrinting(MappedGenerator<TSource, TResult> outer, string open)
        {
            _outer = outer;
            _open = open;
        }

        public override ulong Label => _outer.Label;

        public override TResult DoDraw(TestCase tc) => _outer.DoDraw(tc);

        public override TResult DoDrawAndPrint(TestCase tc, Printer printer)
        {
            printer.BeginGroup(_open);
            var input = tc.DrawAndPrint(_outer._source, printer);
            printer.EndGroup(")");
            return _outer._f(input);
        }
    }
}

/// <summary>A generator that draws from another until a predicate accepts the value.</summary>
public sealed class FilteredGenerator<T> : PrintableGenerator<T>
{
    /// <summary>How many values are tried before the test case is rejected.</summary>
    public const int Attempts = Abi.FilterAttempts;

    private readonly Generator<T> _source;
    private readonly Func<T, bool> _predicate;
    private readonly CachedLabel _label = new();

    internal FilteredGenerator(Generator<T> source, Func<T, bool> predicate)
    {
        _source = source;
        _predicate = predicate;
    }

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.Filter, _source.Label));

    /// <inheritdoc />
    public override T DoDraw(TestCase tc) => Draw(tc, null);

    /// <inheritdoc />
    public override T DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private T Draw(TestCase tc, Printer? printer)
    {
        var output = Components.Out(printer);
        for (var attempt = 0; attempt < Attempts; attempt++)
        {
            tc.StartSpan(Hegel.Label.FilterAttempt);
            var discard = true;
            var speculation = output.Speculate();
            try
            {
                var value = Components.Draw(tc, _source, printer);
                if (_predicate(value))
                {
                    discard = false;
                    speculation.Commit();
                    return value;
                }
            }
            finally
            {
                try
                {
                    speculation.Dispose();
                }
                finally
                {
                    tc.StopSpan(discard);
                }
            }
        }
        throw new AssumeRejected();
    }
}

/// <summary>A generator that draws a value and then from the generator chosen for it.</summary>
public sealed class FlatMappedGenerator<TSource, TResult> : PrintableGenerator<TResult>
{
    private readonly Generator<TSource> _source;
    private readonly Func<TSource, Generator<TResult>> _f;
    private readonly CachedLabel _label = new();

    internal FlatMappedGenerator(Generator<TSource> source, Func<TSource, Generator<TResult>> f)
    {
        _source = source;
        _f = f;
    }

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.FlatMap, _source.Label));

    /// <inheritdoc />
    public override TResult DoDraw(TestCase tc) => Draw(tc, null);

    /// <inheritdoc />
    public override TResult DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private TResult Draw(TestCase tc, Printer? printer) => Components.Draw(tc, _f(tc.DrawSilent(_source)), printer);
}

/// <summary>A generator printing another's values with a function of your own.</summary>
public sealed class PrintedGenerator<T> : PrintableGenerator<T>
{
    private readonly Generator<T> _source;
    private readonly Action<T, Printer> _print;

    internal PrintedGenerator(Generator<T> source, Action<T, Printer> print)
    {
        _source = source;
        _print = print;
    }

    /// <inheritdoc />
    public override ulong Label => _source.Label;

    /// <inheritdoc />
    public override T DoDraw(TestCase tc) => _source.DoDraw(tc);

    /// <inheritdoc />
    public override T DoDrawAndPrint(TestCase tc, Printer printer)
    {
        var value = _source.DoDraw(tc);
        if (printer.ShouldPrint)
        {
            _print(value, printer);
        }
        return value;
    }
}

/// <summary>A generator defined after it is referred to, for recursive definitions.</summary>
public sealed class Deferred<T> : PrintableGenerator<T>
{
    private Generator<T>? _delegate;

    internal Deferred()
    {
    }

    /// <summary>Define the generator. Allowed once.</summary>
    public void Set(Generator<T> generator)
    {
        ArgumentNullException.ThrowIfNull(generator);
        if (Interlocked.CompareExchange(ref _delegate, generator, null) != null)
        {
            throw new InvalidOperationException("deferred generator has already been set");
        }
    }

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Deferred;

    /// <inheritdoc />
    public override T DoDraw(TestCase tc) => Components.Draw(tc, Delegate(), null);

    /// <inheritdoc />
    public override T DoDrawAndPrint(TestCase tc, Printer printer) => Components.Draw(tc, Delegate(), printer);

    private Generator<T> Delegate() => _delegate ?? throw new InvalidOperationException("deferred generator has not been set");
}

internal sealed class CompositeGenerator<T> : ValueGenerator<T>
{
    private readonly Func<TestCase, T> _body;

    internal CompositeGenerator(Func<TestCase, T> body)
    {
        _body = body;
    }

    public override ulong Label => Hegel.Label.Composite;

    public override T DoDraw(TestCase tc) => _body(tc);
}

internal sealed class PrintingCompositeGenerator<T> : PrintableGenerator<T>
{
    private readonly Func<TestCase, Printer, T> _body;

    internal PrintingCompositeGenerator(Func<TestCase, Printer, T> body)
    {
        _body = body;
    }

    public override ulong Label => Hegel.Label.Composite;

    public override T DoDrawAndPrint(TestCase tc, Printer printer) => _body(tc, printer);
}

internal sealed class ConstantGenerator<T> : ValueGenerator<T>
{
    private readonly T _value;

    internal ConstantGenerator(T value)
    {
        _value = value;
    }

    public override ulong Label => Hegel.Label.Of("hegel.just");

    public override T DoDraw(TestCase tc) => _value;
}

internal sealed class SampledFromGenerator<T> : ValueGenerator<T>
{
    private readonly T[] _values;

    internal SampledFromGenerator(IEnumerable<T> values)
    {
        _values = values.ToArray();
    }

    public override ulong Label => Hegel.Label.SampledFrom;

    public override T DoDraw(TestCase tc) => _values[tc.GenerateInteger(0, _values.Length - 1)];
}

internal sealed class OneOfGenerator<T> : PrintableGenerator<T>
{
    private readonly Generator<T>[] _options;
    private readonly CachedLabel _label = new();

    internal OneOfGenerator(Generator<T>[] options)
    {
        if (options.Length == 0)
        {
            throw new ArgumentException("OneOf requires at least one generator", nameof(options));
        }
        _options = (Generator<T>[])options.Clone();
    }

    public override ulong Label => _label.Get(() => CachedLabel.Combine(Hegel.Label.OneOf, _options));

    public override T DoDraw(TestCase tc) => Draw(tc, null);

    public override T DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private T Draw(TestCase tc, Printer? printer) =>
        Components.Draw(tc, _options[tc.GenerateInteger(0, _options.Length - 1)], printer);
}

internal sealed class OptionalGenerator<T, TResult> : PrintableGenerator<TResult>
{
    private readonly Generator<T> _inner;
    private readonly Func<T, TResult> _present;
    private readonly TResult _empty;
    private readonly CachedLabel _label = new();

    internal OptionalGenerator(Generator<T> inner, Func<T, TResult> present, TResult empty)
    {
        _inner = inner;
        _present = present;
        _empty = empty;
    }

    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.Optional, _inner.Label));

    public override TResult DoDraw(TestCase tc) => Draw(tc, null);

    public override TResult DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private TResult Draw(TestCase tc, Printer? printer)
    {
        if (tc.GenerateBoolean(0.5))
        {
            return _present(Components.Draw(tc, _inner, printer));
        }
        Components.Out(printer).Text("null");
        return _empty;
    }
}

internal sealed class TupleGenerator<T> : PrintableGenerator<T>
{
    private readonly IGenerator[] _elements;
    private readonly Func<object?[], T> _assemble;
    private readonly CachedLabel _label = new();

    internal TupleGenerator(IGenerator[] elements, Func<object?[], T> assemble)
    {
        if (elements.Any(element => element == null))
        {
            throw new ArgumentNullException(nameof(elements), "Tuples: every element generator must be non-null");
        }
        _elements = elements;
        _assemble = assemble;
    }

    public override ulong Label => _label.Get(() => CachedLabel.Combine(Hegel.Label.Tuple, _elements));

    public override T DoDraw(TestCase tc) => Draw(tc, null);

    public override T DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private T Draw(TestCase tc, Printer? printer)
    {
        var p = Components.Out(printer);
        var values = new object?[_elements.Length];
        p.BeginGroup("(");
        for (var i = 0; i < _elements.Length; i++)
        {
            if (i > 0)
            {
                p.Separator();
            }
            values[i] = _elements[i].Draw(tc, printer);
        }
        p.EndGroup(")");
        return _assemble(values);
    }
}

internal static class IndexSamples
{
    internal static List<int> Draw(TestCase tc, int n, ulong minSize, ulong maxSize)
    {
        var remaining = Enumerable.Range(0, n).ToList();
        var chosen = new List<int>();
        var id = tc.NewCollection(minSize, maxSize);
        try
        {
            while (remaining.Count > 0 && tc.CollectionMore(id))
            {
                var index = (int)tc.GenerateInteger(0, remaining.Count - 1);
                chosen.Add(remaining[index]);
                remaining.RemoveAt(index);
            }
        }
        finally
        {
            tc.CollectionFree(id);
        }
        return chosen;
    }
}

internal sealed class PermutationGenerator<T> : ValueGenerator<List<T>>
{
    private readonly T[] _elements;

    internal PermutationGenerator(IEnumerable<T> elements)
    {
        _elements = elements.ToArray();
    }

    public override ulong Label => Hegel.Label.Permutation;

    public override List<T> DoDraw(TestCase tc)
    {
        var n = (ulong)_elements.Length;
        return IndexSamples.Draw(tc, _elements.Length, n, n).Select(i => _elements[i]).ToList();
    }
}

/// <summary>Draws subsequences of a sequence: its elements in order, some left out.</summary>
public sealed class SubsequenceGenerator<T> : ValueGenerator<List<T>>
{
    private readonly T[] _elements;
    private readonly ulong _minSize;
    private readonly ulong _maxSize;

    internal SubsequenceGenerator(T[] elements, int minSize, ulong maxSize)
    {
        _minSize = Sizes.Validate(minSize, maxSize, "Subsequences");
        if (minSize > elements.Length)
        {
            throw new ArgumentOutOfRangeException(nameof(minSize), minSize, $"Cannot generate a subsequence: MinSize {minSize} is larger than the {elements.Length} elements in the sequence");
        }
        _elements = elements;
        _maxSize = maxSize;
    }

    /// <summary>At least <paramref name="minSize"/> elements.</summary>
    public SubsequenceGenerator<T> MinSize(int minSize) => new(_elements, minSize, _maxSize);

    /// <summary>At most <paramref name="maxSize"/> elements.</summary>
    public SubsequenceGenerator<T> MaxSize(int maxSize) => new(_elements, (int)_minSize, Sizes.Max(maxSize, "Subsequences"));

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Subsequence;

    /// <inheritdoc />
    public override List<T> DoDraw(TestCase tc)
    {
        var n = (ulong)_elements.Length;
        var max = _maxSize == Abi.Unbounded ? n : Math.Min(_maxSize, n);
        var indices = IndexSamples.Draw(tc, _elements.Length, _minSize, max);
        indices.Sort();
        return indices.Select(i => _elements[i]).ToList();
    }
}

/// <summary>Draws samples of a sequence: some of its elements in any order, with or without replacement.</summary>
public sealed class SampleGenerator<T> : ValueGenerator<List<T>>
{
    private readonly T[] _elements;
    private readonly ulong _minSize;
    private readonly ulong _maxSize;
    private readonly bool _replacement;

    internal SampleGenerator(T[] elements, int minSize, ulong maxSize, bool replacement)
    {
        _minSize = Sizes.Validate(minSize, maxSize, "Samples");
        var n = elements.Length;
        if (replacement)
        {
            if (n == 0 && minSize > 0)
            {
                throw new ArgumentOutOfRangeException(nameof(minSize), minSize, "Cannot generate a non-empty sample from an empty sequence");
            }
        }
        else if (minSize > n)
        {
            throw new ArgumentOutOfRangeException(nameof(minSize), minSize, $"Cannot generate a sample without replacement: MinSize {minSize} is larger than the {n} elements in the sequence");
        }
        _elements = elements;
        _maxSize = maxSize;
        _replacement = replacement;
    }

    /// <summary>At least <paramref name="minSize"/> elements.</summary>
    public SampleGenerator<T> MinSize(int minSize) => new(_elements, minSize, _maxSize, _replacement);

    /// <summary>At most <paramref name="maxSize"/> elements.</summary>
    public SampleGenerator<T> MaxSize(int maxSize) => new(_elements, (int)_minSize, Sizes.Max(maxSize, "Samples"), _replacement);

    /// <summary>Elements may repeat (the default).</summary>
    public SampleGenerator<T> WithReplacement() => new(_elements, (int)_minSize, _maxSize, true);

    /// <summary>Each element appears at most once.</summary>
    public SampleGenerator<T> WithoutReplacement() => new(_elements, (int)_minSize, _maxSize, false);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Sample;

    /// <inheritdoc />
    public override List<T> DoDraw(TestCase tc)
    {
        var n = _elements.Length;
        var output = new List<T>();
        if (_replacement)
        {
            var id = tc.NewCollection(_minSize, n == 0 ? 0 : _maxSize);
            try
            {
                while (tc.CollectionMore(id))
                {
                    output.Add(_elements[tc.GenerateInteger(0, n - 1)]);
                }
            }
            finally
            {
                tc.CollectionFree(id);
            }
        }
        else
        {
            var max = _maxSize == Abi.Unbounded ? (ulong)n : Math.Min(_maxSize, (ulong)n);
            output.AddRange(IndexSamples.Draw(tc, n, _minSize, max).Select(i => _elements[i]));
        }
        return output;
    }
}

internal sealed class ContentKey : IEquatable<ContentKey>
{
    private readonly object? _key;

    internal ContentKey(object? key)
    {
        _key = key;
    }

    public bool Equals(ContentKey? other) => other != null && DeepEquals(_key, other._key);

    public override bool Equals(object? obj) => Equals(obj as ContentKey);

    public override int GetHashCode() => DeepHash(_key);

    private static bool DeepEquals(object? a, object? b)
    {
        if (a is System.Collections.IStructuralEquatable sa && b is System.Collections.IStructuralEquatable)
        {
            return sa.Equals(b, System.Collections.StructuralComparisons.StructuralEqualityComparer);
        }
        return Equals(a, b);
    }

    private static int DeepHash(object? value) => value switch
    {
        null => 0,
        System.Collections.IStructuralEquatable s => s.GetHashCode(System.Collections.StructuralComparisons.StructuralEqualityComparer),
        _ => value.GetHashCode(),
    };
}

/// <summary>Draws lists of values from an element generator.</summary>
public sealed class ListGenerator<T> : PrintableGenerator<List<T>>
{
    private readonly Generator<T> _element;
    private readonly ulong _minSize;
    private readonly ulong _maxSize;
    private readonly Func<T, object?>? _uniqueKey;
    private readonly CachedLabel _label = new();

    internal ListGenerator(Generator<T> element, int minSize, ulong maxSize, Func<T, object?>? uniqueKey)
    {
        _minSize = Sizes.Validate(minSize, maxSize, "Lists");
        _element = element;
        _maxSize = maxSize;
        _uniqueKey = uniqueKey;
    }

    /// <summary>At least <paramref name="minSize"/> elements.</summary>
    public ListGenerator<T> MinSize(int minSize) => new(_element, minSize, _maxSize, _uniqueKey);

    /// <summary>At most <paramref name="maxSize"/> elements.</summary>
    public ListGenerator<T> MaxSize(int maxSize) => new(_element, (int)_minSize, Sizes.Max(maxSize, "Lists"), _uniqueKey);

    /// <summary>No two equal elements.</summary>
    public ListGenerator<T> Unique() => UniqueBy(value => value);

    /// <summary>No two elements with equal <paramref name="key"/>s.</summary>
    public ListGenerator<T> UniqueBy(Func<T, object?> key) => new(_element, (int)_minSize, _maxSize, key);

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.List, _element.Label));

    /// <inheritdoc />
    public override List<T> DoDraw(TestCase tc) => Draw(tc, null, $"new {Printer.TypeName(typeof(List<T>))} {{", "}");

    /// <inheritdoc />
    public override List<T> DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer, $"new {Printer.TypeName(typeof(List<T>))} {{", "}");

    internal List<T> Draw(TestCase tc, Printer? printer, string open, string close)
    {
        var p = Components.Out(printer);
        var id = tc.NewCollection(_minSize, _maxSize);
        try
        {
            var output = new List<T>();
            var keys = new HashSet<ContentKey>();
            p.BeginGroup(open);
            while (tc.CollectionMore(id))
            {
                using var speculation = p.Speculate();
                if (output.Count > 0)
                {
                    p.Separator();
                }
                var value = Components.Draw(tc, _element, printer);
                if (_uniqueKey != null && !keys.Add(new ContentKey(_uniqueKey(value))))
                {
                    speculation.Abort();
                    tc.CollectionReject(id, "duplicate element");
                }
                else
                {
                    speculation.Commit();
                    output.Add(value);
                }
            }
            p.EndGroup(close);
            return output;
        }
        finally
        {
            tc.CollectionFree(id);
        }
    }
}

/// <summary>Draws arrays of values from an element generator.</summary>
public sealed class ArrayGenerator<T> : PrintableGenerator<T[]>
{
    private readonly ListGenerator<T> _lists;
    private readonly CachedLabel _label = new();

    internal ArrayGenerator(ListGenerator<T> lists)
    {
        _lists = lists;
    }

    /// <summary>At least <paramref name="minSize"/> elements.</summary>
    public ArrayGenerator<T> MinSize(int minSize) => new(_lists.MinSize(minSize));

    /// <summary>At most <paramref name="maxSize"/> elements.</summary>
    public ArrayGenerator<T> MaxSize(int maxSize) => new(_lists.MaxSize(maxSize));

    /// <summary>No two equal elements.</summary>
    public ArrayGenerator<T> Unique() => new(_lists.Unique());

    /// <summary>No two elements with equal <paramref name="key"/>s.</summary>
    public ArrayGenerator<T> UniqueBy(Func<T, object?> key) => new(_lists.UniqueBy(key));

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.Array, _lists.Label));

    /// <inheritdoc />
    public override T[] DoDraw(TestCase tc) => Draw(tc, null);

    /// <inheritdoc />
    public override T[] DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private T[] Draw(TestCase tc, Printer? printer) =>
        _lists.Draw(tc, printer, $"new {Printer.TypeName(typeof(T))}[] {{", "}").ToArray();
}

/// <summary>Draws sets of distinct values from an element generator.</summary>
public sealed class SetGenerator<T> : PrintableGenerator<HashSet<T>>
{
    private readonly Generator<T> _element;
    private readonly ulong _minSize;
    private readonly ulong _maxSize;
    private readonly CachedLabel _label = new();

    internal SetGenerator(Generator<T> element, int minSize, ulong maxSize)
    {
        _minSize = Sizes.Validate(minSize, maxSize, "Sets");
        _element = element;
        _maxSize = maxSize;
    }

    /// <summary>At least <paramref name="minSize"/> elements.</summary>
    public SetGenerator<T> MinSize(int minSize) => new(_element, minSize, _maxSize);

    /// <summary>At most <paramref name="maxSize"/> elements.</summary>
    public SetGenerator<T> MaxSize(int maxSize) => new(_element, (int)_minSize, Sizes.Max(maxSize, "Sets"));

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.Set, _element.Label));

    /// <inheritdoc />
    public override HashSet<T> DoDraw(TestCase tc) => Draw(tc, null);

    /// <inheritdoc />
    public override HashSet<T> DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private HashSet<T> Draw(TestCase tc, Printer? printer)
    {
        var p = Components.Out(printer);
        var id = tc.NewCollection(_minSize, _maxSize);
        try
        {
            var output = new HashSet<T>();
            p.BeginGroup($"new {Printer.TypeName(typeof(HashSet<T>))} {{");
            while (tc.CollectionMore(id))
            {
                using var speculation = p.Speculate();
                if (output.Count > 0)
                {
                    p.Separator();
                }
                var value = Components.Draw(tc, _element, printer);
                if (output.Add(value))
                {
                    speculation.Commit();
                }
                else
                {
                    speculation.Abort();
                    tc.CollectionReject(id, "duplicate element");
                }
            }
            p.EndGroup("}");
            return output;
        }
        finally
        {
            tc.CollectionFree(id);
        }
    }
}

/// <summary>Draws dictionaries from key and value generators.</summary>
public sealed class MapGenerator<TKey, TValue> : PrintableGenerator<Dictionary<TKey, TValue>>
    where TKey : notnull
{
    private readonly Generator<TKey> _keys;
    private readonly Generator<TValue> _values;
    private readonly ulong _minSize;
    private readonly ulong _maxSize;
    private readonly CachedLabel _label = new();

    internal MapGenerator(Generator<TKey> keys, Generator<TValue> values, int minSize, ulong maxSize)
    {
        _minSize = Sizes.Validate(minSize, maxSize, "Maps");
        _keys = keys;
        _values = values;
        _maxSize = maxSize;
    }

    /// <summary>At least <paramref name="minSize"/> entries.</summary>
    public MapGenerator<TKey, TValue> MinSize(int minSize) => new(_keys, _values, minSize, _maxSize);

    /// <summary>At most <paramref name="maxSize"/> entries.</summary>
    public MapGenerator<TKey, TValue> MaxSize(int maxSize) => new(_keys, _values, (int)_minSize, Sizes.Max(maxSize, "Maps"));

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.Map, _keys.Label, _values.Label));

    /// <inheritdoc />
    public override Dictionary<TKey, TValue> DoDraw(TestCase tc) => Draw(tc, null);

    /// <inheritdoc />
    public override Dictionary<TKey, TValue> DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private Dictionary<TKey, TValue> Draw(TestCase tc, Printer? printer)
    {
        var p = Components.Out(printer);
        var id = tc.NewCollection(_minSize, _maxSize);
        try
        {
            var output = new Dictionary<TKey, TValue>();
            p.BeginGroup($"new {Printer.TypeName(typeof(Dictionary<TKey, TValue>))} {{");
            while (tc.CollectionMore(id))
            {
                using var speculation = p.Speculate();
                if (output.Count > 0)
                {
                    p.Separator();
                }
                p.Text("[");
                var key = Components.Draw(tc, _keys, printer);
                if (output.ContainsKey(key))
                {
                    speculation.Abort();
                    tc.CollectionReject(id, "duplicate key");
                }
                else
                {
                    p.Text("] = ");
                    output[key] = Components.Draw(tc, _values, printer);
                    speculation.Commit();
                }
            }
            p.EndGroup("}");
            return output;
        }
        finally
        {
            tc.CollectionFree(id);
        }
    }
}

/// <summary>Draws recursive structures: a base case, extended by a function a bounded number of times.</summary>
public sealed class RecursiveGenerator<T> : PrintableGenerator<T>
{
    /// <summary>The depth a recursive draw may reach unless <see cref="MaxDepth"/> says otherwise.</summary>
    public const int DefaultMaxDepth = 32;

    /// <summary>The number of leaves a recursive draw may have unless <see cref="MaxLeaves"/> says otherwise.</summary>
    public const int DefaultMaxLeaves = 100;

    private readonly Generator<T> _base;
    private readonly Func<Generator<T>, Generator<T>> _extend;
    private readonly int _maxDepth;
    private readonly int _maxLeaves;
    private readonly CachedLabel _label = new();

    internal RecursiveGenerator(Generator<T> @base, Func<Generator<T>, Generator<T>> extend, int maxDepth, int maxLeaves)
    {
        if (maxDepth < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(maxDepth), maxDepth, "Recursive: MaxDepth must be >= 0");
        }
        if (maxLeaves < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maxLeaves), maxLeaves, "Recursive: MaxLeaves must be >= 1");
        }
        _base = @base;
        _extend = extend;
        _maxDepth = maxDepth;
        _maxLeaves = maxLeaves;
    }

    /// <summary>Recurse at most <paramref name="maxDepth"/> levels.</summary>
    public RecursiveGenerator<T> MaxDepth(int maxDepth) => new(_base, _extend, maxDepth, _maxLeaves);

    /// <summary>Draw at most <paramref name="maxLeaves"/> base cases.</summary>
    public RecursiveGenerator<T> MaxLeaves(int maxLeaves) => new(_base, _extend, _maxDepth, maxLeaves);

    private sealed class OverBudget : Exception
    {
    }

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.Recursive, _base.Label));

    /// <inheritdoc />
    public override T DoDraw(TestCase tc) => Draw(tc, null);

    /// <inheritdoc />
    public override T DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private T Draw(TestCase tc, Printer? printer)
    {
        var p = Components.Out(printer);
        var recursion = tc.NewRecursion((ulong)_maxDepth, (ulong)_maxLeaves);
        try
        {
            while (true)
            {
                using var speculation = p.Speculate();
                T value;
                try
                {
                    value = DrawSubtree(tc, recursion, 0, printer);
                }
                catch (OverBudget)
                {
                    tc.RecursionRetry(recursion);
                    continue;
                }
                if (tc.RecursionFinish(recursion))
                {
                    speculation.Commit();
                    return value;
                }
            }
        }
        finally
        {
            tc.RecursionFree(recursion);
        }
    }

    private T DrawSubtree(TestCase tc, nint recursion, ulong depth, Printer? printer)
    {
        if (tc.RecursionBranch(recursion, depth))
        {
            return Components.Draw(tc, _extend(new Subtrees(this, recursion, depth + 1)), printer);
        }
        if (!tc.RecursionLeaf(recursion))
        {
            throw new OverBudget();
        }
        return Components.Draw(tc, _base, printer);
    }

    private sealed class Subtrees : PrintableGenerator<T>
    {
        private readonly RecursiveGenerator<T> _outer;
        private readonly nint _recursion;
        private readonly ulong _depth;

        internal Subtrees(RecursiveGenerator<T> outer, nint recursion, ulong depth)
        {
            _outer = outer;
            _recursion = recursion;
            _depth = depth;
        }

        public override ulong Label => _outer.Label;

        public override T DoDraw(TestCase inner) => _outer.DrawSubtree(inner, _recursion, _depth, null);

        public override T DoDrawAndPrint(TestCase inner, Printer innerPrinter) => _outer.DrawSubtree(inner, _recursion, _depth, innerPrinter);
    }
}
