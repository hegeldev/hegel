namespace Hegel;

/// <summary>
/// An explicit test case: the values its draws return, positionally and by draw label, run before
/// the generated ones when <see cref="Settings.Examples"/> holds it.
/// </summary>
public sealed class Example
{
    private Example(IReadOnlyList<object?> positional, IReadOnlyDictionary<string, object?> named)
    {
        Positional = positional;
        Named = named;
    }

    /// <summary>An example whose draws return <paramref name="values"/> in order.</summary>
    public static Example Of(params object?[] values)
    {
        ArgumentNullException.ThrowIfNull(values);
        return new(Array.AsReadOnly((object?[])values.Clone()), new Dictionary<string, object?>());
    }

    /// <summary>This example with the draw labelled <paramref name="name"/> returning <paramref name="value"/>.</summary>
    public Example With(string name, object? value)
    {
        ArgumentNullException.ThrowIfNull(name);
        var copy = new Dictionary<string, object?>(Named) { [name] = value };
        return new Example(Positional, copy);
    }

    internal IReadOnlyList<object?> Positional { get; }

    internal IReadOnlyDictionary<string, object?> Named { get; }

    /// <inheritdoc />
    public override string ToString()
    {
        var parts = Positional.Select(Render).Concat(Named.Select(e => $"{e.Key}={Render(e.Value)}"));
        return $"Example({string.Join(", ", parts)})";
    }

    private static string Render(object? value) =>
        value is string s ? Printer.Quote(s, '"') : Printer.Invariant(value);
}
