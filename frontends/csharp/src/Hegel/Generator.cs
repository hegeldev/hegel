namespace Hegel;

/// <summary>
/// Draws values of <typeparamref name="T"/> from a <see cref="TestCase"/>. Implement
/// <see cref="DoDraw"/> for a generator that draws through other generators or the engine
/// primitives, or derive from <see cref="PrintableGenerator{T}"/> to print as you draw. Built-in
/// generators are in <see cref="Generators"/>.
/// </summary>
/// <typeparam name="T">The type of value drawn.</typeparam>
public abstract class Generator<T> : IGenerator
{
    object? IGenerator.Draw(TestCase tc, Printer? printer) => printer == null ? tc.DrawSilent(this) : tc.DrawAndPrint(this, printer);

    ulong IGenerator.Label => Label;

    /// <summary>Draw one value. Called inside a span labelled <see cref="Label"/>.</summary>
    public abstract T DoDraw(TestCase tc);

    private ulong? _label;

    /// <summary>
    /// The span label the engine files this generator's draws under. Defaults to a label derived
    /// from the class's full name.
    /// </summary>
    public virtual ulong Label => _label ??= Hegel.Label.Of(GetType().FullName!);

    /// <summary>Draw from this generator and apply <paramref name="f"/>.</summary>
    public MappedGenerator<T, TResult> Map<TResult>(Func<T, TResult> f)
    {
        ArgumentNullException.ThrowIfNull(f);
        return new(this, f);
    }

    /// <summary>Draw from this generator until <paramref name="predicate"/> accepts the value.</summary>
    public virtual PrintableGenerator<T> Filter(Func<T, bool> predicate)
    {
        ArgumentNullException.ThrowIfNull(predicate);
        return new FilteredGenerator<T>(this, predicate);
    }

    /// <summary>Draw from this generator and then from the generator <paramref name="f"/> gives for the value.</summary>
    public PrintableGenerator<TResult> FlatMap<TResult>(Func<T, Generator<TResult>> f)
    {
        ArgumentNullException.ThrowIfNull(f);
        return new FlatMappedGenerator<T, TResult>(this, f);
    }

    /// <summary>This generator, printing its values with <paramref name="print"/>.</summary>
    public PrintableGenerator<T> PrintWith(Action<T, Printer> print)
    {
        ArgumentNullException.ThrowIfNull(print);
        return new PrintedGenerator<T>(this, print);
    }

    /// <summary>This generator, printing its values with <see cref="Printer.Repr(object?)"/>.</summary>
    public PrintableGenerator<T> PrintAsRepr() => PrintWith((value, printer) => printer.Repr(value));
}

/// <summary>
/// A <see cref="Generator{T}"/> that prints each value as it draws it, so a failing test case's
/// draws can be shown as the code that rebuilds them. Only printable generators can be drawn with
/// <see cref="TestCase.Draw{T}(Generator{T}, string?)"/>.
/// </summary>
/// <typeparam name="T">The type of value drawn.</typeparam>
public abstract class PrintableGenerator<T> : Generator<T>
{
    /// <summary>Draw one value, printing it to <paramref name="printer"/> (which may be <see cref="Printer.Noop"/>).</summary>
    public abstract T DoDrawAndPrint(TestCase tc, Printer printer);

    /// <inheritdoc />
    public override T DoDraw(TestCase tc) => DoDrawAndPrint(tc, Printer.Noop);
}

internal interface IGenerator
{
    object? Draw(TestCase tc, Printer? printer);

    ulong Label { get; }
}
