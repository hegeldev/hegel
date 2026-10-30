using Hegel.Native;

namespace Hegel;

internal static class Components
{
    internal static T Draw<T>(TestCase tc, Generator<T> component, Printer? printer) =>
        printer == null ? tc.DrawSilent(component) : tc.DrawAndPrint(component, printer);

    internal static Printer Out(Printer? printer) => printer ?? Printer.Noop;
}

/// <summary>A printable generator that prints its values with <see cref="Printer.Repr(object?)"/>.</summary>
/// <typeparam name="T">The type of value drawn.</typeparam>
public abstract class ValueGenerator<T> : PrintableGenerator<T>
{
    /// <inheritdoc />
    public abstract override T DoDraw(TestCase tc);

    /// <inheritdoc />
    public sealed override T DoDrawAndPrint(TestCase tc, Printer printer)
    {
        var value = DoDraw(tc);
        printer.Repr(value, typeof(T));
        return value;
    }
}

internal sealed class CachedLabel
{
    private ulong? _value;

    internal static ulong Combine(ulong own, IEnumerable<IGenerator> components) =>
        Label.Combine([own, .. components.Select(c => c.Label)]);

    internal ulong Get(Func<ulong> compute)
    {
        var label = _value;
        if (label == null)
        {
            label = compute();
            _value = label;
        }
        return label.Value;
    }
}

internal sealed class HandleCache
{
    private StringGeneratorHandle? _handle;

    internal StringGeneratorHandle Get(TestCase tc, Func<TestCase, StringGeneratorHandle> build)
    {
        var h = _handle;
        if (h == null || !tc.OwnsStringGenerator(h))
        {
            h = build(tc);
            _handle = h;
        }
        return h;
    }
}

internal static class Sizes
{
    internal static ulong Validate(int minSize, ulong maxSize, string what)
    {
        if (minSize < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(minSize), minSize, $"{what}: MinSize must be >= 0");
        }
        if (maxSize != Abi.Unbounded && maxSize < (ulong)minSize)
        {
            throw new ArgumentOutOfRangeException(nameof(maxSize), maxSize, $"{what}: MaxSize ({maxSize}) must be >= MinSize ({minSize})");
        }
        return (ulong)minSize;
    }

    internal static ulong Max(int maxSize, string what) =>
        maxSize >= 0 ? (ulong)maxSize : throw new ArgumentOutOfRangeException(nameof(maxSize), maxSize, $"{what}: MaxSize must be >= 0");

    internal static ulong EngineMax(ulong maxSize) => maxSize == Abi.Unbounded ? Abi.MaxSizeDefault : maxSize;
}
