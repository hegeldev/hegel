using System.Numerics;

namespace Hegel;

/// <summary>Draws 32-bit integers in a range.</summary>
public sealed class IntegerGenerator : ValueGenerator<int>
{
    private readonly int _min;
    private readonly int _max;

    internal IntegerGenerator(int min, int max)
    {
        if (min > max)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"Integers: Min ({min}) > Max ({max})");
        }
        _min = min;
        _max = max;
    }

    /// <summary>At least <paramref name="min"/>.</summary>
    public IntegerGenerator Min(int min) => new(min, _max);

    /// <summary>At most <paramref name="max"/>.</summary>
    public IntegerGenerator Max(int max) => new(_min, max);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.integers");

    /// <inheritdoc />
    public override int DoDraw(TestCase tc) => (int)tc.GenerateInteger(_min, _max);
}

/// <summary>Draws 64-bit integers in a range.</summary>
public sealed class LongGenerator : ValueGenerator<long>
{
    private readonly long _min;
    private readonly long _max;

    internal LongGenerator(long min, long max)
    {
        if (min > max)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"Longs: Min ({min}) > Max ({max})");
        }
        _min = min;
        _max = max;
    }

    /// <summary>At least <paramref name="min"/>.</summary>
    public LongGenerator Min(long min) => new(min, _max);

    /// <summary>At most <paramref name="max"/>.</summary>
    public LongGenerator Max(long max) => new(_min, max);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.integers");

    /// <inheritdoc />
    public override long DoDraw(TestCase tc) => tc.GenerateInteger(_min, _max);
}

/// <summary>Draws integers of any size, in a range or around ±2^127 when unbounded.</summary>
public sealed class BigIntegerGenerator : ValueGenerator<BigInteger>
{
    private static readonly BigInteger UnboundedMagnitude = BigInteger.One << 127;

    private readonly BigInteger? _min;
    private readonly BigInteger? _max;
    private readonly BigInteger _lo;
    private readonly BigInteger _hi;

    internal BigIntegerGenerator(BigInteger? min, BigInteger? max)
    {
        if (min != null && max != null && min > max)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"BigIntegers: Min ({min}) > Max ({max})");
        }
        _min = min;
        _max = max;
        var lo = -UnboundedMagnitude;
        if (max != null)
        {
            lo = BigInteger.Min(lo, max.Value - UnboundedMagnitude + 1);
        }
        var hi = UnboundedMagnitude - 1;
        if (min != null)
        {
            hi = BigInteger.Max(hi, min.Value + UnboundedMagnitude - 1);
        }
        _lo = min ?? lo;
        _hi = max ?? hi;
    }

    /// <summary>At least <paramref name="min"/>.</summary>
    public BigIntegerGenerator Min(BigInteger min) => new(min, _max);

    /// <summary>At most <paramref name="max"/>.</summary>
    public BigIntegerGenerator Max(BigInteger max) => new(_min, max);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.integers");

    /// <inheritdoc />
    public override BigInteger DoDraw(TestCase tc) => tc.GenerateIntegerBig(_lo, _hi);
}

internal static class Floats
{
    internal readonly record struct DrawParams(double Min, double Max, bool AllowNan, bool AllowInfinity, double SmallestNonzeroMagnitude);

    private const double FloatMinNormal = 1.17549435E-38;
    private const double DoubleMinNormal = 2.2250738585072014E-308;

    internal static DrawParams Resolve(string what, int width, double? min, double? max, bool? allowNan, bool? allowInfinity, bool? allowSubnormal, bool excludeMin, bool excludeMax)
    {
        if (min is double.NaN)
        {
            throw new ArgumentException($"{what}: Min must not be NaN", nameof(min));
        }
        if (max is double.NaN)
        {
            throw new ArgumentException($"{what}: Max must not be NaN", nameof(max));
        }
        var hasMin = min != null;
        var hasMax = max != null;
        if (min is { } lo0 && max is { } hi0)
        {
            if (lo0 > hi0)
            {
                throw new ArgumentOutOfRangeException(nameof(min), min, $"{what}: Min ({Printer.Double(lo0)}) > Max ({Printer.Double(hi0)})");
            }
            if (lo0 == 0.0 && hi0 == 0.0 && BitConverter.DoubleToInt64Bits(lo0) == 0 && BitConverter.DoubleToInt64Bits(hi0) != 0)
            {
                throw new ArgumentOutOfRangeException(nameof(max), max, $"{what}: no values between Min 0.0 and Max -0.0");
            }
            if (lo0 == hi0 && (excludeMin || excludeMax))
            {
                throw new ArgumentOutOfRangeException(nameof(min), min, $"{what}: MinExclusive/MaxExclusive leave no values in [{Printer.Double(lo0)}, {Printer.Double(hi0)}]");
            }
        }
        if (excludeMin && min == double.PositiveInfinity)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"{what}: MinExclusive(+Infinity) leaves no values");
        }
        if (excludeMax && max == double.NegativeInfinity)
        {
            throw new ArgumentOutOfRangeException(nameof(max), max, $"{what}: MaxExclusive(-Infinity) leaves no values");
        }
        var an = allowNan ?? (!hasMin && !hasMax);
        var ai = allowInfinity ?? (!hasMin || !hasMax);
        if (an && (hasMin || hasMax))
        {
            throw new ArgumentException($"{what}: cannot allow NaN together with a bound", nameof(allowNan));
        }
        if (ai && hasMin && hasMax)
        {
            throw new ArgumentException($"{what}: cannot allow infinity with both bounds set", nameof(allowInfinity));
        }
        var smallestNormal = width == 32 ? FloatMinNormal : DoubleMinNormal;
        var subnormal = allowSubnormal ?? SubnormalDefault(min, max, smallestNormal);
        if (subnormal)
        {
            if (min >= smallestNormal)
            {
                throw new ArgumentException($"{what}: AllowSubnormal, but Min excludes all values below the smallest positive normal {Printer.Double(smallestNormal)}", nameof(allowSubnormal));
            }
            if (max <= -smallestNormal)
            {
                throw new ArgumentException($"{what}: AllowSubnormal, but Max excludes all values above the smallest negative normal -{Printer.Double(smallestNormal)}", nameof(allowSubnormal));
            }
        }
        else if (min is { } lo1 && max is { } hi1)
        {
            var containsZero = lo1 <= 0.0 && hi1 >= 0.0;
            if (!containsZero && hi1 < smallestNormal && lo1 > -smallestNormal)
            {
                throw new ArgumentException($"{what}: AllowSubnormal(false) leaves no values in [{Printer.Double(lo1)}, {Printer.Double(hi1)}]", nameof(allowSubnormal));
            }
        }
        var boundedDefault = !an && !ai;
        var widthMax = width == 32 ? float.MaxValue : double.MaxValue;
        var lo = min ?? (boundedDefault ? -widthMax : double.NegativeInfinity);
        var hi = max ?? (boundedDefault ? widthMax : double.PositiveInfinity);
        var smallestNonzero = subnormal ? (width == 32 ? float.Epsilon : double.Epsilon) : smallestNormal;
        return new DrawParams(lo, hi, an, ai, smallestNonzero);
    }

    private static bool SubnormalDefault(double? min, double? max, double smallestNormal)
    {
        if (min is { } lo && max is { } hi)
        {
            if (lo == hi)
            {
                return -smallestNormal < lo && lo < smallestNormal;
            }
            return lo < smallestNormal && hi > -smallestNormal;
        }
        if (min is { } onlyMin)
        {
            return onlyMin < smallestNormal;
        }
        if (max is { } onlyMax)
        {
            return onlyMax > -smallestNormal;
        }
        return true;
    }
}

/// <summary>Draws 64-bit floating-point numbers.</summary>
public sealed class DoubleGenerator : ValueGenerator<double>
{
    private readonly Floats.DrawParams _params;
    private readonly double? _min;
    private readonly double? _max;
    private readonly bool? _allowNan;
    private readonly bool? _allowInfinity;
    private readonly bool? _allowSubnormal;
    private readonly bool _excludeMin;
    private readonly bool _excludeMax;

    internal DoubleGenerator(double? min, double? max, bool? allowNan, bool? allowInfinity, bool? allowSubnormal, bool excludeMin, bool excludeMax)
    {
        _params = Floats.Resolve("Doubles", 64, min, max, allowNan, allowInfinity, allowSubnormal, excludeMin, excludeMax);
        _min = min;
        _max = max;
        _allowNan = allowNan;
        _allowInfinity = allowInfinity;
        _allowSubnormal = allowSubnormal;
        _excludeMin = excludeMin;
        _excludeMax = excludeMax;
    }

    /// <summary>At least <paramref name="min"/>.</summary>
    public DoubleGenerator Min(double min) => new(min, _max, _allowNan, _allowInfinity, _allowSubnormal, false, _excludeMax);

    /// <summary>At most <paramref name="max"/>.</summary>
    public DoubleGenerator Max(double max) => new(_min, max, _allowNan, _allowInfinity, _allowSubnormal, _excludeMin, false);

    /// <summary>More than <paramref name="min"/>.</summary>
    public DoubleGenerator MinExclusive(double min) => new(min, _max, _allowNan, _allowInfinity, _allowSubnormal, true, _excludeMax);

    /// <summary>Less than <paramref name="max"/>.</summary>
    public DoubleGenerator MaxExclusive(double max) => new(_min, max, _allowNan, _allowInfinity, _allowSubnormal, _excludeMin, true);

    /// <summary>Whether NaN may be drawn (by default, only when unbounded).</summary>
    public DoubleGenerator AllowNan(bool allow) => new(_min, _max, allow, _allowInfinity, _allowSubnormal, _excludeMin, _excludeMax);

    /// <summary>Whether infinities may be drawn (by default, only towards an unbounded side).</summary>
    public DoubleGenerator AllowInfinity(bool allow) => new(_min, _max, _allowNan, allow, _allowSubnormal, _excludeMin, _excludeMax);

    /// <summary>Whether subnormal numbers may be drawn (by default, when the range contains them).</summary>
    public DoubleGenerator AllowSubnormal(bool allow) => new(_min, _max, _allowNan, _allowInfinity, allow, _excludeMin, _excludeMax);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.floats");

    /// <inheritdoc />
    public override double DoDraw(TestCase tc) =>
        tc.GenerateFloat(64, _params.Min, _params.Max, _params.AllowNan, _params.AllowInfinity, _excludeMin, _excludeMax, _params.SmallestNonzeroMagnitude);
}

/// <summary>Draws 32-bit floating-point numbers.</summary>
public sealed class FloatGenerator : ValueGenerator<float>
{
    private readonly Floats.DrawParams _params;
    private readonly double? _min;
    private readonly double? _max;
    private readonly bool? _allowNan;
    private readonly bool? _allowInfinity;
    private readonly bool? _allowSubnormal;
    private readonly bool _excludeMin;
    private readonly bool _excludeMax;

    internal FloatGenerator(double? min, double? max, bool? allowNan, bool? allowInfinity, bool? allowSubnormal, bool excludeMin, bool excludeMax)
    {
        _params = Floats.Resolve("Floats", 32, min, max, allowNan, allowInfinity, allowSubnormal, excludeMin, excludeMax);
        _min = min;
        _max = max;
        _allowNan = allowNan;
        _allowInfinity = allowInfinity;
        _allowSubnormal = allowSubnormal;
        _excludeMin = excludeMin;
        _excludeMax = excludeMax;
    }

    /// <summary>At least <paramref name="min"/>.</summary>
    public FloatGenerator Min(float min) => new(min, _max, _allowNan, _allowInfinity, _allowSubnormal, false, _excludeMax);

    /// <summary>At most <paramref name="max"/>.</summary>
    public FloatGenerator Max(float max) => new(_min, max, _allowNan, _allowInfinity, _allowSubnormal, _excludeMin, false);

    /// <summary>More than <paramref name="min"/>.</summary>
    public FloatGenerator MinExclusive(float min) => new(min, _max, _allowNan, _allowInfinity, _allowSubnormal, true, _excludeMax);

    /// <summary>Less than <paramref name="max"/>.</summary>
    public FloatGenerator MaxExclusive(float max) => new(_min, max, _allowNan, _allowInfinity, _allowSubnormal, _excludeMin, true);

    /// <summary>Whether NaN may be drawn (by default, only when unbounded).</summary>
    public FloatGenerator AllowNan(bool allow) => new(_min, _max, allow, _allowInfinity, _allowSubnormal, _excludeMin, _excludeMax);

    /// <summary>Whether infinities may be drawn (by default, only towards an unbounded side).</summary>
    public FloatGenerator AllowInfinity(bool allow) => new(_min, _max, _allowNan, allow, _allowSubnormal, _excludeMin, _excludeMax);

    /// <summary>Whether subnormal numbers may be drawn (by default, when the range contains them).</summary>
    public FloatGenerator AllowSubnormal(bool allow) => new(_min, _max, _allowNan, _allowInfinity, allow, _excludeMin, _excludeMax);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.floats");

    /// <inheritdoc />
    public override float DoDraw(TestCase tc) =>
        (float)tc.GenerateFloat(32, _params.Min, _params.Max, _params.AllowNan, _params.AllowInfinity, _excludeMin, _excludeMax, _params.SmallestNonzeroMagnitude);
}

internal sealed class BooleanGenerator : ValueGenerator<bool>
{
    private readonly double _p;

    internal BooleanGenerator(double p)
    {
        if (!(p >= 0.0 && p <= 1.0))
        {
            throw new ArgumentOutOfRangeException(nameof(p), p, "WeightedBooleans: p must be between 0 and 1");
        }
        _p = p;
    }

    public override ulong Label => Hegel.Label.Of("hegel.booleans");

    public override bool DoDraw(TestCase tc) => tc.GenerateBoolean(_p);
}

/// <summary>Draws byte arrays.</summary>
public sealed class BinaryGenerator : ValueGenerator<byte[]>
{
    private readonly ulong _minSize;
    private readonly ulong _maxSize;

    internal BinaryGenerator(int minSize, ulong maxSize)
    {
        _minSize = Sizes.Validate(minSize, maxSize, "Binary");
        _maxSize = maxSize;
    }

    /// <summary>At least <paramref name="minSize"/> bytes.</summary>
    public BinaryGenerator MinSize(int minSize) => new(minSize, _maxSize);

    /// <summary>At most <paramref name="maxSize"/> bytes.</summary>
    public BinaryGenerator MaxSize(int maxSize) => new((int)_minSize, Sizes.Max(maxSize, "Binary"));

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.binary");

    /// <inheritdoc />
    public override byte[] DoDraw(TestCase tc) => tc.GenerateBytes(_minSize, Sizes.EngineMax(_maxSize));
}
