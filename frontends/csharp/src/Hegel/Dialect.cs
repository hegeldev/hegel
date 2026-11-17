namespace Hegel;

/// <summary>
/// The language a failing test case's draws are printed in: how the statement binding a draw is
/// written, and how <see cref="Printer.Repr(object?, Type)"/> prints a value. C# unless another
/// frontend, such as Hegel for F#, installs its own with <see cref="Use"/> around a run.
/// </summary>
public abstract class Dialect
{
    private static readonly AsyncLocal<Dialect?> Installed = new();

    /// <summary>The C# dialect: <c>var name = value;</c>, with values printed as the C# that builds them.</summary>
    public static Dialect CSharp { get; } = new CSharpDialect();

    /// <summary>The dialect in effect: the one installed with <see cref="Use"/>, or <see cref="CSharp"/>.</summary>
    public static Dialect Current => Installed.Value ?? CSharp;

    /// <summary>
    /// Make <paramref name="dialect"/> the current one until the returned scope is disposed, on this
    /// thread and on the threads and tasks it starts meanwhile.
    /// </summary>
    public static IDisposable Use(Dialect dialect)
    {
        ArgumentNullException.ThrowIfNull(dialect);
        var scope = new Scope(Installed.Value);
        Installed.Value = dialect;
        return scope;
    }

    /// <summary>Print <paramref name="value"/>, drawn with the static type <paramref name="staticType"/>, as the code that builds it.</summary>
    public abstract void Repr(Printer printer, object? value, Type staticType);

    /// <summary>Print the start of the statement binding a draw to <paramref name="name"/>.</summary>
    public abstract void BeginDraw(Printer printer, string name);

    /// <summary>Print the end of the statement binding a draw; the line break follows.</summary>
    public abstract void EndDraw(Printer printer);

    private sealed class Scope(Dialect? previous) : IDisposable
    {
        public void Dispose() => Installed.Value = previous;
    }

    private sealed class CSharpDialect : Dialect
    {
        public override void Repr(Printer printer, object? value, Type staticType) => printer.Render(value);

        public override void BeginDraw(Printer printer, string name) => printer.Text($"var {name} = ");

        public override void EndDraw(Printer printer) => printer.Text(";");
    }
}
