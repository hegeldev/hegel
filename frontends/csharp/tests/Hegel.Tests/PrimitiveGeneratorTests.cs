using System.Numerics;
using Xunit;

namespace Hegel.Tests;

public class PrimitiveGeneratorTests
{
    [Fact]
    public void IntegersRespectBounds()
    {
        Gen.All(Generators.Integers().Min(-3).Max(5), v => v >= -3 && v <= 5, 200);
        Gen.All(Generators.Integers(), _ => true);
    }

    [Fact]
    public void IntegersRejectInvertedBounds()
    {
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Integers().Min(2).Max(1));
        Assert.Contains("Integers: Min (2) > Max (1)", e.Message);
    }

    [Fact]
    public void IntegersHaveTheEngineLabel()
    {
        Assert.Equal(Label.Of("hegel.integers"), Generators.Integers().Label);
        Assert.Equal(Label.Of("hegel.integers"), Generators.Longs().Label);
        Assert.Equal(Label.Of("hegel.integers"), Generators.BigIntegers().Label);
    }

    [Fact]
    public void LongsRespectBounds()
    {
        Gen.All(Generators.Longs().Min(long.MinValue).Max(-1), v => v < 0, 100);
        Gen.All(Generators.Longs().Max(10).Min(10), v => v == 10);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Longs().Max(-1).Min(0));
        Assert.Contains("Longs: Min (0) > Max (-1)", e.Message);
    }

    [Fact]
    public void BigIntegersUnboundedStayAround2To127()
    {
        var limit = BigInteger.One << 127;
        Gen.All(Generators.BigIntegers(), v => v >= -limit && v < limit, 100);
    }

    [Fact]
    public void BigIntegersWithOneBoundExtendTheOtherSide()
    {
        var big = BigInteger.Parse("1000000000000000000000000000000000000000000");
        Gen.All(Generators.BigIntegers().Min(big), v => v >= big, 100);
        Gen.All(Generators.BigIntegers().Max(-big), v => v <= -big, 100);
        Gen.All(Generators.BigIntegers().Min(-5).Max(5), v => v >= -5 && v <= 5, 100);
        Gen.All(Generators.BigIntegers().Max(5).Min(-5), v => v >= -5 && v <= 5, 20);
    }

    [Fact]
    public void BigIntegersRejectInvertedBounds()
    {
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.BigIntegers().Min(1).Max(0));
        Assert.Contains("BigIntegers: Min (1) > Max (0)", e.Message);
    }

    [Fact]
    public void DoublesRespectBounds()
    {
        Gen.All(Generators.Doubles().Min(0.0).Max(1.0), v => v >= 0.0 && v <= 1.0, 200);
        Gen.All(Generators.Doubles().MinExclusive(0.0).MaxExclusive(1.0), v => v > 0.0 && v < 1.0, 200);
        Gen.All(Generators.Doubles().Min(1.0), v => v >= 1.0, 100);
        Gen.All(Generators.Doubles().Max(-1.0), v => v <= -1.0, 100);
    }

    [Fact]
    public void DoublesUnboundedAllowSpecialValuesUnlessToldOtherwise()
    {
        Gen.All(Generators.Doubles(), _ => true, 50);
        Gen.All(Generators.Doubles().AllowNan(false), v => !double.IsNaN(v), 200);
        Gen.All(Generators.Doubles().AllowInfinity(false).AllowNan(false), v => double.IsFinite(v), 200);
        Gen.All(Generators.Doubles().AllowNan(true).AllowInfinity(true), _ => true, 50);
        Gen.All(Generators.Doubles().AllowSubnormal(false), v => v == 0.0 || double.IsNaN(v) || Math.Abs(v) >= 2.2250738585072014E-308, 200);
        Gen.All(Generators.Doubles().Min(-1.0).Max(1.0).AllowSubnormal(true), v => v >= -1.0 && v <= 1.0, 50);
    }

    [Fact]
    public void DoublesWithOneBoundMayBeInfinite()
    {
        Gen.All(Generators.Doubles().Min(0.0).AllowInfinity(true), v => v >= 0.0, 50);
        Gen.All(Generators.Doubles().Min(0.0).AllowInfinity(false), v => v >= 0.0 && !double.IsPositiveInfinity(v), 200);
    }

    [Fact]
    public void FloatsRespectBounds()
    {
        Gen.All(Generators.Floats().Min(0f).Max(1f), v => v >= 0f && v <= 1f, 200);
        Gen.All(Generators.Floats().MinExclusive(0f).MaxExclusive(1f), v => v > 0f && v < 1f, 200);
        Gen.All(Generators.Floats(), _ => true, 50);
        Gen.All(Generators.Floats().AllowNan(false).AllowInfinity(false), v => float.IsFinite(v), 200);
        Gen.All(Generators.Floats().AllowSubnormal(false), v => v == 0f || float.IsNaN(v) || Math.Abs(v) >= 1.17549435E-38f, 200);
        Gen.All(Generators.Floats().Min(1f).AllowSubnormal(false), v => v >= 1f, 50);
        Gen.All(Generators.Floats().Max(-1f), v => v <= -1f, 50);
        Gen.All(Generators.Floats().Min(-1f).Max(1f).AllowSubnormal(true), v => v >= -1f && v <= 1f, 50);
        Assert.Equal(Label.Of("hegel.floats"), Generators.Floats().Label);
        Assert.Equal(Label.Of("hegel.floats"), Generators.Doubles().Label);
    }

    [Fact]
    public void FloatValidationRejectsNaNBounds()
    {
        Assert.Contains("Doubles: Min must not be NaN", Assert.Throws<ArgumentException>(() => Generators.Doubles().Min(double.NaN)).Message);
        Assert.Contains("Doubles: Max must not be NaN", Assert.Throws<ArgumentException>(() => Generators.Doubles().Max(double.NaN)).Message);
        Assert.Contains("Floats: Min must not be NaN", Assert.Throws<ArgumentException>(() => Generators.Floats().Min(float.NaN)).Message);
    }

    [Fact]
    public void FloatValidationRejectsEmptyRanges()
    {
        Assert.Contains("Doubles: Min (2.0) > Max (1.0)", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Doubles().Min(2.0).Max(1.0)).Message);
        Assert.Contains("no values between Min 0.0 and Max -0.0", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Doubles().Min(0.0).Max(-0.0)).Message);
        Assert.Contains("MinExclusive/MaxExclusive leave no values in [1.0, 1.0]", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Doubles().MinExclusive(1.0).Max(1.0)).Message);
        Assert.Contains("MinExclusive/MaxExclusive leave no values in [1.0, 1.0]", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Doubles().Min(1.0).MaxExclusive(1.0)).Message);
        Assert.Contains("MinExclusive(+Infinity) leaves no values", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Doubles().MinExclusive(double.PositiveInfinity)).Message);
        Assert.Contains("MaxExclusive(-Infinity) leaves no values", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Doubles().MaxExclusive(double.NegativeInfinity)).Message);
    }

    [Fact]
    public void FloatValidationRejectsSpecialValuesThatContradictBounds()
    {
        Assert.Contains("cannot allow NaN together with a bound", Assert.Throws<ArgumentException>(() => Generators.Doubles().Min(0.0).AllowNan(true)).Message);
        Assert.Contains("cannot allow infinity with both bounds set", Assert.Throws<ArgumentException>(() => Generators.Doubles().Min(0.0).Max(1.0).AllowInfinity(true)).Message);
    }

    [Fact]
    public void FloatValidationChecksSubnormals()
    {
        Assert.Contains("AllowSubnormal, but Min excludes all values below the smallest positive normal", Assert.Throws<ArgumentException>(() => Generators.Doubles().Min(1.0).AllowSubnormal(true)).Message);
        Assert.Contains("AllowSubnormal, but Max excludes all values above the smallest negative normal", Assert.Throws<ArgumentException>(() => Generators.Doubles().Max(-1.0).AllowSubnormal(true)).Message);
        Assert.Contains("AllowSubnormal(false) leaves no values in [5E-324, 1E-323]", Assert.Throws<ArgumentException>(() => Generators.Doubles().Min(double.Epsilon).Max(2 * double.Epsilon).AllowSubnormal(false)).Message);
        Assert.Contains("AllowSubnormal(false) leaves no values", Assert.Throws<ArgumentException>(() => Generators.Floats().Min(float.Epsilon).Max(2 * float.Epsilon).AllowSubnormal(false)).Message);
        Gen.All(Generators.Doubles().Min(double.Epsilon).Max(double.Epsilon), v => v == double.Epsilon, 10);
        Gen.All(Generators.Doubles().Min(1.0).Max(1.0), v => v == 1.0, 10);
        Gen.All(Generators.Doubles().Min(-1.0).Max(-0.5), v => v >= -1.0 && v <= -0.5, 20);
        Gen.All(Generators.Doubles().Min(0.5).Max(1.0).AllowSubnormal(false), v => v >= 0.5 && v <= 1.0, 20);
    }

    [Fact]
    public void FloatsResolveDefaultsSubnormalsFromTheBounds()
    {
        Assert.True(Floats.Resolve("x", 64, null, null, null, null, null, false, false).SmallestNonzeroMagnitude == double.Epsilon);
        Assert.Equal(1.17549435E-38, Floats.Resolve("x", 32, 1.0, null, null, null, null, false, false).SmallestNonzeroMagnitude);
        Assert.Equal(1.17549435E-38, Floats.Resolve("x", 32, null, -1.0, null, null, null, false, false).SmallestNonzeroMagnitude);
        Assert.Equal(float.Epsilon, Floats.Resolve("x", 32, null, 1.0, null, null, null, false, false).SmallestNonzeroMagnitude);
        Assert.Equal(double.Epsilon, Floats.Resolve("x", 64, 0.0, 0.0, null, null, null, false, false).SmallestNonzeroMagnitude);
        Assert.Equal(2.2250738585072014E-308, Floats.Resolve("x", 64, 1.0, 1.0, null, null, null, false, false).SmallestNonzeroMagnitude);
        var p = Floats.Resolve("x", 64, null, null, false, false, null, false, false);
        Assert.Equal(-double.MaxValue, p.Min);
        Assert.Equal(double.MaxValue, p.Max);
        var q = Floats.Resolve("x", 32, null, null, false, false, null, false, false);
        Assert.Equal(-(double)float.MaxValue, q.Min);
        Assert.Equal((double)float.MaxValue, q.Max);
    }

    [Fact]
    public void BooleansAreWeighted()
    {
        Gen.All(Generators.Booleans(), _ => true, 50);
        Gen.All(Generators.WeightedBooleans(1.0), v => v, 50);
        Gen.All(Generators.WeightedBooleans(0.0), v => !v, 50);
        Assert.Equal(Label.Of("hegel.booleans"), Generators.Booleans().Label);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.WeightedBooleans(1.5));
        Assert.Contains("WeightedBooleans: p must be between 0 and 1", e.Message);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.WeightedBooleans(double.NaN));
    }

    [Fact]
    public void BinaryRespectsSizes()
    {
        Gen.All(Generators.Binary().MinSize(2).MaxSize(4), v => v.Length >= 2 && v.Length <= 4, 100);
        Gen.All(Generators.Binary(), _ => true, 20);
        Gen.All(Generators.Binary().MaxSize(0), v => v.Length == 0, 10);
        Assert.Equal(Label.Of("hegel.binary"), Generators.Binary().Label);
    }

    [Fact]
    public void SizeValidationRejectsBadBounds()
    {
        Assert.Contains("Binary: MinSize must be >= 0", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Binary().MinSize(-1)).Message);
        Assert.Contains("Binary: MaxSize must be >= 0", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Binary().MaxSize(-1)).Message);
        Assert.Contains("Binary: MaxSize (1) must be >= MinSize (2)", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Binary().MinSize(2).MaxSize(1)).Message);
        Assert.Contains("Lists: MaxSize (0) must be >= MinSize (3)", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Lists(Generators.Integers()).MaxSize(0).MinSize(3)).Message);
    }

    [Fact]
    public void PrimitivesPrintAsLiterals()
    {
        Assert.Equal("var x_1 = 0;", Gen.MinimalOutput(Generators.Integers()));
        Assert.Equal("var x_1 = 0;", Gen.MinimalOutput(Generators.Longs()));
        Assert.Equal("var x_1 = 0;", Gen.MinimalOutput(Generators.BigIntegers()));
        Assert.Equal("var x_1 = false;", Gen.MinimalOutput(Generators.Booleans()));
        Assert.Equal("var x_1 = 0.0;", Gen.MinimalOutput(Generators.Doubles().Min(0.0).Max(1.0)));
        Assert.Equal("var x_1 = 0.0f;", Gen.MinimalOutput(Generators.Floats().Min(0f).Max(1f)));
        Assert.Equal("var x_1 = new byte[] {};", Gen.MinimalOutput(Generators.Binary()));
    }
}
