using System.Numerics;
using Hegel.Native;

namespace Hegel;

/// <summary>A handle to an engine string generator (text, regex, email, URL or domain), freed with the handle.</summary>
public sealed class StringGeneratorHandle
{
    internal ILibhegel Lib { get; }
    internal nint Handle { get; }

    internal StringGeneratorHandle(ILibhegel lib, nint handle)
    {
        Lib = lib;
        Handle = handle;
    }

    /// <summary>Frees the engine handle.</summary>
    ~StringGeneratorHandle()
    {
        Lib.StringGeneratorFree(Handle);
    }
}

/// <summary>A handle to a compiled engine grammar, freed with the handle.</summary>
public sealed class GrammarHandle
{
    internal ILibhegel Lib { get; }
    internal nint Handle { get; }

    internal GrammarHandle(ILibhegel lib, nint handle)
    {
        Lib = lib;
        Handle = handle;
    }

    /// <summary>Frees the engine handle.</summary>
    ~GrammarHandle()
    {
        Lib.GrammarFree(Handle);
    }
}

internal readonly record struct StateMachineInfo(nint Id, int Concurrency);

internal interface IDataSource
{
    bool GenerateBoolean(double p);
    long GenerateInteger(long min, long max);
    BigInteger GenerateIntegerBig(BigInteger min, BigInteger max);
    double GenerateFloat(int width, double min, double max, bool allowNan, bool allowInfinity, bool excludeMin, bool excludeMax, double smallestNonzeroMagnitude);
    byte[] GenerateBytes(ulong minSize, ulong maxSize);
    string GenerateString(StringGeneratorHandle generator);
    DateOnly GenerateDate(DateOnly min, DateOnly max);
    TimeOnly GenerateTime(TimeOnly min, TimeOnly max);
    DateTime GenerateDatetime(DateTime min, DateTime max);
    Guid GenerateUuid(int? version);
    byte[] GenerateIpv4();
    byte[] GenerateIpv6();
    StringGeneratorHandle TextGenerator(ulong minSize, ulong maxSize, string? codec, uint minCodepoint, uint maxCodepoint, IReadOnlyList<string>? categories, IReadOnlyList<string>? excludeCategories, string? includeCharacters, string? excludeCharacters);
    StringGeneratorHandle RegexGenerator(string pattern, bool fullmatch, StringGeneratorHandle? alphabet);
    StringGeneratorHandle EmailGenerator();
    StringGeneratorHandle UrlGenerator();
    StringGeneratorHandle DomainGenerator(ulong maxLength);
    bool OwnsStringGenerator(StringGeneratorHandle generator);
    GrammarHandle Grammar(string text, string? start, ulong maxSize, uint maxDepth, StringGeneratorHandle? alphabet, IReadOnlyList<string> explicitNames, Func<string, string?> files);
    string GenerateGrammar(GrammarHandle grammar, Func<int, string> terminal);
    bool OwnsGrammar(GrammarHandle grammar);
    void StartSpan(ulong label);
    void StopSpan(bool discard);
    nint NewCollection(ulong minSize, ulong maxSize);
    bool CollectionMore(nint id);
    void CollectionReject(nint id, string? why);
    void CollectionFree(nint id);
    nint NewRepeat();
    bool RepeatMore(nint id);
    void RepeatFailed(nint id);
    void RepeatFree(nint id);
    nint NewPool();
    long PoolAdd(nint poolId);
    long PoolGenerate(nint poolId, bool consume);
    void PoolRestore(nint poolId, long variableId);
    nint NewRecursion(ulong maxDepth, ulong maxLeaves);
    bool RecursionBranch(nint recursion, ulong depth);
    bool RecursionLeaf(nint recursion);
    void RecursionRetry(nint recursion);
    bool RecursionFinish(nint recursion);
    void RecursionFree(nint recursion);
    StateMachineInfo NewStateMachine(IReadOnlyList<string> ruleNames, long[] ruleGroups, double[]? ruleWeights, IReadOnlyList<string> invariantNames, bool[] invariantAlwaysCheck, long minConcurrency, long maxConcurrency, long stepCount);
    long StateMachineNextGroup(nint stateMachineId);
    long StateMachineNextRule(nint stateMachineId, long workerIndex);
    void StateMachineRuleRejected(nint stateMachineId, long workerIndex);
    bool StateMachineShouldCheckInvariant(nint stateMachineId, long invariantIndex);
    void StateMachineFree(nint stateMachineId);
    IDataSource Clone();
    void SetWorker(long workerIndex);
    IDataSource Block(ulong indent);
    void Release();
    void Note(string message);
    Printer Printer(Printer.Regions regions);
    void ReleasePools();
    void ReleaseThreadContext();
    bool IsAborted { get; }
    void Target(double value, string label);
    void Event(string label);
    void EventValue(string label, double value);
}
