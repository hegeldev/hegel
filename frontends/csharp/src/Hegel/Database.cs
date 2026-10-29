namespace Hegel;

/// <summary>
/// Where a run keeps the failures it finds, to replay them first next time: the engine's default
/// location, a directory of your choosing, or nowhere.
/// </summary>
public sealed class Database : IEquatable<Database>
{
    internal enum Kind
    {
        DefaultLocation,
        Disabled,
        Path,
    }

    internal Kind Which { get; }

    private Database(Kind kind, string? path)
    {
        Which = kind;
        Path = path;
    }

    /// <summary>The engine's default location (a <c>.hegel</c> directory found from the working directory).</summary>
    public static Database DefaultLocation { get; } = new(Kind.DefaultLocation, null);

    /// <summary>No database: nothing is saved or replayed.</summary>
    public static Database Disabled { get; } = new(Kind.Disabled, null);

    /// <summary>A database in the directory at <paramref name="path"/>.</summary>
    public static Database At(string path) => new(Kind.Path, path ?? throw new ArgumentNullException(nameof(path)));

    /// <summary>The directory, when this is a <see cref="At"/> database.</summary>
    public string? Path { get; }

    internal static Database FromEngine(string? database) =>
        database == null ? DefaultLocation : database.Length == 0 ? Disabled : At(database);

    internal string? ForEngine() => Which switch
    {
        Kind.Disabled => "",
        Kind.Path => Path,
        _ => null,
    };

    /// <inheritdoc />
    public bool Equals(Database? other) => other != null && other.Which == Which && other.Path == Path;

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as Database);

    /// <inheritdoc />
    public override int GetHashCode() => HashCode.Combine(Which, Path);

    /// <inheritdoc />
    public override string ToString() => Which switch
    {
        Kind.Disabled => "Database.Disabled",
        Kind.Path => $"Database.At({Printer.Quote(Path!, '"')})",
        _ => "Database.DefaultLocation",
    };
}
