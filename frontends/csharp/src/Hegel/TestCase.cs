using System.ComponentModel;
using System.Numerics;
using System.Runtime.ExceptionServices;

namespace Hegel;

/// <summary>
/// One run of the test body: draw values from generators, make assumptions, add notes. The
/// engine chooses what each draw returns, records the draws of a failing run and shrinks them to
/// the smallest failing test case, which is printed as the C# that rebuilds it.
/// </summary>
public sealed class TestCase
{
    private IDataSource _source;
    private readonly Shared _shared;
    private readonly ExplicitDraws? _explicit;
    private readonly Names _names;
    private readonly bool _scoped;
    private Printer? _printer;
    private bool _pinned;
    private int _drawDepth;

    private sealed class Names
    {
        private readonly Dictionary<string, int> _uses = [];

        internal string Next(string? label)
        {
            var @base = label ?? "draw";
            lock (_uses)
            {
                _uses[@base] = _uses.GetValueOrDefault(@base) + 1;
                return $"{@base}_{_uses[@base]}";
            }
        }
    }

    private sealed class Shared
    {
        internal readonly bool Captured;
        internal readonly bool Emits;
        internal readonly Printer.Regions Regions = new();
        private readonly Dictionary<string, object?> _draws = [];
        private readonly List<string> _order = [];
        private readonly List<TestCase> _blocks = [];

        internal Shared(bool captured, bool verbose)
        {
            Captured = captured;
            Emits = captured || verbose;
        }

        internal void Record(string name, object? value)
        {
            if (!Captured)
            {
                return;
            }
            lock (_draws)
            {
                var key = name;
                for (var n = 2; _draws.ContainsKey(key); n++)
                {
                    key = $"{name}_{n}";
                }
                _draws[key] = value;
                _order.Add(key);
            }
        }

        internal IReadOnlyDictionary<string, object?> Draws()
        {
            lock (_draws)
            {
                var copy = new Dictionary<string, object?>();
                foreach (var key in _order)
                {
                    copy[key] = _draws[key];
                }
                return copy;
            }
        }

        internal void AddBlock(TestCase block)
        {
            lock (_blocks)
            {
                _blocks.Add(block);
            }
        }

        internal List<TestCase> TakeBlocks()
        {
            lock (_blocks)
            {
                var taken = new List<TestCase>(_blocks);
                _blocks.Clear();
                return taken;
            }
        }
    }

    internal TestCase(IDataSource source, bool captured, bool verbose = false)
        : this(source, new Shared(captured, verbose), null, new Names(), false)
    {
    }

    private TestCase(IDataSource source, Shared shared, ExplicitDraws? explicitDraws, Names names, bool scoped)
    {
        _source = source;
        _shared = shared;
        _explicit = explicitDraws;
        _names = names;
        _scoped = scoped;
    }

    internal static TestCase Explicit(ExplicitDraws draws) =>
        new(ExplicitDataSource.Instance, new Shared(true, false), draws, new Names(), false);

    internal TestCase ForWorker(int worker)
    {
        var handle = new TestCase(_source.Clone(), _shared, null, _names, false);
        try
        {
            handle.SetWorker(worker);
        }
        catch
        {
            handle.Release();
            throw;
        }
        return handle;
    }

    internal TestCase Block(ulong indent) => new(_source.Block(indent), _shared, null, new Names(), true);

    internal void EndBlock()
    {
        if (!_pinned)
        {
            Release();
        }
    }

    internal void Pin()
    {
        if (_scoped && !_pinned)
        {
            _pinned = true;
            _shared.AddBlock(this);
        }
    }

    internal void Release()
    {
        try
        {
            ClosePrinter();
        }
        finally
        {
            _source.Release();
        }
    }

    internal void ReleaseAll()
    {
        try
        {
            ClosePrinter();
            foreach (var block in _shared.TakeBlocks())
            {
                block.Release();
            }
        }
        finally
        {
            _shared.Regions.FreeAll();
        }
    }

    private void ClosePrinter()
    {
        if (_printer != null)
        {
            var p = _printer;
            _printer = null;
            p.Free();
        }
    }

    internal IReadOnlyList<string> Output()
    {
        ClosePrinter();
        if (!_shared.Emits)
        {
            return [];
        }
        var root = _source.Printer(_shared.Regions);
        try
        {
            return Printer.Lines(root.Read());
        }
        finally
        {
            root.Free();
        }
    }

    /// <summary>
    /// Whether this is the final run of a failing test case, the one that is reported: the place
    /// for expensive diagnostics.
    /// </summary>
    public bool IsFinal => _shared.Captured;

    /// <summary>
    /// Draw a value from <paramref name="generator"/>, printed in the failure report as
    /// <c>var label_1 = ...;</c> (<c>draw_1</c> without a label). The generator must be printable.
    /// </summary>
    public T Draw<T>(Generator<T> generator, string? label = null)
    {
        ArgumentNullException.ThrowIfNull(generator);
        if (label != null && label.Contains('\n'))
        {
            throw new HegelUsageException($"draw label {Printer.Quote(label, '"')} must not contain a newline");
        }
        RequirePrintable(generator);
        if (_explicit != null)
        {
            return ExplicitDraw<T>(label);
        }
        if (_drawDepth > 0 || !_shared.Emits)
        {
            return DrawAndPrint(generator, Printer.Noop);
        }
        var p = CurrentPrinter();
        var name = _names.Next(label);
        T value;
        using (var speculation = p.Speculate())
        {
            var dialect = Dialect.Current;
            dialect.BeginDraw(p, name);
            value = DrawAndPrint(generator, p);
            dialect.EndDraw(p);
            p.HardBreak();
            speculation.Commit();
        }
        _shared.Record(name, value);
        return value;
    }

    /// <summary>Draw a value from <paramref name="generator"/> without printing it; the generator need not be printable.</summary>
    public T DrawSilent<T>(Generator<T> generator)
    {
        ArgumentNullException.ThrowIfNull(generator);
        if (_explicit != null)
        {
            throw new InvalidExampleException("DrawSilent is not supported in explicit test cases");
        }
        _drawDepth++;
        StartSpan(generator.Label);
        try
        {
            return generator.DoDraw(this);
        }
        finally
        {
            StopSpan(false);
            _drawDepth--;
        }
    }

    /// <summary>Draw a value from <paramref name="generator"/>, printing it to <paramref name="printer"/>: how a generator draws its parts.</summary>
    public T DrawAndPrint<T>(Generator<T> generator, Printer printer)
    {
        ArgumentNullException.ThrowIfNull(generator);
        ArgumentNullException.ThrowIfNull(printer);
        var printable = RequirePrintable(generator);
        _drawDepth++;
        StartSpan(generator.Label);
        try
        {
            return printable.DoDrawAndPrint(this, printer);
        }
        finally
        {
            StopSpan(false);
            _drawDepth--;
        }
    }

    private static PrintableGenerator<T> RequirePrintable<T>(Generator<T> generator)
    {
        if (generator is PrintableGenerator<T> printable)
        {
            return printable;
        }
        throw new HegelUsageException($"{generator.GetType().FullName} cannot print the values it draws: derive from PrintableGenerator, or make it printable with .PrintWith((value, printer) => ...) or .PrintAsRepr(); a Map whose input is printable can print the call with .PrintAsCall(\"Type.Factory\"); or draw it with tc.DrawSilent(...)");
    }

    private T ExplicitDraw<T>(string? label)
    {
        var name = _names.Next(label);
        var value = _explicit!.Take(name);
        _shared.Record(name, value);
        if (value is T typed)
        {
            return typed;
        }
        if (value == null && default(T) == null)
        {
            return default!;
        }
        throw new InvalidExampleException($"Explicit example {_explicit.Example} gives draw {name} a {(value == null ? "null" : value.GetType().FullName)}, but the draw expects a {typeof(T).FullName}");
    }

    private Printer CurrentPrinter() => _printer ??= _source.Printer(_shared.Regions);

    /// <summary>Reject this test case unless <paramref name="condition"/> holds; the engine tries another.</summary>
    public void Assume(bool condition)
    {
        if (!condition)
        {
            throw new AssumeRejected();
        }
    }

    /// <summary>Reject this test case; the engine tries another.</summary>
    public void Reject() => throw new AssumeRejected();

    /// <summary>Add <paramref name="message"/> to the test case's printed output.</summary>
    public void Note(string message)
    {
        ArgumentNullException.ThrowIfNull(message);
        if (_explicit != null)
        {
            _explicit.Note(message);
        }
        else if (_shared.Emits)
        {
            _source.Note(message);
        }
    }

    /// <summary>Guide the engine towards test cases that maximise <paramref name="value"/>.</summary>
    public void Target(double value, string label = "") => _source.Target(value, label);

    /// <summary>Count an occurrence of <paramref name="label"/> in the run's statistics.</summary>
    public void Event(string label) => _source.Event(label);

    /// <summary>Record <paramref name="value"/> under <paramref name="label"/> in the run's statistics.</summary>
    public void EventValue(string label, double value) => _source.EventValue(label, value);

    /// <summary>A second handle on this test case, for drawing from another thread.</summary>
    public TestCase Clone()
    {
        if (_explicit != null)
        {
            return new TestCase(_source, _shared, _explicit, _names, false);
        }
        var clone = new TestCase(_source.Clone(), _shared, null, _names, false);
        _shared.AddBlock(clone);
        return clone;
    }

    /// <summary>Mark this handle as belonging to the concurrent worker <paramref name="worker"/>.</summary>
    public void SetWorker(int worker)
    {
        if (_explicit != null)
        {
            return;
        }
        _source.SetWorker(worker);
    }

    /// <summary>
    /// Run <paramref name="body"/> a number of times the engine chooses, each in its own block of
    /// the output. Nothing after the call runs: it ends the test case.
    /// </summary>
    public void Repeat(Action body)
    {
        ArgumentNullException.ThrowIfNull(body);
        if (_explicit != null)
        {
            Note("// Repetition #1");
            body();
            throw new LoopDone();
        }
        var id = _source.NewRepeat();
        try
        {
            long repetition = 0;
            while (_source.RepeatMore(id))
            {
                repetition++;
                Note($"// Repetition #{repetition}");
                RepeatOnce(id, body);
            }
        }
        finally
        {
            _source.RepeatFree(id);
        }
        throw new LoopDone();
    }

    private void RepeatOnce(nint id, Action body)
    {
        Exception failure;
        var outer = _source;
        var block = outer.Block(2);
        SwapSource(block);
        try
        {
            body();
            return;
        }
        catch (Exception e)
        {
            failure = e;
        }
        finally
        {
            SwapSource(outer);
            block.Release();
        }
        if (failure is AssumeRejected && !IsAborted)
        {
            return;
        }
        if (!IsControl(failure))
        {
            _source.RepeatFailed(id);
        }
        ExceptionDispatchInfo.Throw(failure);
    }

    internal static bool IsControl(Exception e) =>
        e is StopTest or AssumeRejected or LoopDone or HegelUsageException or HegelException;

    private void SwapSource(IDataSource next)
    {
        ClosePrinter();
        _source = next;
    }

    /// <summary>Engine primitive: a boolean that is true with probability <paramref name="p"/>.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public bool GenerateBoolean(double p) => _source.GenerateBoolean(p);

    /// <summary>Engine primitive: an integer in [<paramref name="min"/>, <paramref name="max"/>].</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public long GenerateInteger(long min, long max) => _source.GenerateInteger(min, max);

    /// <summary>Engine primitive: an integer of any size in [<paramref name="min"/>, <paramref name="max"/>].</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public BigInteger GenerateIntegerBig(BigInteger min, BigInteger max) => _source.GenerateIntegerBig(min, max);

    /// <summary>Engine primitive: a floating-point number of <paramref name="width"/> bits.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public double GenerateFloat(int width, double min, double max, bool allowNan, bool allowInfinity, bool excludeMin, bool excludeMax, double smallestNonzeroMagnitude) =>
        _source.GenerateFloat(width, min, max, allowNan, allowInfinity, excludeMin, excludeMax, smallestNonzeroMagnitude);

    /// <summary>Engine primitive: bytes.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public byte[] GenerateBytes(ulong minSize, ulong maxSize) => _source.GenerateBytes(minSize, maxSize);

    /// <summary>Engine primitive: a string from an engine string generator.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public string GenerateString(StringGeneratorHandle generator) => _source.GenerateString(generator);

    /// <summary>Engine primitive: a date.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public DateOnly GenerateDate(DateOnly min, DateOnly max) => _source.GenerateDate(min, max);

    /// <summary>Engine primitive: a time of day.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public TimeOnly GenerateTime(TimeOnly min, TimeOnly max) => _source.GenerateTime(min, max);

    /// <summary>Engine primitive: a date and time.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public DateTime GenerateDatetime(DateTime min, DateTime max) => _source.GenerateDatetime(min, max);

    /// <summary>Engine primitive: a UUID of <paramref name="version"/>, or any version.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public Guid GenerateUuid(int? version) => _source.GenerateUuid(version);

    /// <summary>Engine primitive: the four bytes of an IPv4 address.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public byte[] GenerateIpv4() => _source.GenerateIpv4();

    /// <summary>Engine primitive: the sixteen bytes of an IPv6 address.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public byte[] GenerateIpv6() => _source.GenerateIpv6();

    /// <summary>Engine primitive: a text generator handle.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public StringGeneratorHandle TextGenerator(ulong minSize, ulong maxSize, string? codec, uint minCodepoint, uint maxCodepoint, IReadOnlyList<string>? categories, IReadOnlyList<string>? excludeCategories, string? includeCharacters, string? excludeCharacters) =>
        _source.TextGenerator(minSize, maxSize, codec, minCodepoint, maxCodepoint, categories, excludeCategories, includeCharacters, excludeCharacters);

    /// <summary>Engine primitive: a regex generator handle.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public StringGeneratorHandle RegexGenerator(string pattern, bool fullmatch, StringGeneratorHandle? alphabet) => _source.RegexGenerator(pattern, fullmatch, alphabet);

    /// <summary>Engine primitive: an email generator handle.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public StringGeneratorHandle EmailGenerator() => _source.EmailGenerator();

    /// <summary>Engine primitive: a URL generator handle.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public StringGeneratorHandle UrlGenerator() => _source.UrlGenerator();

    /// <summary>Engine primitive: a domain generator handle.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public StringGeneratorHandle DomainGenerator(ulong maxLength) => _source.DomainGenerator(maxLength);

    /// <summary>Engine primitive: whether <paramref name="generator"/> was made by this test case's engine.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public bool OwnsStringGenerator(StringGeneratorHandle generator) => _source.OwnsStringGenerator(generator);

    /// <summary>Engine primitive: compile a Lark grammar.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public GrammarHandle Grammar(string text, string? start, ulong maxSize, uint maxDepth, StringGeneratorHandle? alphabet, IReadOnlyList<string> explicitNames, Func<string, string?> files) =>
        _source.Grammar(text, start, maxSize, maxDepth, alphabet, explicitNames, files);

    /// <summary>Engine primitive: a string from a compiled grammar, with <paramref name="terminal"/> drawing the explicit terminals.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public string GenerateGrammar(GrammarHandle grammar, Func<int, string> terminal) => _source.GenerateGrammar(grammar, terminal);

    /// <summary>Engine primitive: whether <paramref name="grammar"/> was made by this test case's engine.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public bool OwnsGrammar(GrammarHandle grammar) => _source.OwnsGrammar(grammar);

    /// <summary>Run <paramref name="body"/> inside a span labelled <paramref name="label"/>.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public T Span<T>(ulong label, Func<T> body)
    {
        StartSpan(label);
        try
        {
            return body();
        }
        finally
        {
            StopSpan(false);
        }
    }

    /// <summary>Engine primitive: open a span.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public void StartSpan(ulong label) => _source.StartSpan(label);

    /// <summary>Engine primitive: close the innermost span, discarding its draws when <paramref name="discard"/>.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public void StopSpan(bool discard) => _source.StopSpan(discard);

    /// <summary>Engine primitive: start a collection of between <paramref name="minSize"/> and <paramref name="maxSize"/> elements.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public nint NewCollection(ulong minSize, ulong maxSize) => _source.NewCollection(minSize, maxSize);

    /// <summary>Engine primitive: whether the collection wants another element.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public bool CollectionMore(nint id) => _source.CollectionMore(id);

    /// <summary>Engine primitive: the last element was rejected.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public void CollectionReject(nint id, string? why) => _source.CollectionReject(id, why);

    /// <summary>Engine primitive: free a collection.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public void CollectionFree(nint id) => _source.CollectionFree(id);

    /// <summary>Engine primitive: start a recursive draw.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public nint NewRecursion(ulong maxDepth, ulong maxLeaves) => _source.NewRecursion(maxDepth, maxLeaves);

    /// <summary>Engine primitive: whether to branch at <paramref name="depth"/>.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public bool RecursionBranch(nint recursion, ulong depth) => _source.RecursionBranch(recursion, depth);

    /// <summary>Engine primitive: a leaf was drawn; false when the draw must unwind and retry.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public bool RecursionLeaf(nint recursion) => _source.RecursionLeaf(recursion);

    /// <summary>Engine primitive: retry the recursive draw.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public void RecursionRetry(nint recursion) => _source.RecursionRetry(recursion);

    /// <summary>Engine primitive: the recursive draw finished; false when it must retry.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public bool RecursionFinish(nint recursion) => _source.RecursionFinish(recursion);

    /// <summary>Engine primitive: free a recursion.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    public void RecursionFree(nint recursion) => _source.RecursionFree(recursion);

    internal nint NewPool() => _source.NewPool();

    internal long PoolAdd(nint poolId) => _source.PoolAdd(poolId);

    internal long PoolGenerate(nint poolId, bool consume) => _source.PoolGenerate(poolId, consume);

    internal void PoolRestore(nint poolId, long variableId) => _source.PoolRestore(poolId, variableId);

    internal StateMachineInfo NewStateMachine(IReadOnlyList<string> ruleNames, long[] ruleGroups, double[]? ruleWeights, IReadOnlyList<string> invariantNames, bool[] invariantAlwaysCheck, long minConcurrency, long maxConcurrency, long stepCount) =>
        _source.NewStateMachine(ruleNames, ruleGroups, ruleWeights, invariantNames, invariantAlwaysCheck, minConcurrency, maxConcurrency, stepCount);

    internal long StateMachineNextGroup(nint stateMachineId) => _source.StateMachineNextGroup(stateMachineId);

    internal long StateMachineNextRule(nint stateMachineId, long workerIndex) => _source.StateMachineNextRule(stateMachineId, workerIndex);

    internal void StateMachineRuleRejected(nint stateMachineId, long workerIndex) => _source.StateMachineRuleRejected(stateMachineId, workerIndex);

    internal bool StateMachineShouldCheckInvariant(nint stateMachineId, long invariantIndex) => _source.StateMachineShouldCheckInvariant(stateMachineId, invariantIndex);

    internal void StateMachineFree(nint stateMachineId) => _source.StateMachineFree(stateMachineId);

    internal void ReleaseThreadContext() => _source.ReleaseThreadContext();

    internal bool IsAborted => _source.IsAborted;

    internal IReadOnlyDictionary<string, object?> Draws() => _shared.Draws();
}
