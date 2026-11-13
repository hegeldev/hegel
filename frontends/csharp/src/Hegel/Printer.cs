using System.Collections;
using System.Globalization;
using System.Numerics;
using System.Runtime.CompilerServices;
using Hegel.Native;

namespace Hegel;

/// <summary>
/// Prints a drawn value as the engine lays it out: text, groups that break across lines when they
/// run past the width, and comments. A <see cref="PrintableGenerator{T}"/> prints through one as it
/// draws; outside a draw, <see cref="Document"/> gives you one.
/// </summary>
public sealed class Printer
{
    private static readonly Printer NoopPrinter = new(null, 0, null);

    private readonly ILibhegel? _lib;
    private readonly nint _handle;
    private readonly Regions? _regions;
    private readonly object _gate = new();
    private bool _freed;

    internal sealed class Regions
    {
        private readonly List<Printer> _open = [];

        internal void Add(Printer printer)
        {
            lock (_open)
            {
                _open.Add(printer);
            }
        }

        internal void FreeAll()
        {
            List<Printer> taken;
            lock (_open)
            {
                taken = [.. _open];
                _open.Clear();
            }
            foreach (var printer in taken)
            {
                printer.Free();
            }
        }
    }

    internal Printer(ILibhegel? lib, nint handle, Regions? regions)
    {
        _lib = lib;
        _handle = handle;
        _regions = regions;
    }

    /// <summary>A printer that discards everything: what a silent draw prints to.</summary>
    public static Printer Noop => NoopPrinter;

    private bool Inactive => _handle == 0 || _freed;

    /// <summary>Whether anything printed now would be shown: false for <see cref="Noop"/> and inside an aborted speculation.</summary>
    public bool ShouldPrint
    {
        get
        {
            lock (_gate)
            {
                if (Inactive)
                {
                    return false;
                }
                Check(_lib!, _lib!.PrinterIsLive(_handle, out var live), "is_live");
                return live;
            }
        }
    }

    /// <summary>Print <paramref name="text"/>; each newline in it is a hard break.</summary>
    public void Text(string text)
    {
        lock (_gate)
        {
            if (Inactive)
            {
                return;
            }
            var lines = text.Split('\n');
            for (var i = 0; i < lines.Length; i++)
            {
                if (i > 0)
                {
                    Check(_lib!, _lib!.PrinterHardBreak(_handle), "hard_break");
                }
                if (lines[i].Length > 0)
                {
                    Check(_lib!, _lib!.PrinterText(_handle, lines[i]), "text");
                }
            }
        }
    }

    /// <summary>Print <paramref name="value"/> as the code that builds it, in the current <see cref="Dialect"/>.</summary>
    public void Repr(object? value) => Repr(value, typeof(object));

    /// <summary>
    /// Print <paramref name="value"/>, drawn with the static type <paramref name="staticType"/>, as
    /// the code that builds it, in the current <see cref="Dialect"/>. The static type tells a dialect
    /// what a <c>null</c> stands for.
    /// </summary>
    public void Repr(object? value, Type staticType)
    {
        ArgumentNullException.ThrowIfNull(staticType);
        lock (_gate)
        {
            if (ShouldPrint)
            {
                Dialect.Current.Repr(this, value, staticType);
            }
        }
    }

    /// <summary>Print <paramref name="representation"/>, letting the engine break it at its brackets and commas.</summary>
    public void Reflow(string representation) => Call(lib => lib.PrinterReflow(_handle, representation), "reflow");

    /// <summary>A place the line may break; <paramref name="separator"/> is printed when it does not.</summary>
    public void Breakable(string separator) => Call(lib => lib.PrinterBreakable(_handle, separator), "breakable");

    /// <summary>Print <paramref name="text"/> only when the enclosing group breaks.</summary>
    public void IfBreak(string text) => Call(lib => lib.PrinterIfBreak(_handle, text), "if_break");

    /// <summary>End the line.</summary>
    public void HardBreak() => Call(lib => lib.PrinterHardBreak(_handle), "hard_break");

    /// <summary>Open a group with <paramref name="open"/>, indenting its contents by <paramref name="indent"/> when it breaks.</summary>
    public void BeginGroup(int indent, string open) => Call(lib => lib.PrinterBeginGroup(_handle, checked((ulong)indent), open), "begin_group");

    /// <summary>Open a group with <paramref name="open"/>, indented by its width.</summary>
    public void BeginGroup(string open) => BeginGroup(new StringInfo(open).LengthInTextElements, open);

    /// <summary>Close the innermost group with <paramref name="close"/>.</summary>
    public void EndGroup(string close) => Call(lib => lib.PrinterEndGroup(_handle, close), "end_group");

    /// <summary>Print <paramref name="body"/> inside a group.</summary>
    public void Group(int indent, string open, string close, Action body)
    {
        BeginGroup(indent, open);
        body();
        EndGroup(close);
    }

    /// <summary>Print <paramref name="body"/> inside a group and return its result.</summary>
    public TResult Group<TResult>(int indent, string open, string close, Func<TResult> body)
    {
        BeginGroup(indent, open);
        var result = body();
        EndGroup(close);
        return result;
    }

    /// <summary>Print <paramref name="items"/> with <paramref name="print"/>, separated, inside a group.</summary>
    public void Seq<TItem>(string open, string close, IEnumerable<TItem> items, Action<TItem> print)
    {
        BeginGroup(open);
        var first = true;
        foreach (var item in items)
        {
            if (!first)
            {
                Separator();
            }
            first = false;
            print(item);
        }
        EndGroup(close);
    }

    /// <summary>A comma and a breakable space.</summary>
    public void Separator()
    {
        Text(",");
        Breakable(" ");
    }

    /// <summary>Change the indentation of the following lines by <paramref name="delta"/>.</summary>
    public void ShiftIndent(int delta) => Call(lib => lib.PrinterShiftIndent(_handle, delta), "shift_indent");

    /// <summary>A trailing comment on the current line.</summary>
    public void Comment(string text) => Call(lib => lib.PrinterComment(_handle, "  // " + text), "comment");

    /// <summary>
    /// Start printing speculatively: what follows is kept by <see cref="Speculation.Commit"/> and
    /// discarded by <see cref="Speculation.Abort"/> or by disposing the speculation first.
    /// </summary>
    public Speculation Speculate()
    {
        Call(lib => lib.PrinterBeginSpeculative(_handle), "begin_speculative");
        return new Speculation(this);
    }

    private void EndSpeculation(bool keep) =>
        Call(lib => keep ? lib.PrinterCommitSpeculative(_handle) : lib.PrinterAbortSpeculative(_handle), keep ? "commit_speculative" : "abort_speculative");

    /// <summary>
    /// A printer for a region of the output that is filled in later (printed to after this
    /// printer has moved on), as a filter's attempt count is.
    /// </summary>
    public Printer Deferred()
    {
        lock (_gate)
        {
            if (Inactive)
            {
                return NoopPrinter;
            }
            var rc = _lib!.PrinterDeferred(_handle, out var child);
            Check(_lib, rc, "deferred");
            if (rc != Abi.Ok)
            {
                return NoopPrinter;
            }
            var printer = new Printer(_lib, child, _regions);
            _regions!.Add(printer);
            return printer;
        }
    }

    private void Call(Func<ILibhegel, int> call, string op)
    {
        lock (_gate)
        {
            if (!Inactive)
            {
                Check(_lib!, call(_lib!), op);
            }
        }
    }

    internal string Read()
    {
        var resolved = _lib!.PrinterResolve(_handle);
        if (resolved != Abi.Ok && resolved != Abi.EInvalidArg)
        {
            throw new HegelException($"hegel_printer_resolve failed (rc={resolved}): {_lib.LastErrorMessage()}");
        }
        var rc = _lib.PrinterValue(_handle, out var value);
        if (rc != Abi.Ok)
        {
            throw new HegelException($"hegel_printer_value failed (rc={rc}): {_lib.LastErrorMessage()}");
        }
        return value;
    }

    internal static IReadOnlyList<string> Lines(string text)
    {
        if (text.Length == 0)
        {
            return [];
        }
        var lines = text.Split('\n');
        return lines[^1].Length == 0 ? lines[..^1] : lines;
    }

    internal void Free()
    {
        lock (_gate)
        {
            _freed = true;
            _lib!.PrinterFree(_handle);
        }
    }

    internal static void Check(ILibhegel lib, int rc, string op)
    {
        if (rc == Abi.Ok || rc == Abi.EInvalidHandle)
        {
            return;
        }
        var message = $"hegel_printer_{op} failed (rc={rc}): {lib.LastErrorMessage()}";
        if (rc == Abi.EInvalidArg)
        {
            throw new HegelUsageException(message);
        }
        throw new HegelException(message);
    }

    internal void Render(object? value)
    {
        switch (value)
        {
            case null:
                Text("null");
                break;
            case string s:
                Text(Quote(s, '"'));
                break;
            case char c:
                Text(Quote(c.ToString(), '\''));
                break;
            case bool b:
                Text(b ? "true" : "false");
                break;
            case float f:
                Text(Float(f));
                break;
            case double d:
                Text(Double(d));
                break;
            case decimal m:
                Text(m.ToString(CultureInfo.InvariantCulture) + "m");
                break;
            case not null when ScalarLiteral(value) is { } literal:
                Text(literal);
                break;
            case Enum e:
                Text(EnumLiteral(e));
                break;
            case Array array:
                Sequence($"new {TypeName(array.GetType().GetElementType()!)}[] {{", array.Cast<object?>(), "}");
                break;
            case IDictionary dictionary:
                Dictionary(dictionary);
                break;
            case ITuple tuple:
                Sequence("(", Enumerable.Range(0, tuple.Length).Select(i => tuple[i]), ")");
                break;
            case IEnumerable enumerable when value.GetType() is { IsGenericType: true } type && (type.GetGenericTypeDefinition() == typeof(List<>) || type.GetGenericTypeDefinition() == typeof(HashSet<>)):
                Sequence($"new {TypeName(type)} {{", enumerable.Cast<object?>(), "}");
                break;
            case IEnumerable enumerable:
                Sequence("[", enumerable.Cast<object?>(), "]");
                break;
            default:
                Reflow(Invariant(value));
                break;
        }
    }

    private void Dictionary(IDictionary dictionary)
    {
        var type = dictionary.GetType();
        var open = type.IsGenericType ? $"new {TypeName(type)} {{" : "new Dictionary<object, object> {";
        BeginGroup(open);
        var first = true;
        foreach (DictionaryEntry entry in dictionary)
        {
            if (!first)
            {
                Separator();
            }
            first = false;
            Text("[");
            Render(entry.Key);
            Text("] = ");
            Render(entry.Value);
        }
        EndGroup("}");
    }

    private void Sequence(string open, IEnumerable<object?> elements, string close)
    {
        BeginGroup(open);
        var first = true;
        foreach (var element in elements)
        {
            if (!first)
            {
                Separator();
            }
            first = false;
            Render(element);
        }
        EndGroup(close);
    }

    private static readonly Dictionary<Type, string> Keywords = new()
    {
        [typeof(bool)] = "bool",
        [typeof(byte)] = "byte",
        [typeof(sbyte)] = "sbyte",
        [typeof(char)] = "char",
        [typeof(short)] = "short",
        [typeof(ushort)] = "ushort",
        [typeof(int)] = "int",
        [typeof(uint)] = "uint",
        [typeof(long)] = "long",
        [typeof(ulong)] = "ulong",
        [typeof(float)] = "float",
        [typeof(double)] = "double",
        [typeof(decimal)] = "decimal",
        [typeof(string)] = "string",
        [typeof(object)] = "object",
    };

    internal static string TypeName(Type type)
    {
        if (Keywords.TryGetValue(type, out var keyword))
        {
            return keyword;
        }
        if (type.IsArray)
        {
            return TypeName(type.GetElementType()!) + "[]";
        }
        if (Nullable.GetUnderlyingType(type) is { } underlying)
        {
            return TypeName(underlying) + "?";
        }
        if (type.IsGenericType)
        {
            var name = type.Name;
            var tick = name.IndexOf('`');
            var arguments = string.Join(", ", type.GetGenericArguments().Select(TypeName));
            return $"{(tick < 0 ? name : name[..tick])}<{arguments}>";
        }
        return type.Name;
    }

    internal static string Float(float f)
    {
        if (float.IsNaN(f))
        {
            return "float.NaN";
        }
        if (float.IsPositiveInfinity(f))
        {
            return "float.PositiveInfinity";
        }
        if (float.IsNegativeInfinity(f))
        {
            return "float.NegativeInfinity";
        }
        var text = f.ToString("R", CultureInfo.InvariantCulture);
        return (text.Contains('.') || text.Contains('E') ? text : text + ".0") + "f";
    }

    internal static string Double(double d)
    {
        if (double.IsNaN(d))
        {
            return "double.NaN";
        }
        if (double.IsPositiveInfinity(d))
        {
            return "double.PositiveInfinity";
        }
        if (double.IsNegativeInfinity(d))
        {
            return "double.NegativeInfinity";
        }
        var text = d.ToString("R", CultureInfo.InvariantCulture);
        return text.Contains('.') || text.Contains('E') ? text : text + ".0";
    }

    internal static string? ScalarLiteral(object value) => value switch
    {
        Guid g => $"Guid.Parse(\"{g}\")",
        DateOnly date => $"DateOnly.Parse(\"{date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture)}\")",
        TimeOnly time => $"TimeOnly.Parse(\"{time.ToString("HH:mm:ss.FFFFFFF", CultureInfo.InvariantCulture)}\")",
        DateTime dateTime => $"DateTime.Parse(\"{dateTime.ToString("yyyy-MM-ddTHH:mm:ss.FFFFFFF", CultureInfo.InvariantCulture)}\")",
        DateTimeOffset offset => $"DateTimeOffset.Parse(\"{offset.ToString("yyyy-MM-ddTHH:mm:ss.FFFFFFFzzz", CultureInfo.InvariantCulture)}\")",
        TimeSpan span => $"TimeSpan.Parse(\"{span.ToString("c", CultureInfo.InvariantCulture)}\")",
        TimeZoneInfo zone => $"TimeZoneInfo.FindSystemTimeZoneById({Quote(zone.Id, '"')})",
        _ => null,
    };

    internal static string Invariant(object? value) => value switch
    {
        null => "null",
        float f => Float(f),
        double d => Double(d),
        bool b => b ? "true" : "false",
        BigInteger big => big.ToString(CultureInfo.InvariantCulture),
        IFormattable formattable => formattable.ToString(null, CultureInfo.InvariantCulture),
        _ => value.ToString() ?? "",
    };

    internal static string EnumLiteral(Enum value)
    {
        var type = TypeName(value.GetType());
        var text = value.ToString();
        if (char.IsAsciiDigit(text[0]) || text[0] == '-')
        {
            return $"({type}){text}";
        }
        return string.Join(" | ", text.Split(", ").Select(member => $"{type}.{member}"));
    }

    internal static string Quote(string s, char delimiter)
    {
        var sb = new System.Text.StringBuilder(s.Length + 2).Append(delimiter);
        for (var i = 0; i < s.Length; i++)
        {
            var c = s[i];
            if (char.IsHighSurrogate(c) && i + 1 < s.Length && char.IsLowSurrogate(s[i + 1]))
            {
                sb.Append(c).Append(s[i + 1]);
                i++;
                continue;
            }
            switch (c)
            {
                case '\\':
                    sb.Append("\\\\");
                    break;
                case '\n':
                    sb.Append("\\n");
                    break;
                case '\r':
                    sb.Append("\\r");
                    break;
                case '\t':
                    sb.Append("\\t");
                    break;
                case '\0':
                    sb.Append("\\0");
                    break;
                default:
                    if (c == delimiter)
                    {
                        sb.Append('\\').Append(c);
                    }
                    else if (char.IsControl(c) || c == '\u00a0' || c == '\u2028' || c == '\u2029' || char.IsSurrogate(c))
                    {
                        sb.Append(CultureInfo.InvariantCulture, $"\\u{(int)c:x4}");
                    }
                    else
                    {
                        sb.Append(c);
                    }
                    break;
            }
        }
        return sb.Append(delimiter).ToString();
    }

    /// <summary>A speculative region of a <see cref="Printer"/>'s output: kept on <see cref="Commit"/>, discarded otherwise.</summary>
    public sealed class Speculation : IDisposable
    {
        private readonly Printer _printer;
        private bool _open = true;

        internal Speculation(Printer printer)
        {
            _printer = printer;
        }

        /// <summary>Keep what was printed since <see cref="Printer.Speculate"/>.</summary>
        public void Commit()
        {
            _open = false;
            _printer.EndSpeculation(true);
        }

        /// <summary>Discard what was printed since <see cref="Printer.Speculate"/>.</summary>
        public void Abort()
        {
            _open = false;
            _printer.EndSpeculation(false);
        }

        /// <summary>Abort unless already committed or aborted.</summary>
        public void Dispose()
        {
            if (_open)
            {
                Abort();
            }
        }
    }
}
