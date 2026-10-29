using System.Numerics;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.ExceptionServices;

namespace Hegel;

internal static class Derive
{
    private static readonly MethodInfo ForTypeMethod = typeof(Derive).GetMethod(nameof(ForType), BindingFlags.Static | BindingFlags.NonPublic)!;

    internal static IGenerator FromType(Type type) => (IGenerator)Call(ForTypeMethod.MakeGenericMethod(type));

    private static object Call(MethodInfo method)
    {
        try
        {
            return method.Invoke(null, null)!;
        }
        catch (TargetInvocationException e) when (e.InnerException != null)
        {
            ExceptionDispatchInfo.Throw(e.InnerException);
            throw;
        }
    }

    internal static PrintableGenerator<T> ForType<T>()
    {
        var type = typeof(T);
        if (Scalar(type) is { } scalar)
        {
            return (PrintableGenerator<T>)scalar;
        }
        if (type.IsEnum)
        {
            return (PrintableGenerator<T>)Generators.SampledFrom(Enum.GetValues(type).Cast<T>().ToArray());
        }
        if (Nullable.GetUnderlyingType(type) is { } underlying)
        {
            return (PrintableGenerator<T>)Construct(typeof(NullableOf<>), underlying);
        }
        if (type.IsArray && type.GetArrayRank() == 1)
        {
            return (PrintableGenerator<T>)Construct(typeof(ArrayOf<>), type.GetElementType()!);
        }
        if (type.IsGenericType)
        {
            var definition = type.GetGenericTypeDefinition();
            var arguments = type.GetGenericArguments();
            if (definition == typeof(List<>) || definition == typeof(IList<>) || definition == typeof(IReadOnlyList<>) || definition == typeof(IEnumerable<>) || definition == typeof(ICollection<>) || definition == typeof(IReadOnlyCollection<>))
            {
                return Upcast<T>(Construct(typeof(ListOf<>), arguments[0]), typeof(List<>).MakeGenericType(arguments));
            }
            if (definition == typeof(HashSet<>) || definition == typeof(ISet<>) || definition == typeof(IReadOnlySet<>))
            {
                return Upcast<T>(Construct(typeof(SetOf<>), arguments[0]), typeof(HashSet<>).MakeGenericType(arguments));
            }
            if (definition == typeof(Dictionary<,>) || definition == typeof(IDictionary<,>) || definition == typeof(IReadOnlyDictionary<,>))
            {
                return Upcast<T>(Construct(typeof(MapOf<,>), arguments), typeof(Dictionary<,>).MakeGenericType(arguments));
            }
            if (typeof(ITuple).IsAssignableFrom(type) && type.FullName!.StartsWith("System.ValueTuple`", StringComparison.Ordinal))
            {
                return TupleOf<T>(arguments);
            }
        }
        if (RecordGenerator<T>.IsRecord(type))
        {
            CheckAcyclic(type, new Stack<Type>());
            return new RecordGenerator<T>(new Dictionary<string, IGenerator>());
        }
        throw new HegelUsageException($"No default generator for {type.FullName}; supply one explicitly (a Generator parameter, or Generators.Records<T>().With(...) for a component).");
    }

    private static object Construct(Type definition, params Type[] arguments) =>
        Call(definition.MakeGenericType(arguments).GetMethod("Make")!);

    private static PrintableGenerator<T> Upcast<T>(object generator, Type source) =>
        source == typeof(T)
            ? (PrintableGenerator<T>)generator
            : (PrintableGenerator<T>)Activator.CreateInstance(typeof(UpcastGenerator<,>).MakeGenericType(source, typeof(T)), generator)!;

    private sealed class UpcastGenerator<TSource, TTarget>(PrintableGenerator<TSource> source) : PrintableGenerator<TTarget>
    {
        public override ulong Label => source.Label;

        public override TTarget DoDrawAndPrint(TestCase tc, Printer printer) => (TTarget)(object)source.DoDrawAndPrint(tc, printer)!;
    }

    private static class NullableOf<T> where T : struct
    {
        public static PrintableGenerator<T?> Make() => Generators.Nullable(ForType<T>());
    }

    private static class ArrayOf<T>
    {
        public static PrintableGenerator<T[]> Make() => Generators.Arrays(ForType<T>());
    }

    private static class ListOf<T>
    {
        public static PrintableGenerator<List<T>> Make() => Generators.Lists(ForType<T>());
    }

    private static class SetOf<T>
    {
        public static PrintableGenerator<HashSet<T>> Make() => Generators.Sets(ForType<T>());
    }

    private static class MapOf<TKey, TValue> where TKey : notnull
    {
        public static PrintableGenerator<Dictionary<TKey, TValue>> Make() => Generators.Maps(ForType<TKey>(), ForType<TValue>());
    }

    private static PrintableGenerator<T> TupleOf<T>(Type[] arguments)
    {
        var elements = arguments.Select(FromType).ToArray();
        var constructor = typeof(T).GetConstructor(arguments)!;
        return new TupleGenerator<T>(elements, values => (T)constructor.Invoke(values));
    }

    private static void CheckAcyclic(Type record, Stack<Type> path)
    {
        if (path.Contains(record))
        {
            var cycle = string.Join(" -> ", path.Reverse().Select(t => t.Name));
            throw new HegelUsageException($"Cannot derive a generator for {record.FullName}: it refers to itself through its components ({cycle} -> {record.Name}), so deriving one would recurse without bound. Build its generator with Generators.Recursive or Generators.Deferred, or give the recursive component a generator with Generators.Records<{record.Name}>().With(...)");
        }
        path.Push(record);
        foreach (var parameter in RecordGenerator<object>.PrimaryConstructor(record).GetParameters())
        {
            CheckAcyclicType(parameter.ParameterType, path);
        }
        path.Pop();
    }

    private static void CheckAcyclicType(Type type, Stack<Type> path)
    {
        if (type.IsArray)
        {
            CheckAcyclicType(type.GetElementType()!, path);
        }
        else if (type.IsGenericType)
        {
            foreach (var argument in type.GetGenericArguments())
            {
                CheckAcyclicType(argument, path);
            }
        }
        else if (RecordGenerator<object>.IsRecord(type))
        {
            CheckAcyclic(type, path);
        }
    }

    private static object? Scalar(Type type)
    {
        if (type == typeof(int))
        {
            return Generators.Integers();
        }
        if (type == typeof(long))
        {
            return Generators.Longs();
        }
        if (type == typeof(short))
        {
            return Generators.Integers().Min(short.MinValue).Max(short.MaxValue).Map(v => (short)v);
        }
        if (type == typeof(sbyte))
        {
            return Generators.Integers().Min(sbyte.MinValue).Max(sbyte.MaxValue).Map(v => (sbyte)v);
        }
        if (type == typeof(byte))
        {
            return Generators.Integers().Min(byte.MinValue).Max(byte.MaxValue).Map(v => (byte)v);
        }
        if (type == typeof(ushort))
        {
            return Generators.Integers().Min(ushort.MinValue).Max(ushort.MaxValue).Map(v => (ushort)v);
        }
        if (type == typeof(uint))
        {
            return Generators.Longs().Min(uint.MinValue).Max(uint.MaxValue).Map(v => (uint)v);
        }
        if (type == typeof(ulong))
        {
            return Generators.BigIntegers().Min(ulong.MinValue).Max(ulong.MaxValue).Map(v => (ulong)v);
        }
        if (type == typeof(char))
        {
            return Generators.Integers().Min(0).Max(char.MaxValue).Filter(c => !char.IsSurrogate((char)c)).Map(c => (char)c);
        }
        if (type == typeof(bool))
        {
            return Generators.Booleans();
        }
        if (type == typeof(float))
        {
            return Generators.Floats();
        }
        if (type == typeof(double))
        {
            return Generators.Doubles();
        }
        if (type == typeof(BigInteger))
        {
            return Generators.BigIntegers();
        }
        if (type == typeof(string))
        {
            return Generators.Text();
        }
        if (type == typeof(Guid))
        {
            return Generators.Uuids();
        }
        if (type == typeof(byte[]))
        {
            return Generators.Binary();
        }
        if (type == typeof(TimeSpan))
        {
            return Generators.TimeSpans();
        }
        if (type == typeof(DateOnly))
        {
            return Generators.Dates();
        }
        if (type == typeof(TimeOnly))
        {
            return Generators.Times();
        }
        if (type == typeof(DateTime))
        {
            return Generators.DateTimes();
        }
        if (type == typeof(DateTimeOffset))
        {
            return Generators.DateTimes().WithOffsets(Generators.UtcOffsets());
        }
        if (type == typeof(TimeZoneInfo))
        {
            return Generators.TimeZones();
        }
        return null;
    }
}

/// <summary>Draws records by drawing each component, printed as <c>new Name(a, b)</c>.</summary>
public sealed class RecordGenerator<T> : PrintableGenerator<T>
{
    private readonly IReadOnlyDictionary<string, IGenerator> _overrides;
    private readonly ConstructorInfo _constructor;
    private readonly CachedLabel _label = new();
    private IGenerator[]? _components;

    internal RecordGenerator(IReadOnlyDictionary<string, IGenerator> overrides)
    {
        if (!IsRecord(typeof(T)))
        {
            throw new ArgumentException($"{typeof(T).FullName} is not a record");
        }
        _overrides = overrides;
        _constructor = PrimaryConstructor(typeof(T));
    }

    private IGenerator[] Components() =>
        _components ??= _constructor.GetParameters()
            .Select(parameter => _overrides.TryGetValue(parameter.Name!, out var g) ? g : Derive.FromType(parameter.ParameterType))
            .ToArray();

    internal static bool IsRecord(Type type) =>
        type.GetMethod("<Clone>$", BindingFlags.Public | BindingFlags.Instance) != null || (type.IsValueType && type.GetMethod("PrintMembers", BindingFlags.NonPublic | BindingFlags.Instance) != null);

    internal static ConstructorInfo PrimaryConstructor(Type type) =>
        type.GetConstructors(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)
            .Where(c => !(c.GetParameters().Length == 1 && c.GetParameters()[0].ParameterType == type))
            .OrderByDescending(c => c.GetParameters().Length)
            .First();

    /// <summary>Draw the component <paramref name="component"/> from <paramref name="generator"/> instead of deriving one from its type.</summary>
    public RecordGenerator<T> With<TComponent>(string component, Generator<TComponent> generator)
    {
        ArgumentNullException.ThrowIfNull(component);
        ArgumentNullException.ThrowIfNull(generator);
        if (!_constructor.GetParameters().Any(p => p.Name == component))
        {
            throw new ArgumentException($"{typeof(T).FullName} has no record component named '{component}'", nameof(component));
        }
        var next = new Dictionary<string, IGenerator>(_overrides) { [component] = generator };
        return new RecordGenerator<T>(next);
    }

    /// <inheritdoc />
    public override ulong Label => _label.Get(() => Hegel.Label.Combine(Hegel.Label.FixedDict, Hegel.Label.Of(typeof(T).FullName!)));

    /// <inheritdoc />
    public override T DoDraw(TestCase tc) => Draw(tc, null);

    /// <inheritdoc />
    public override T DoDrawAndPrint(TestCase tc, Printer printer) => Draw(tc, printer);

    private T Draw(TestCase tc, Printer? printer)
    {
        var p = Hegel.Components.Out(printer);
        var components = Components();
        var values = new object?[components.Length];
        p.BeginGroup($"new {Printer.TypeName(typeof(T))}(");
        for (var i = 0; i < components.Length; i++)
        {
            if (i > 0)
            {
                p.Separator();
            }
            values[i] = components[i].Draw(tc, printer);
        }
        p.EndGroup(")");
        return (T)_constructor.Invoke(values);
    }
}
