using System.Numerics;
using System.Runtime.CompilerServices;
using Hegel.Native;

namespace Hegel;

internal sealed class ExplicitDraws : IDisposable
{
    private readonly Example _example;
    private readonly Printer _printer;
    private readonly HashSet<string> _used = [];
    private readonly object _gate = new();
    private int _consumed;

    internal ExplicitDraws(ILibhegel lib, Example example)
    {
        _example = example;
        _printer = new Printer(lib, lib.PrinterNew(0), new Printer.Regions());
    }

    internal Example Example => _example;

    internal object? Take(string name)
    {
        lock (_gate)
        {
            object? value;
            if (_example.Named.TryGetValue(name, out var named))
            {
                _used.Add(name);
                value = named;
            }
            else if (_consumed < _example.Positional.Count)
            {
                value = _example.Positional[_consumed++];
            }
            else
            {
                throw new InvalidExampleException($"Explicit example {_example} has no value for draw {name}: give it by name with Example.With, or add a positional value");
            }
            var dialect = Dialect.Current;
            dialect.BeginDraw(_printer, name);
            _printer.Repr(value);
            dialect.EndDraw(_printer);
            _printer.HardBreak();
            return value;
        }
    }

    internal void Note(string message)
    {
        lock (_gate)
        {
            foreach (var line in message.Split('\n'))
            {
                _printer.Text(line);
                _printer.HardBreak();
            }
        }
    }

    internal void CheckConsumed()
    {
        lock (_gate)
        {
            var problems = _example.Named.Keys.Where(name => !_used.Contains(name)).Select(name => $"no draw is named {name}").ToList();
            var leftover = _example.Positional.Count - _consumed;
            if (leftover > 0)
            {
                problems.Add($"{leftover} positional value(s) were not drawn");
            }
            if (problems.Count > 0)
            {
                throw new InvalidExampleException($"Explicit example {_example} was not fully used: {string.Join("; ", problems)}");
            }
        }
    }

    internal IReadOnlyList<string> Output() => Printer.Lines(_printer.Read());

    public void Dispose() => _printer.Free();
}

internal sealed class ExplicitDataSource : IDataSource
{
    internal static readonly ExplicitDataSource Instance = new();

    private static InvalidExampleException Unsupported([CallerMemberName] string name = "") =>
        new($"{name} is not supported in explicit test cases");

    public bool GenerateBoolean(double p) => throw Unsupported();
    public long GenerateInteger(long min, long max) => throw Unsupported();
    public BigInteger GenerateIntegerBig(BigInteger min, BigInteger max) => throw Unsupported();
    public double GenerateFloat(int width, double min, double max, bool allowNan, bool allowInfinity, bool excludeMin, bool excludeMax, double smallestNonzeroMagnitude) => throw Unsupported();
    public byte[] GenerateBytes(ulong minSize, ulong maxSize) => throw Unsupported();
    public string GenerateString(StringGeneratorHandle generator) => throw Unsupported();
    public DateOnly GenerateDate(DateOnly min, DateOnly max) => throw Unsupported();
    public TimeOnly GenerateTime(TimeOnly min, TimeOnly max) => throw Unsupported();
    public DateTime GenerateDatetime(DateTime min, DateTime max) => throw Unsupported();
    public Guid GenerateUuid(int? version) => throw Unsupported();
    public byte[] GenerateIpv4() => throw Unsupported();
    public byte[] GenerateIpv6() => throw Unsupported();
    public StringGeneratorHandle TextGenerator(ulong minSize, ulong maxSize, string? codec, uint minCodepoint, uint maxCodepoint, IReadOnlyList<string>? categories, IReadOnlyList<string>? excludeCategories, string? includeCharacters, string? excludeCharacters) => throw Unsupported();
    public StringGeneratorHandle RegexGenerator(string pattern, bool fullmatch, StringGeneratorHandle? alphabet) => throw Unsupported();
    public StringGeneratorHandle EmailGenerator() => throw Unsupported();
    public StringGeneratorHandle UrlGenerator() => throw Unsupported();
    public StringGeneratorHandle DomainGenerator(ulong maxLength) => throw Unsupported();
    public bool OwnsStringGenerator(StringGeneratorHandle generator) => throw Unsupported();
    public GrammarHandle Grammar(string text, string? start, ulong maxSize, uint maxDepth, StringGeneratorHandle? alphabet, IReadOnlyList<string> explicitNames, Func<string, string?> files) => throw Unsupported();
    public string GenerateGrammar(GrammarHandle grammar, Func<int, string> terminal) => throw Unsupported();
    public bool OwnsGrammar(GrammarHandle grammar) => throw Unsupported();
    public void StartSpan(ulong label) => throw Unsupported();
    public void StopSpan(bool discard) => throw Unsupported();
    public nint NewCollection(ulong minSize, ulong maxSize) => throw Unsupported();
    public bool CollectionMore(nint id) => throw Unsupported();
    public void CollectionReject(nint id, string? why) => throw Unsupported();
    public void CollectionFree(nint id) => throw Unsupported();
    public nint NewRepeat() => throw Unsupported();
    public bool RepeatMore(nint id) => throw Unsupported();
    public void RepeatFailed(nint id) => throw Unsupported();
    public void RepeatFree(nint id) => throw Unsupported();
    public nint NewPool() => throw Unsupported();
    public long PoolAdd(nint poolId) => throw Unsupported();
    public long PoolGenerate(nint poolId, bool consume) => throw Unsupported();
    public void PoolRestore(nint poolId, long variableId) => throw Unsupported();
    public nint NewRecursion(ulong maxDepth, ulong maxLeaves) => throw Unsupported();
    public bool RecursionBranch(nint recursion, ulong depth) => throw Unsupported();
    public bool RecursionLeaf(nint recursion) => throw Unsupported();
    public void RecursionRetry(nint recursion) => throw Unsupported();
    public bool RecursionFinish(nint recursion) => throw Unsupported();
    public void RecursionFree(nint recursion) => throw Unsupported();
    public StateMachineInfo NewStateMachine(IReadOnlyList<string> ruleNames, long[] ruleGroups, double[]? ruleWeights, IReadOnlyList<string> invariantNames, bool[] invariantAlwaysCheck, long minConcurrency, long maxConcurrency, long stepCount) => throw Unsupported();
    public long StateMachineNextGroup(nint stateMachineId) => throw Unsupported();
    public long StateMachineNextRule(nint stateMachineId, long workerIndex) => throw Unsupported();
    public void StateMachineRuleRejected(nint stateMachineId, long workerIndex) => throw Unsupported();
    public bool StateMachineShouldCheckInvariant(nint stateMachineId, long invariantIndex) => throw Unsupported();
    public void StateMachineFree(nint stateMachineId) => throw Unsupported();
    public IDataSource Clone() => throw Unsupported();
    public void SetWorker(long workerIndex) => throw Unsupported();
    public IDataSource Block(ulong indent) => throw Unsupported();
    public void Release() => throw Unsupported();
    public void Note(string message) => throw Unsupported();
    public Printer Printer(Printer.Regions regions) => throw Unsupported();
    public void ReleasePools() => throw Unsupported();
    public void ReleaseThreadContext() => throw Unsupported();
    public bool IsAborted => throw Unsupported();
    public void Target(double value, string label) { }
    public void Event(string label) { }
    public void EventValue(string label, double value) { }
}
