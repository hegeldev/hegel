using System.Numerics;
using Hegel.Native;

namespace Hegel;

internal sealed class LiveDataSource : IDataSource
{
    private sealed class Flag
    {
        internal bool Value;
    }

    private readonly ILibhegel _lib;
    private readonly nint _tc;
    private readonly List<nint> _pools;
    private readonly Flag _aborted;
    private readonly Flag _unwinding;
    private bool _released;

    internal LiveDataSource(ILibhegel lib, nint tc)
        : this(lib, tc, [], new Flag(), new Flag())
    {
    }

    private LiveDataSource(ILibhegel lib, nint tc, List<nint> pools, Flag aborted, Flag unwinding)
    {
        _lib = lib;
        _tc = tc;
        _pools = pools;
        _aborted = aborted;
        _unwinding = unwinding;
    }

    public bool IsAborted => _aborted.Value;

    internal const string ReleasedMessage =
        "This TestCase belonged to a stateful rule that has ended and can no longer be used: draw through the TestCase the current rule was given.";

    private void Translate(int rc, string op)
    {
        switch (rc)
        {
            case Abi.Ok:
                return;
            case Abi.EStopTest:
                _aborted.Value = true;
                throw new StopTest();
            case Abi.EAssume:
                _aborted.Value = true;
                throw new AssumeRejected();
            case Abi.EInvalidArg:
                throw new HegelUsageException(_lib.LastErrorMessage());
            case Abi.EConcurrentUse:
                throw new HegelException($"hegel_{op}: the test-case handle was used from two threads at once. In a concurrent state machine, draw only through the TestCase handed to the rule and share generated values through a ConcurrentPool.");
            default:
                throw new HegelException($"hegel_{op} failed (rc={rc}): {_lib.LastErrorMessage()}");
        }
    }

    private void CheckLive()
    {
        CheckNotReleased();
        if (_aborted.Value)
        {
            throw new StopTest();
        }
    }

    private void CheckNotReleased()
    {
        if (_released)
        {
            throw new HegelUsageException(ReleasedMessage);
        }
    }

    public bool GenerateBoolean(double p)
    {
        CheckLive();
        Translate(_lib.GenerateBoolean(_tc, p, out var value), "generate_boolean");
        return value;
    }

    public long GenerateInteger(long min, long max)
    {
        CheckLive();
        Translate(_lib.GenerateInteger(_tc, min, max, out var value), "generate_integer");
        return value;
    }

    public BigInteger GenerateIntegerBig(BigInteger min, BigInteger max)
    {
        if (min >= long.MinValue && min <= long.MaxValue && max >= long.MinValue && max <= long.MaxValue)
        {
            return GenerateInteger((long)min, (long)max);
        }
        CheckLive();
        Translate(_lib.GenerateIntegerBig(_tc, min.ToByteArray(), max.ToByteArray(), out var value), "generate_integer_big");
        return new BigInteger(value);
    }

    public double GenerateFloat(int width, double min, double max, bool allowNan, bool allowInfinity, bool excludeMin, bool excludeMax, double smallestNonzeroMagnitude)
    {
        CheckLive();
        Translate(_lib.GenerateFloat(_tc, width, min, max, allowNan, allowInfinity, excludeMin, excludeMax, smallestNonzeroMagnitude, out var value), "generate_float");
        return value;
    }

    public byte[] GenerateBytes(ulong minSize, ulong maxSize)
    {
        CheckLive();
        Translate(_lib.GenerateBytes(_tc, minSize, maxSize, out var value), "generate_bytes");
        return value;
    }

    public string GenerateString(StringGeneratorHandle generator)
    {
        CheckLive();
        try
        {
            Translate(_lib.GenerateString(_tc, generator.Handle, out var value), "generate_string");
            return value;
        }
        finally
        {
            GC.KeepAlive(generator);
        }
    }

    public DateOnly GenerateDate(DateOnly min, DateOnly max)
    {
        CheckLive();
        Translate(_lib.GenerateDate(_tc, ToNative(min), ToNative(max), out var value), "generate_date");
        return FromNative(value);
    }

    public TimeOnly GenerateTime(TimeOnly min, TimeOnly max)
    {
        CheckLive();
        Translate(_lib.GenerateTime(_tc, ToNative(min), ToNative(max), out var value), "generate_time");
        return FromNative(value);
    }

    public DateTime GenerateDatetime(DateTime min, DateTime max)
    {
        CheckLive();
        var nativeMin = new HegelDateTime { Date = ToNative(DateOnly.FromDateTime(min)), Time = ToNative(TimeOnly.FromDateTime(min)) };
        var nativeMax = new HegelDateTime { Date = ToNative(DateOnly.FromDateTime(max)), Time = ToNative(TimeOnly.FromDateTime(max)) };
        Translate(_lib.GenerateDatetime(_tc, nativeMin, nativeMax, out var value), "generate_datetime");
        return FromNative(value.Date).ToDateTime(FromNative(value.Time), DateTimeKind.Unspecified);
    }

    internal static HegelDate ToNative(DateOnly date) => new() { Year = date.Year, Month = (byte)date.Month, Day = (byte)date.Day };

    internal static DateOnly FromNative(HegelDate date) => new(date.Year, date.Month, date.Day);

    internal static HegelTime ToNative(TimeOnly time) => new()
    {
        Hour = (byte)time.Hour,
        Minute = (byte)time.Minute,
        Second = (byte)time.Second,
        Nanosecond = (uint)(time.Ticks % TimeSpan.TicksPerSecond * 100),
    };

    internal static TimeOnly FromNative(HegelTime time) =>
        new(((time.Hour * 60L + time.Minute) * 60 + time.Second) * TimeSpan.TicksPerSecond + time.Nanosecond / 100);

    public Guid GenerateUuid(int? version)
    {
        CheckLive();
        var bytes = new byte[16];
        Translate(_lib.GenerateUuid(_tc, version ?? 0, version.HasValue, bytes), "generate_uuid");
        return new Guid(bytes, bigEndian: true);
    }

    public byte[] GenerateIpv4()
    {
        CheckLive();
        var bytes = new byte[4];
        Translate(_lib.GenerateIpv4(_tc, bytes), "generate_ipv4");
        return bytes;
    }

    public byte[] GenerateIpv6()
    {
        CheckLive();
        var bytes = new byte[16];
        Translate(_lib.GenerateIpv6(_tc, bytes), "generate_ipv6");
        return bytes;
    }

    public StringGeneratorHandle TextGenerator(ulong minSize, ulong maxSize, string? codec, uint minCodepoint, uint maxCodepoint, IReadOnlyList<string>? categories, IReadOnlyList<string>? excludeCategories, string? includeCharacters, string? excludeCharacters)
    {
        CheckLive();
        Translate(_lib.StringGeneratorText(minSize, maxSize, codec, minCodepoint, maxCodepoint, categories, excludeCategories, includeCharacters, excludeCharacters, out var handle), "string_generator_text");
        return new StringGeneratorHandle(_lib, handle);
    }

    public StringGeneratorHandle RegexGenerator(string pattern, bool fullmatch, StringGeneratorHandle? alphabet)
    {
        CheckLive();
        try
        {
            Translate(_lib.StringGeneratorRegex(pattern, fullmatch, alphabet?.Handle ?? 0, out var handle), "string_generator_regex");
            return new StringGeneratorHandle(_lib, handle);
        }
        finally
        {
            GC.KeepAlive(alphabet);
        }
    }

    public StringGeneratorHandle EmailGenerator()
    {
        CheckLive();
        Translate(_lib.StringGeneratorEmail(out var handle), "string_generator_email");
        return new StringGeneratorHandle(_lib, handle);
    }

    public StringGeneratorHandle UrlGenerator()
    {
        CheckLive();
        Translate(_lib.StringGeneratorUrl(out var handle), "string_generator_url");
        return new StringGeneratorHandle(_lib, handle);
    }

    public StringGeneratorHandle DomainGenerator(ulong maxLength)
    {
        CheckLive();
        Translate(_lib.StringGeneratorDomain(maxLength, out var handle), "string_generator_domain");
        return new StringGeneratorHandle(_lib, handle);
    }

    public bool OwnsStringGenerator(StringGeneratorHandle generator) => ReferenceEquals(generator.Lib, _lib);

    public GrammarHandle Grammar(string text, string? start, ulong maxSize, uint maxDepth, StringGeneratorHandle? alphabet, IReadOnlyList<string> explicitNames, Func<string, string?> files)
    {
        CheckLive();
        Translate(_lib.GrammarBuilderNew(text, out var builder), "grammar_builder_new");
        try
        {
            for (ulong index = 0; ; index++)
            {
                var rc = _lib.GrammarBuilderFile(builder, index, out var path);
                if (rc == Abi.ENotFound)
                {
                    break;
                }
                Translate(rc, "grammar_builder_file");
                Translate(_lib.GrammarBuilderProvide(builder, index, files(path)), "grammar_builder_provide");
            }
            Translate(_lib.GrammarNew(builder, start, maxSize, maxDepth, alphabet?.Handle ?? 0, explicitNames, out var grammar), "grammar_new");
            return new GrammarHandle(_lib, grammar);
        }
        finally
        {
            _lib.GrammarBuilderFree(builder);
            GC.KeepAlive(alphabet);
        }
    }

    public string GenerateGrammar(GrammarHandle grammar, Func<int, string> terminal)
    {
        CheckLive();
        var draw = _lib.GrammarDrawNew(grammar.Handle, 0);
        try
        {
            int rc;
            while ((rc = _lib.GrammarDrawNext(_tc, draw, out var index)) == Abi.Ok)
            {
                Translate(_lib.GrammarDrawProvide(draw, terminal(checked((int)index))), "grammar_draw_provide");
            }
            Translate(rc == Abi.ENotFound ? Abi.Ok : rc, "grammar_draw_next");
            Translate(_lib.GrammarDrawResult(draw, out var value), "grammar_draw_result");
            return value;
        }
        finally
        {
            _lib.GrammarDrawFree(draw);
            GC.KeepAlive(grammar);
        }
    }

    public bool OwnsGrammar(GrammarHandle grammar) => ReferenceEquals(grammar.Lib, _lib);

    public void StartSpan(ulong label)
    {
        CheckLive();
        Translate(_lib.StartSpan(_tc, label), "start_span");
    }

    public void StopSpan(bool discard)
    {
        if (_aborted.Value || _unwinding.Value)
        {
            return;
        }
        Translate(_lib.StopSpan(_tc, discard), "stop_span");
    }

    public nint NewCollection(ulong minSize, ulong maxSize)
    {
        CheckLive();
        Translate(_lib.NewCollection(_tc, minSize, maxSize, out var id), "new_collection");
        return id;
    }

    public bool CollectionMore(nint id)
    {
        CheckLive();
        Translate(_lib.CollectionMore(_tc, id, out var more), "collection_more");
        return more;
    }

    public void CollectionReject(nint id, string? why)
    {
        CheckLive();
        Translate(_lib.CollectionReject(_tc, id, why), "collection_reject");
    }

    public void CollectionFree(nint id) => _lib.CollectionFree(id);

    public nint NewRepeat()
    {
        CheckLive();
        Translate(_lib.NewRepeat(_tc, out var id), "new_repeat");
        return id;
    }

    public bool RepeatMore(nint id)
    {
        CheckLive();
        Translate(_lib.RepeatMore(_tc, id, out var more), "repeat_more");
        return more;
    }

    public void RepeatFailed(nint id)
    {
        CheckLive();
        Translate(_lib.RepeatFailed(_tc, id), "repeat_failed");
    }

    public void RepeatFree(nint id) => _lib.RepeatFree(id);

    public nint NewPool()
    {
        CheckLive();
        Translate(_lib.NewPool(_tc, out var id), "new_pool");
        lock (_pools)
        {
            _pools.Add(id);
        }
        return id;
    }

    public void ReleasePools()
    {
        lock (_pools)
        {
            foreach (var pool in _pools)
            {
                _lib.PoolFree(pool);
            }
            _pools.Clear();
        }
    }

    public void ReleaseThreadContext() => _lib.ReleaseThreadContext();

    public long PoolAdd(nint poolId)
    {
        CheckLive();
        Translate(_lib.PoolAdd(_tc, poolId, out var id), "pool_add");
        return id;
    }

    public long PoolGenerate(nint poolId, bool consume)
    {
        CheckLive();
        Translate(_lib.PoolGenerate(_tc, poolId, consume, out var id), "pool_generate");
        return id;
    }

    public void PoolRestore(nint poolId, long variableId)
    {
        if (_aborted.Value)
        {
            return;
        }
        Translate(_lib.PoolRestore(_tc, poolId, variableId), "pool_restore");
    }

    public nint NewRecursion(ulong maxDepth, ulong maxLeaves)
    {
        CheckLive();
        Translate(_lib.NewRecursion(_tc, maxDepth, maxLeaves, out var recursion), "new_recursion");
        return recursion;
    }

    public bool RecursionBranch(nint recursion, ulong depth)
    {
        CheckLive();
        Translate(_lib.RecursionBranch(_tc, recursion, depth, out var branch), "recursion_branch");
        return branch;
    }

    public bool RecursionLeaf(nint recursion)
    {
        CheckLive();
        var rc = _lib.RecursionLeaf(_tc, recursion);
        if (rc == Abi.ERetry)
        {
            _unwinding.Value = true;
            return false;
        }
        Translate(rc, "recursion_leaf");
        return true;
    }

    public void RecursionRetry(nint recursion)
    {
        _unwinding.Value = false;
        CheckLive();
        Translate(_lib.RecursionRetry(_tc, recursion), "recursion_retry");
    }

    public bool RecursionFinish(nint recursion)
    {
        CheckLive();
        var rc = _lib.RecursionFinish(_tc, recursion);
        if (rc == Abi.ERetry)
        {
            return false;
        }
        Translate(rc, "recursion_finish");
        return true;
    }

    public void RecursionFree(nint recursion) => _lib.RecursionFree(recursion);

    public StateMachineInfo NewStateMachine(IReadOnlyList<string> ruleNames, long[] ruleGroups, double[]? ruleWeights, IReadOnlyList<string> invariantNames, bool[] invariantAlwaysCheck, long minConcurrency, long maxConcurrency, long stepCount)
    {
        CheckLive();
        Translate(_lib.NewStateMachine(_tc, ruleNames, ruleGroups, ruleWeights, invariantNames, invariantAlwaysCheck, minConcurrency, maxConcurrency, stepCount, out var id, out var concurrency), "new_state_machine");
        return new StateMachineInfo(id, checked((int)concurrency));
    }

    public long StateMachineNextGroup(nint stateMachineId)
    {
        CheckLive();
        Translate(_lib.StateMachineNextGroup(_tc, stateMachineId, out var group), "state_machine_next_group");
        return group;
    }

    public long StateMachineNextRule(nint stateMachineId, long workerIndex)
    {
        CheckLive();
        Translate(_lib.StateMachineNextRule(_tc, stateMachineId, workerIndex, out var index), "state_machine_next_rule");
        return index;
    }

    public void StateMachineRuleRejected(nint stateMachineId, long workerIndex)
    {
        CheckLive();
        Translate(_lib.StateMachineRuleRejected(_tc, stateMachineId, workerIndex), "state_machine_rule_rejected");
    }

    public bool StateMachineShouldCheckInvariant(nint stateMachineId, long invariantIndex)
    {
        CheckLive();
        Translate(_lib.StateMachineShouldCheckInvariant(_tc, stateMachineId, invariantIndex, out var check), "state_machine_should_check_invariant");
        return check;
    }

    public void StateMachineFree(nint stateMachineId) => _lib.StateMachineFree(stateMachineId);

    public IDataSource Clone()
    {
        CheckLive();
        Translate(_lib.TestCaseClone(_tc, out var clone), "test_case_clone");
        return new LiveDataSource(_lib, clone, _pools, new Flag(), new Flag());
    }

    public void SetWorker(long workerIndex)
    {
        CheckNotReleased();
        _lib.TestCaseSetWorker(_tc, workerIndex);
    }

    public IDataSource Block(ulong indent)
    {
        Translate(_lib.TestCaseBlock(_tc, indent, out var block), "test_case_block");
        return new LiveDataSource(_lib, block, _pools, _aborted, _unwinding);
    }

    public void Note(string message)
    {
        CheckNotReleased();
        var rc = _lib.Note(_tc, message);
        if (rc != Abi.EInvalidHandle)
        {
            Translate(rc, "note");
        }
    }

    public Printer Printer(Printer.Regions regions)
    {
        CheckNotReleased();
        Translate(_lib.TestCasePrinter(_tc, out var printer), "test_case_printer");
        return new Printer(_lib, printer, regions);
    }

    public void Release()
    {
        _released = true;
        _lib.TestCaseFree(_tc);
    }

    public void Target(double value, string label)
    {
        CheckLive();
        Translate(_lib.Target(_tc, value, label), "target");
    }

    public void Event(string label)
    {
        CheckLive();
        Translate(_lib.Event(_tc, label), "event");
    }

    public void EventValue(string label, double value)
    {
        CheckLive();
        Translate(_lib.EventValue(_tc, value, label), "event_value");
    }
}
