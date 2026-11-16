using System.Numerics;
using Xunit;

namespace Hegel.Tests;

public enum Colour
{
    Red,
    Green,
    Blue,
}

public record Point(int X, int Y);

public record struct Pair(byte A, string B);

public record Shape(Point Origin, List<Point> Corners, Colour Colour, int? Weight);

public record Loop(Loop? Next, int Id);

public record Tree(Tree[] Kids);

public record Polygon(Point[] Points);

public record Forest(List<Forest> Groves);

public record Wrapped(Dictionary<string, Loop> Inner);

public record Settable
{
    public int X { get; init; }

    public required string Name { get; set; }

    public int Doubled => X * 2;
}

public record Mixed(int Id)
{
    public bool Flag { get; init; }
}

public record struct Empty;

public record Cyclic(Via<int> Next);

public record Via<T>(Cyclic Back);

public record Person
{
    public Person(string name)
    {
        Name = name;
    }

    public string Name { get; init; }
}

public record Money(int Amount)
{
    public Money(int amount, int scale) : this(amount * scale)
    {
    }

    private Money(int amount, int scale, int shift) : this(amount * scale + shift)
    {
    }

    public static Money Shifted(int amount) => new(amount, 1, 1);
}

public abstract record Animal(int Legs);

public record Dog(int Legs) : Animal(Legs);

public record Zoo(Animal Star);

public record Positive(int N)
{
    public int N { get; } = N > 0 ? N : throw new ArgumentOutOfRangeException(nameof(N));

    public int Half
    {
        get => N / 2;
        init => _ = value >= 0 ? value : throw new ArgumentOutOfRangeException(nameof(value));
    }
}

public sealed class NotARecord
{
    public int X { get; set; }
}

public class DeriveTests
{
    [Fact]
    public void ScalarsAreDerived()
    {
        Gen.All(Generators.ForType<int>(), _ => true, 5);
        Gen.All(Generators.ForType<long>(), _ => true, 5);
        Gen.All(Generators.ForType<short>(), _ => true, 5);
        Gen.All(Generators.ForType<sbyte>(), _ => true, 5);
        Gen.All(Generators.ForType<byte>(), _ => true, 5);
        Gen.All(Generators.ForType<ushort>(), _ => true, 5);
        Gen.All(Generators.ForType<uint>(), _ => true, 5);
        Gen.All(Generators.ForType<ulong>(), _ => true, 5);
        Gen.All(Generators.ForType<char>(), c => !char.IsSurrogate(c), 50);
        Gen.All(Generators.ForType<bool>(), _ => true, 5);
        Gen.All(Generators.ForType<float>(), _ => true, 5);
        Gen.All(Generators.ForType<double>(), _ => true, 5);
        Gen.All(Generators.ForType<BigInteger>(), _ => true, 5);
        Gen.All(Generators.ForType<string>(), _ => true, 5);
        Gen.All(Generators.ForType<Guid>(), _ => true, 5);
        Gen.All(Generators.ForType<byte[]>(), _ => true, 5);
        Gen.All(Generators.ForType<TimeSpan>(), _ => true, 5);
        Gen.All(Generators.ForType<DateOnly>(), _ => true, 5);
        Gen.All(Generators.ForType<TimeOnly>(), _ => true, 5);
        Gen.All(Generators.ForType<DateTime>(), _ => true, 5);
        Gen.All(Generators.ForType<DateTimeOffset>(), d => Math.Abs(d.Offset.TotalHours) <= 14, 5);
        Gen.All(Generators.ForType<TimeZoneInfo>(), _ => true, 5);
    }

    [Fact]
    public void EnumsSampleTheirMembers()
    {
        var values = Gen.Sample(Generators.ForType<Colour>(), 50);
        Assert.All(values, v => Assert.True(Enum.IsDefined(v)));
        Assert.Contains(Colour.Blue, values);
    }

    [Fact]
    public void NullablesAndArraysAreDerived()
    {
        var nullable = Gen.Sample(Generators.ForType<int?>(), 100);
        Assert.Contains(null, nullable);
        Assert.Contains(nullable, v => v != null);
        Gen.All(Generators.ForType<int[]>(), _ => true, 10);
        Gen.All(Generators.ForType<Colour[]>(), xs => xs.All(Enum.IsDefined), 10);
    }

    [Fact]
    public void ConcreteCollectionsAreDerived()
    {
        Gen.All(Generators.ForType<List<int>>(), _ => true, 5);
        Gen.All(Generators.ForType<HashSet<int>>(), _ => true, 5);
        Gen.All(Generators.ForType<Dictionary<int, string>>(), _ => true, 5);
        Assert.Equal("var x_1 = new List<Point> {};", Gen.MinimalOutput(Generators.ForType<List<Point>>()));
        Assert.Equal("var x_1 = new Dictionary<string, Colour> {};", Gen.MinimalOutput(Generators.ForType<Dictionary<string, Colour>>()));
    }

    [Fact]
    public void ListInterfacesAreDerived()
    {
        Gen.All(Generators.ForType<IList<int>>(), _ => true, 5);
        Gen.All(Generators.ForType<IReadOnlyList<int>>(), _ => true, 5);
        Gen.All(Generators.ForType<IEnumerable<int>>(), _ => true, 5);
        Gen.All(Generators.ForType<ICollection<int>>(), _ => true, 5);
        Gen.All(Generators.ForType<IReadOnlyCollection<int>>(), _ => true, 5);
    }

    [Fact]
    public void SetInterfacesAreDerived()
    {
        Gen.All(Generators.ForType<ISet<int>>(), _ => true, 5);
        Gen.All(Generators.ForType<IReadOnlySet<int>>(), _ => true, 5);
    }

    [Fact]
    public void DictionaryInterfacesAreDerived()
    {
        Gen.All(Generators.ForType<IDictionary<int, string>>(), _ => true, 5);
        Gen.All(Generators.ForType<IReadOnlyDictionary<int, string>>(), _ => true, 5);
    }

    [Fact]
    public void ValueTuplesAreDerived()
    {
        Gen.All(Generators.ForType<(int, string)>(), _ => true, 5);
        Gen.All(Generators.ForType<(byte, bool, Colour)>(), t => Enum.IsDefined(t.Item3), 5);
        Assert.Equal("var x_1 = (0, false);", Gen.MinimalOutput(Generators.ForType<(int, bool)>()));
    }

    [Fact]
    public void RecordsAreDerivedFromTheirPrimaryConstructor()
    {
        Gen.All(Generators.ForType<Point>(), _ => true, 10);
        Gen.All(Generators.ForType<Pair>(), _ => true, 10);
        Gen.All(Generators.ForType<Shape>(), s => Enum.IsDefined(s.Colour), 10);
        Gen.All(Generators.ForType<Polygon>(), _ => true, 10);
        Assert.Equal("var x_1 = new Polygon(new Point[] {});", Gen.MinimalOutput(Generators.ForType<Polygon>()));
        Assert.Equal("var x_1 = new Point(0, 0);", Gen.MinimalOutput(Generators.ForType<Point>()));
        Assert.Equal("var x_1 = new Box<int>(0);", Gen.MinimalOutput(Generators.ForType<Box<int>>()));
        Assert.Equal("var x_1 = new Pair(0, \"\");", Gen.MinimalOutput(Generators.ForType<Pair>()));
        Assert.Matches(@"^var x_1 = new Shape\(new Point\(0, 0\), new List<Point> \{\}, Colour\.Red, null\);$", Gen.MinimalOutput(Generators.ForType<Shape>()));
        Assert.Equal(Label.Combine(Label.FixedDict, Label.Of(typeof(Point).FullName!)), Generators.Records<Point>().Label);
    }

    [Fact]
    public void RecordsDrawTheirSettablePropertiesToo()
    {
        Assert.Equal("var x_1 = new Settable() { X = 0, Name = \"\" };", Gen.MinimalOutput(Generators.ForType<Settable>()));
        Assert.Equal("var x_1 = new Settable() { X = 1, Name = \"\" };", Gen.MinimalOutput(Generators.ForType<Settable>(), s => s.X > 0));
        Assert.Equal("var x_1 = new Mixed(0) { Flag = true };", Gen.MinimalOutput(Generators.ForType<Mixed>(), m => m.Flag));
        Assert.Equal("var x_1 = new Empty();", Gen.MinimalOutput(Generators.ForType<Empty>()));
        Gen.All(Generators.Records<Settable>().With("Name", Generators.Just("n")).With("X", Generators.Just(3)), s => s is { Name: "n", X: 3, Doubled: 6 }, 5);
        Assert.Throws<ArgumentException>(() => Generators.Records<Settable>().With("Doubled", Generators.Just(1)));
    }

    [Fact]
    public void AbstractRecordsAreRejected()
    {
        var e = Assert.Throws<ArgumentException>(() => Generators.Records<Animal>());
        Assert.StartsWith("Hegel.Tests.Animal is an abstract record", e.Message);
        Assert.Contains("No default generator for Hegel.Tests.Animal", Assert.Throws<HegelUsageException>(() => Generators.ForType<Animal>()).Message);
        Assert.Throws<HegelUsageException>(() => Gen.Sample(Generators.ForType<Zoo>(), 1));
        Gen.All(Generators.Records<Zoo>().With("Star", Generators.ForType<Dog>()), z => z.Star is Dog, 5);
    }

    [Fact]
    public void ARecordsOwnExceptionsAreNotWrapped()
    {
        var constructing = Generators.Records<Positive>().With("N", Generators.Just(0));
        Assert.Throws<ArgumentOutOfRangeException>(() => Property.Test(tc => tc.Draw(constructing), Gen.Quiet(3), Reporter.Silent));
        var setting = Generators.Records<Positive>().With("N", Generators.Just(1)).With("Half", Generators.Just(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => Property.Test(tc => tc.Draw(setting), Gen.Quiet(3), Reporter.Silent));
    }

    [Fact]
    public void RecordComponentsCanBeOverridden()
    {
        var g = Generators.Records<Point>().With("X", Generators.Just(5)).With("Y", Generators.Integers().Min(1).Max(2));
        Gen.All(g, p => p.X == 5 && p.Y is 1 or 2, 20);
        Assert.Equal("var x_1 = new Point(5, 1);", Gen.MinimalOutput(g));
        var e = Assert.Throws<ArgumentException>(() => Generators.Records<Point>().With("Z", Generators.Just(1)));
        Assert.Contains("has no record component named 'Z'", e.Message);
    }

    [Fact]
    public void RecordComponentOverridesMustDrawTheComponentType()
    {
        var e = Assert.Throws<ArgumentException>(() => Generators.Records<Point>().With("X", Generators.Longs()));
        Assert.Contains("Hegel.Tests.Point.X is of type int, not long", e.Message);
        Gen.All(Generators.Records<Shape>().With("Weight", Generators.Integers().Min(1).Max(1)), s => s.Weight == 1, 5);
    }

    [Fact]
    public void NonRecordsAreRejected()
    {
        var e = Assert.Throws<ArgumentException>(() => Generators.Records<NotARecord>());
        Assert.Contains("is not a record", e.Message);
        var derive = Assert.Throws<HegelUsageException>(() => Generators.ForType<NotARecord>());
        Assert.Contains("No default generator for Hegel.Tests.NotARecord", derive.Message);
        Assert.Throws<HegelUsageException>(() => Generators.ForType<Tuple<int, int>>());
        Assert.Throws<HegelUsageException>(() => Generators.ForType<int[,]>());
        Assert.Throws<HegelUsageException>(() => Generators.ForType<(int, NotARecord)>());
        Assert.Throws<HegelUsageException>(() => Generators.ForType<List<NotARecord>>());
    }

    [Fact]
    public void RecursiveRecordsAreRejected()
    {
        var direct = Assert.Throws<HegelUsageException>(() => Generators.ForType<Loop>());
        Assert.Contains("Loop -> Loop", direct.Message);
        Assert.Contains("Generators.Records<Loop>().With(...)", direct.Message);
        Assert.Contains("Tree -> Tree", Assert.Throws<HegelUsageException>(() => Generators.ForType<Tree>()).Message);
        Assert.Contains("Forest -> Forest", Assert.Throws<HegelUsageException>(() => Generators.ForType<Forest>()).Message);
        Assert.Contains("Wrapped -> Loop -> Loop", Assert.Throws<HegelUsageException>(() => Generators.ForType<Wrapped>()).Message);
        Assert.Contains("Cyclic -> Via`1 -> Cyclic", Assert.Throws<HegelUsageException>(() => Generators.ForType<Cyclic>()).Message);
    }

    [Fact]
    public void RecursiveRecordsWorkWithAnExplicitComponent()
    {
        var g = Generators.Records<Loop>().With("Next", Generators.Nulls<Loop>());
        Gen.All(g, l => l.Next == null, 5);
        Assert.Equal("var x_1 = new Loop(null, 0);", Gen.MinimalOutput(g));
    }

    [Fact]
    public void AConstructorParameterAndThePropertyItSetsAreOneComponent()
    {
        var g = Generators.Records<Person>().With("name", Generators.Just("fixed"));
        Assert.Equal("var x_1 = new Person(\"fixed\");", Gen.MinimalOutput(g));
    }

    [Fact]
    public void ThePrimaryConstructorIsChosenOverLongerOnes()
    {
        Assert.Equal(new Money(1, 2), Money.Shifted(1));
        Assert.Equal("var x_1 = new Money(0);", Gen.MinimalOutput(Generators.ForType<Money>()));
    }
}

public sealed record Box<T>(T Value);
