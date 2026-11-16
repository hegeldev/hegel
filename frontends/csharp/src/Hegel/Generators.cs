using System.Numerics;
using Hegel.Native;

namespace Hegel;

/// <summary>
/// The built-in generators. Each returns a generator you can narrow with its methods
/// (<c>Generators.Integers().Min(0)</c>, <c>Generators.Lists(g).MaxSize(5)</c>) or combine with
/// <see cref="Generator{T}.Map{TResult}"/>, <see cref="Generator{T}.Filter"/> and
/// <see cref="Generator{T}.FlatMap{TResult}"/>.
/// </summary>
public static class Generators
{
    /// <summary>Any <see cref="int"/>.</summary>
    public static IntegerGenerator Integers() => new(int.MinValue, int.MaxValue);

    /// <summary>Any <see cref="long"/>.</summary>
    public static LongGenerator Longs() => new(long.MinValue, long.MaxValue);

    /// <summary>Any <see cref="BigInteger"/> (around ±2^127 until bounded).</summary>
    public static BigIntegerGenerator BigIntegers() => new(null, null);

    /// <summary>Any <see cref="float"/>, NaN and infinities included until bounded.</summary>
    public static FloatGenerator Floats() => new(null, null, null, null, null, false, false);

    /// <summary>Any <see cref="double"/>, NaN and infinities included until bounded.</summary>
    public static DoubleGenerator Doubles() => new(null, null, null, null, null, false, false);

    /// <summary>true or false.</summary>
    public static PrintableGenerator<bool> Booleans() => new BooleanGenerator(0.5);

    /// <summary>true with probability <paramref name="p"/>.</summary>
    public static PrintableGenerator<bool> WeightedBooleans(double p) => new BooleanGenerator(p);

    /// <summary>Any Unicode text.</summary>
    public static TextGenerator Text() => new(0, Abi.Unbounded);

    /// <summary>A single Unicode code point, as a string (a code point may not fit a <see cref="char"/>).</summary>
    public static TextGenerator Characters() => Text().MinSize(1).MaxSize(1);

    /// <summary>Any byte array.</summary>
    public static BinaryGenerator Binary() => new(0, Abi.Unbounded);

    /// <summary>Always <paramref name="value"/>.</summary>
    public static PrintableGenerator<T> Just<T>(T value) => new ConstantGenerator<T>(value);

    /// <summary>Always null.</summary>
    public static PrintableGenerator<T?> Nulls<T>() => new ConstantGenerator<T?>(default);

    /// <summary>One of <paramref name="values"/>.</summary>
    public static PrintableGenerator<T> SampledFrom<T>(IEnumerable<T> values)
    {
        ArgumentNullException.ThrowIfNull(values);
        var array = values.ToArray();
        if (array.Length == 0)
        {
            throw new ArgumentException("SampledFrom requires a non-empty collection", nameof(values));
        }
        return new SampledFromGenerator<T>(array);
    }

    /// <summary>A value from one of <paramref name="options"/>.</summary>
    public static PrintableGenerator<T> OneOf<T>(params Generator<T>[] options)
    {
        ArgumentNullException.ThrowIfNull(options);
        return new OneOfGenerator<T>(options);
    }

    /// <summary>A value from <paramref name="generator"/>, or null.</summary>
    public static PrintableGenerator<T?> Optional<T>(Generator<T> generator) where T : class
    {
        ArgumentNullException.ThrowIfNull(generator);
        return new OptionalGenerator<T, T?>(generator, value => value, null);
    }

    /// <summary>A value from <paramref name="generator"/>, or null.</summary>
    public static PrintableGenerator<T?> Nullable<T>(Generator<T> generator) where T : struct
    {
        ArgumentNullException.ThrowIfNull(generator);
        return new OptionalGenerator<T, T?>(generator, value => value, null);
    }

    /// <summary>Lists of values from <paramref name="element"/>.</summary>
    public static ListGenerator<T> Lists<T>(Generator<T> element)
    {
        ArgumentNullException.ThrowIfNull(element);
        return new(element, 0, Abi.Unbounded, null);
    }

    /// <summary>Arrays of values from <paramref name="element"/>.</summary>
    public static ArrayGenerator<T> Arrays<T>(Generator<T> element)
    {
        ArgumentNullException.ThrowIfNull(element);
        return new(Lists(element));
    }

    /// <summary>Sets of distinct values from <paramref name="element"/>.</summary>
    public static SetGenerator<T> Sets<T>(Generator<T> element)
    {
        ArgumentNullException.ThrowIfNull(element);
        return new(element, 0, Abi.Unbounded);
    }

    /// <summary>Dictionaries with keys from <paramref name="keys"/> and values from <paramref name="values"/>.</summary>
    public static MapGenerator<TKey, TValue> Maps<TKey, TValue>(Generator<TKey> keys, Generator<TValue> values) where TKey : notnull
    {
        ArgumentNullException.ThrowIfNull(keys);
        ArgumentNullException.ThrowIfNull(values);
        return new(keys, values, 0, Abi.Unbounded);
    }

    /// <summary>Reorderings of <paramref name="elements"/>.</summary>
    public static PrintableGenerator<List<T>> Permutations<T>(IEnumerable<T> elements)
    {
        ArgumentNullException.ThrowIfNull(elements);
        return new PermutationGenerator<T>(elements);
    }

    /// <summary>Subsequences of <paramref name="elements"/>: some of them, in order.</summary>
    public static SubsequenceGenerator<T> Subsequences<T>(IEnumerable<T> elements)
    {
        ArgumentNullException.ThrowIfNull(elements);
        return new(elements.ToArray(), 0, Abi.Unbounded);
    }

    /// <summary>Samples of <paramref name="elements"/>: some of them in any order, with replacement unless told otherwise.</summary>
    public static SampleGenerator<T> Samples<T>(IEnumerable<T> elements)
    {
        ArgumentNullException.ThrowIfNull(elements);
        return new(elements.ToArray(), 0, Abi.Unbounded, true);
    }

    /// <summary>Pairs drawn from <paramref name="a"/> and <paramref name="b"/>.</summary>
    public static PrintableGenerator<(T1, T2)> Tuples<T1, T2>(Generator<T1> a, Generator<T2> b) =>
        new TupleGenerator<(T1, T2)>([a, b], v => ((T1)v[0]!, (T2)v[1]!));

    /// <summary>Triples drawn from the generators.</summary>
    public static PrintableGenerator<(T1, T2, T3)> Tuples<T1, T2, T3>(Generator<T1> a, Generator<T2> b, Generator<T3> c) =>
        new TupleGenerator<(T1, T2, T3)>([a, b, c], v => ((T1)v[0]!, (T2)v[1]!, (T3)v[2]!));

    /// <summary>Quadruples drawn from the generators.</summary>
    public static PrintableGenerator<(T1, T2, T3, T4)> Tuples<T1, T2, T3, T4>(Generator<T1> a, Generator<T2> b, Generator<T3> c, Generator<T4> d) =>
        new TupleGenerator<(T1, T2, T3, T4)>([a, b, c, d], v => ((T1)v[0]!, (T2)v[1]!, (T3)v[2]!, (T4)v[3]!));

    /// <summary>Quintuples drawn from the generators.</summary>
    public static PrintableGenerator<(T1, T2, T3, T4, T5)> Tuples<T1, T2, T3, T4, T5>(Generator<T1> a, Generator<T2> b, Generator<T3> c, Generator<T4> d, Generator<T5> e) =>
        new TupleGenerator<(T1, T2, T3, T4, T5)>([a, b, c, d, e], v => ((T1)v[0]!, (T2)v[1]!, (T3)v[2]!, (T4)v[3]!, (T5)v[4]!));

    /// <summary>Sextuples drawn from the generators.</summary>
    public static PrintableGenerator<(T1, T2, T3, T4, T5, T6)> Tuples<T1, T2, T3, T4, T5, T6>(Generator<T1> a, Generator<T2> b, Generator<T3> c, Generator<T4> d, Generator<T5> e, Generator<T6> f) =>
        new TupleGenerator<(T1, T2, T3, T4, T5, T6)>([a, b, c, d, e, f], v => ((T1)v[0]!, (T2)v[1]!, (T3)v[2]!, (T4)v[3]!, (T5)v[4]!, (T6)v[5]!));

    /// <summary>Septuples drawn from the generators.</summary>
    public static PrintableGenerator<(T1, T2, T3, T4, T5, T6, T7)> Tuples<T1, T2, T3, T4, T5, T6, T7>(Generator<T1> a, Generator<T2> b, Generator<T3> c, Generator<T4> d, Generator<T5> e, Generator<T6> f, Generator<T7> g) =>
        new TupleGenerator<(T1, T2, T3, T4, T5, T6, T7)>([a, b, c, d, e, f, g], v => ((T1)v[0]!, (T2)v[1]!, (T3)v[2]!, (T4)v[3]!, (T5)v[4]!, (T6)v[5]!, (T7)v[6]!));

    /// <summary>Octuples drawn from the generators.</summary>
    public static PrintableGenerator<(T1, T2, T3, T4, T5, T6, T7, T8)> Tuples<T1, T2, T3, T4, T5, T6, T7, T8>(Generator<T1> a, Generator<T2> b, Generator<T3> c, Generator<T4> d, Generator<T5> e, Generator<T6> f, Generator<T7> g, Generator<T8> h) =>
        new TupleGenerator<(T1, T2, T3, T4, T5, T6, T7, T8)>([a, b, c, d, e, f, g, h], v => ((T1)v[0]!, (T2)v[1]!, (T3)v[2]!, (T4)v[3]!, (T5)v[4]!, (T6)v[5]!, (T7)v[6]!, (T8)v[7]!));

    /// <summary>A generator written as a function of the test case, printed with <see cref="Printer.Repr(object?)"/>.</summary>
    public static PrintableGenerator<T> Composite<T>(Func<TestCase, T> body)
    {
        ArgumentNullException.ThrowIfNull(body);
        return new CompositeGenerator<T>(body);
    }

    /// <summary>A generator written as a function of the test case and the printer it prints to.</summary>
    public static PrintableGenerator<T> Composite<T>(Func<TestCase, Printer, T> body)
    {
        ArgumentNullException.ThrowIfNull(body);
        return new PrintingCompositeGenerator<T>(body);
    }

    /// <summary>A generator defined later with <see cref="Deferred{T}.Set"/>, for recursive definitions.</summary>
    public static Deferred<T> Deferred<T>() => new();

    /// <summary>Recursive structures: <paramref name="base"/> cases, extended by <paramref name="extend"/>.</summary>
    public static RecursiveGenerator<T> Recursive<T>(Generator<T> @base, Func<Generator<T>, Generator<T>> extend)
    {
        ArgumentNullException.ThrowIfNull(@base);
        ArgumentNullException.ThrowIfNull(extend);
        return new(@base, extend, RecursiveGenerator<T>.DefaultMaxDepth, RecursiveGenerator<T>.DefaultMaxLeaves);
    }

    /// <summary>A generator derived from <typeparamref name="T"/>: a built-in type, an enum, a collection, a tuple or a record of them.</summary>
    public static PrintableGenerator<T> ForType<T>() => Derive.ForType<T>();

    /// <summary>A generator for the record <typeparamref name="T"/>, each component derived from its type unless <see cref="RecordGenerator{T}.With{TComponent}"/> says otherwise.</summary>
    public static RecordGenerator<T> Records<T>() => new(new Dictionary<string, IGenerator>());

    /// <summary>Email addresses.</summary>
    public static PrintableGenerator<string> Emails() => new EmailGenerator();

    /// <summary>URLs.</summary>
    public static PrintableGenerator<string> Urls() => new UrlGenerator();

    /// <summary>Domain names.</summary>
    public static DomainGenerator Domains() => new(255);

    /// <summary>IP addresses, as text.</summary>
    public static IpAddressGenerator IpAddresses() => new(null);

    /// <summary>UUIDs.</summary>
    public static UuidGenerator Uuids() => new(null);

    /// <summary>Dates, of any year from 1 to 9999 unless bounded.</summary>
    /// <remarks>
    /// The temporal generators check their settings with the engine as each is given. A call that
    /// completes a conflict among the settings given so far throws an <see cref="ArgumentException"/>
    /// with the engine's message, whatever order they came in: <c>Min(a).Max(b)</c> and
    /// <c>Max(b).Min(a)</c> both throw at the second call when <c>a</c> is after <c>b</c>. A setting
    /// not yet given counts as unrestricted, not as its default, so a conflict with a default that
    /// a later call could still change is left to the first draw, which reports it as a
    /// <see cref="HegelUsageException"/>.
    /// </remarks>
    public static DateGenerator Dates() => new();

    /// <summary>Times of day, to the tick unless <see cref="TimeGenerator.Precision"/> says otherwise.</summary>
    /// <remarks>Bad arguments are reported as <see cref="Dates"/> describes.</remarks>
    public static TimeGenerator Times() => new();

    /// <summary>Date-times without a time zone.</summary>
    /// <remarks>Bad arguments are reported as <see cref="Dates"/> describes.</remarks>
    public static DateTimeGenerator DateTimes() => new();

    /// <summary>Instants: <see cref="DateTimeOffset"/>s with a zero offset, anywhere in that type's range unless bounded.</summary>
    /// <remarks>Bad arguments are reported as <see cref="Dates"/> describes.</remarks>
    public static InstantGenerator Instants() => new();

    /// <summary>UTC offsets: whole minutes between -14:00 and +14:00 unless the generator is told otherwise.</summary>
    /// <remarks>
    /// Bad arguments are reported as <see cref="Dates"/> describes. A bound beyond the default
    /// range on its own (<c>Offsets().Min(TimeSpan.FromHours(15))</c>) conflicts only with the
    /// other bound's default, so it is the first draw that reports it.
    /// </remarks>
    public static OffsetGenerator Offsets() => new();

    /// <summary>Exact lengths of time of either sign, anywhere in <see cref="TimeSpan"/>'s range unless bounded.</summary>
    /// <remarks>Bad arguments are reported as <see cref="Dates"/> describes.</remarks>
    public static DurationGenerator Durations() => new();

    /// <summary>Calendar spans: a count of each unit from years to nanoseconds, all of one sign.</summary>
    /// <remarks>
    /// Bad arguments are reported as <see cref="Dates"/> describes. Unit bounds that conflict with
    /// the default sign policy are reported by the first draw, since <see cref="SpanGenerator.Sign"/>
    /// could still change it.
    /// </remarks>
    public static SpanGenerator Spans() => new();

    /// <summary>
    /// Time zones: those of <see cref="ZoneSource.Default"/>, as it is now, that this system has a
    /// <see cref="TimeZoneInfo"/> for.
    /// </summary>
    /// <remarks>
    /// Bad arguments are reported as <see cref="Dates"/> describes. A name the zone source lacks
    /// is reported by a builder method only once <see cref="ZoneGenerator.Source"/> has named the
    /// source, and otherwise by the first draw, since the source could still be changed. That
    /// <see cref="ZoneGenerator.AllowLinks"/> and <see cref="ZoneGenerator.AllowLegacy"/> leave
    /// none of the source's zones to draw is always reported by the first draw, since
    /// <see cref="ZoneGenerator.Names"/>, which they do not apply to, could still be called.
    /// </remarks>
    public static ZoneGenerator Zones() => new(ZoneSource.Default);

    /// <summary>
    /// Date-times in time zones: a wall-clock time in a zone of <see cref="ZoneSource.Default"/>,
    /// as it is now, with the offset and instant that zone gives it. Times the clocks skipped are
    /// not drawn unless <see cref="ZonedDateTimeGenerator.AllowImaginary"/> asks for them; repeated
    /// times are.
    /// </summary>
    /// <remarks>
    /// Bad arguments are reported as <see cref="Dates"/> describes, and what depends on the zone
    /// source as <see cref="Zones"/> describes. A drawn value .NET cannot hold (an instant outside
    /// the years 1 to 9999, which a wall-clock time within hours of either end can name) rejects
    /// the test case.
    /// </remarks>
    public static ZonedDateTimeGenerator ZonedDateTimes() => new(ZoneSource.Default);

    /// <summary>Durations, from zero up.</summary>
    [Obsolete("Use Generators.Durations(), which draws durations of either sign: Generators.Durations().Min(TimeSpan.Zero) is this generator.")]
    public static DurationGenerator TimeSpans() => DurationGenerator.NonNegative();

    /// <summary>UTC offsets: whole minutes between -14:00 and +14:00.</summary>
    [Obsolete("Use Generators.Offsets().")]
    public static OffsetGenerator UtcOffsets() => Offsets();

    /// <summary>The system's time zones.</summary>
    [Obsolete("Use Generators.Zones().")]
    public static ZoneGenerator TimeZones() => Zones();

    /// <summary>Strings matching <paramref name="pattern"/>, a regular expression in Python syntax.</summary>
    public static RegexGenerator FromRegex(string pattern)
    {
        ArgumentNullException.ThrowIfNull(pattern);
        return new(pattern, true, null);
    }

    /// <summary>Strings from <paramref name="grammar"/>, in Lark syntax.</summary>
    public static LarkGenerator FromLark(string grammar)
    {
        ArgumentNullException.ThrowIfNull(grammar);
        return new(grammar);
    }
}
