namespace Hegel;

/// <summary>
/// A <see cref="Printer"/> of your own, outside any test case: print to <see cref="Printer"/> and
/// read the laid-out text with <see cref="Finish"/>.
/// </summary>
public sealed class Document : IDisposable
{
    /// <summary>The width a document breaks lines at unless <see cref="MaxWidth"/> says otherwise.</summary>
    public const int DefaultMaxWidth = 79;

    private readonly Printer.Regions _regions = new();
    private int _maxWidth = DefaultMaxWidth;
    private Printer? _printer;
    private bool _closed;

    /// <summary>The width lines break at. Set it before printing.</summary>
    public int MaxWidth
    {
        get => _maxWidth;
        set
        {
            if (_printer != null)
            {
                throw new InvalidOperationException("MaxWidth must be set before the document is printed to");
            }
            _maxWidth = value >= 1 ? value : throw new ArgumentOutOfRangeException(nameof(MaxWidth), value, "MaxWidth must be positive");
        }
    }

    /// <summary>The document's printer.</summary>
    public Printer Printer
    {
        get
        {
            if (_closed)
            {
                throw new InvalidOperationException("the document has been finished or disposed");
            }
            if (_printer == null)
            {
                var lib = Engine.Real();
                var options = lib.PrinterOptionsNew();
                try
                {
                    Printer.Check(lib, lib.PrinterOptionsSetMaxWidth(options, (ulong)_maxWidth), "options_set_max_width");
                    _printer = new Printer(lib, lib.PrinterNew(options), _regions);
                }
                finally
                {
                    lib.PrinterOptionsFree(options);
                }
            }
            return _printer;
        }
    }

    /// <summary>The laid-out text; the document is disposed afterwards.</summary>
    public string Finish()
    {
        var text = Printer.Read();
        Dispose();
        return text;
    }

    /// <summary>Frees the native printer of a document that was neither finished nor disposed.</summary>
    ~Document()
    {
        Release();
    }

    /// <inheritdoc />
    public void Dispose()
    {
        _closed = true;
        Release();
        GC.SuppressFinalize(this);
    }

    private void Release()
    {
        _regions.FreeAll();
        if (_printer != null)
        {
            _printer.Free();
            _printer = null;
        }
    }
}
