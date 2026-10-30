using System.Numerics;
using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class LiveDataSourceTests
{
    private sealed class RcLib : DelegatingLibhegel
    {
        internal RcLib() : base(Engines.Real) { }

        internal int IntegerRc { get; set; } = Abi.Ok;
        internal int NoteRc { get; set; } = Abi.Ok;
        internal int LeafRc { get; set; } = Abi.Ok;
        internal int FinishRc { get; set; } = Abi.Ok;
        internal int RetryRc { get; set; } = Abi.Ok;
        internal int DrawNextRc { get; set; } = Abi.Ok;
        internal int StopSpans { get; private set; }
        internal List<(double Value, string Label)> EventValues { get; } = [];

        public override string LastErrorMessage() => "fake error";

        public override int GenerateInteger(nint testCase, long min, long max, out long value)
        {
            if (IntegerRc != Abi.Ok)
            {
                value = 0;
                return IntegerRc;
            }
            return base.GenerateInteger(testCase, min, max, out value);
        }

        public override int Note(nint testCase, string text) => NoteRc == Abi.Ok ? base.Note(testCase, text) : NoteRc;

        public override int StopSpan(nint testCase, bool discard)
        {
            StopSpans++;
            return base.StopSpan(testCase, discard);
        }

        public override int RecursionLeaf(nint testCase, nint recursion) => LeafRc == Abi.Ok ? base.RecursionLeaf(testCase, recursion) : LeafRc;

        public override int RecursionFinish(nint testCase, nint recursion) => FinishRc == Abi.Ok ? base.RecursionFinish(testCase, recursion) : FinishRc;

        public override int RecursionRetry(nint testCase, nint recursion) => RetryRc == Abi.Ok ? base.RecursionRetry(testCase, recursion) : RetryRc;

        public override int GrammarDrawNext(nint testCase, nint draw, out ulong terminal)
        {
            if (DrawNextRc != Abi.Ok)
            {
                terminal = 0;
                return DrawNextRc;
            }
            return base.GrammarDrawNext(testCase, draw, out terminal);
        }

        public override int EventValue(nint testCase, double value, string label)
        {
            EventValues.Add((value, label));
            return base.EventValue(testCase, value, label);
        }
    }

    [Fact]
    public void StopTestAbortsTheSource()
    {
        var lib = new RcLib { IntegerRc = Abi.EStopTest };
        using var run = new RawRun();
        var source = run.Source(lib);
        Assert.False(source.IsAborted);
        Assert.Throws<StopTest>(() => source.GenerateInteger(0, 1));
        Assert.True(source.IsAborted);
        Assert.Throws<StopTest>(() => source.GenerateBoolean(0.5));
        Assert.Throws<StopTest>(() => source.Clone());
        source.StopSpan(false);
        source.PoolRestore(0, 0);
        Assert.Equal(0, lib.StopSpans);
    }

    [Fact]
    public void FailedAssumptionsAbortTheSource()
    {
        var lib = new RcLib { IntegerRc = Abi.EAssume };
        using var run = new RawRun();
        var source = run.Source(lib);
        Assert.Throws<AssumeRejected>(() => source.GenerateInteger(0, 1));
        Assert.True(source.IsAborted);
    }

    [Fact]
    public void InvalidArgumentsAreUsageErrors()
    {
        var lib = new RcLib { IntegerRc = Abi.EInvalidArg };
        using var run = new RawRun();
        var error = Assert.Throws<HegelUsageException>(() => run.Source(lib).GenerateInteger(0, 1));
        Assert.Equal("fake error", error.Message);
    }

    [Fact]
    public void ConcurrentUseExplainsItself()
    {
        var lib = new RcLib { IntegerRc = Abi.EConcurrentUse };
        using var run = new RawRun();
        var error = Assert.Throws<HegelException>(() => run.Source(lib).GenerateInteger(0, 1));
        Assert.Equal("hegel_generate_integer: the test-case handle was used from two threads at once. In a concurrent state machine, draw only through the TestCase handed to the rule and share generated values through a ConcurrentPool.", error.Message);
    }

    [Fact]
    public void OtherErrorsNameTheCall()
    {
        var lib = new RcLib { IntegerRc = Abi.EInternal };
        using var run = new RawRun();
        var error = Assert.Throws<HegelException>(() => run.Source(lib).GenerateInteger(0, 1));
        Assert.Equal("hegel_generate_integer failed (rc=-8): fake error", error.Message);
    }

    [Fact]
    public void ReleasedSourcesRefuseEverything()
    {
        using var run = new RawRun();
        var source = run.Source().Clone();
        source.Release();
        Assert.Equal(LiveDataSource.ReleasedMessage, Assert.Throws<HegelUsageException>(() => source.GenerateInteger(0, 1)).Message);
        Assert.Equal(LiveDataSource.ReleasedMessage, Assert.Throws<HegelUsageException>(() => source.Note("x")).Message);
        Assert.Equal(LiveDataSource.ReleasedMessage, Assert.Throws<HegelUsageException>(() => source.SetWorker(0)).Message);
        Assert.Equal(LiveDataSource.ReleasedMessage, Assert.Throws<HegelUsageException>(() => source.Printer(new Printer.Regions())).Message);
    }

    [Fact]
    public void BigIntegersUseTheFastPathWithinLongRange()
    {
        var lib = new RcLib { IntegerRc = Abi.EInternal };
        using var run = new RawRun();
        var source = run.Source(lib);
        Assert.Throws<HegelException>(() => source.GenerateIntegerBig(long.MinValue, long.MaxValue));
        var huge = BigInteger.Pow(2, 70);
        Assert.Equal(huge, source.GenerateIntegerBig(huge, huge));
        Assert.Equal(-huge, source.GenerateIntegerBig(-huge, -huge));
        Assert.InRange(source.GenerateIntegerBig(-huge, huge), -huge, huge);
        Assert.InRange(source.GenerateIntegerBig(0, huge), 0, huge);
    }

    [Fact]
    public void TemporalValuesRoundTripThroughTheNativeStructs()
    {
        var time = new TimeOnly(23, 59, 58).Add(TimeSpan.FromTicks(1234567));
        var native = LiveDataSource.ToNative(time);
        Assert.Equal(23, native.Hour);
        Assert.Equal(59, native.Minute);
        Assert.Equal(58, native.Second);
        Assert.Equal(123456700u, native.Nanosecond);
        Assert.Equal(time, LiveDataSource.FromNative(native));
        var date = new DateOnly(1999, 12, 31);
        var nativeDate = LiveDataSource.ToNative(date);
        Assert.Equal(1999, nativeDate.Year);
        Assert.Equal(12, nativeDate.Month);
        Assert.Equal(31, nativeDate.Day);
        Assert.Equal(date, LiveDataSource.FromNative(nativeDate));
        using var run = new RawRun();
        var source = run.Source();
        Assert.Equal(time, source.GenerateTime(time, time));
        Assert.Equal(date, source.GenerateDate(date, date));
        var dateTime = date.ToDateTime(time);
        var drawn = source.GenerateDatetime(dateTime, dateTime);
        Assert.Equal(dateTime, drawn);
        Assert.Equal(DateTimeKind.Unspecified, drawn.Kind);
        Assert.InRange(source.GenerateDate(DateOnly.MinValue, DateOnly.MaxValue), DateOnly.MinValue, DateOnly.MaxValue);
        Assert.InRange(source.GenerateTime(TimeOnly.MinValue, TimeOnly.MaxValue), TimeOnly.MinValue, TimeOnly.MaxValue);
        Assert.InRange(source.GenerateDatetime(DateTime.MinValue, DateTime.MaxValue), DateTime.MinValue, DateTime.MaxValue);
    }

    [Fact]
    public void UuidsAreBigEndian()
    {
        using var run = new RawRun();
        var source = run.Source();
        var v1 = source.GenerateUuid(1).ToByteArray(bigEndian: true);
        Assert.Equal(1, v1[6] >> 4);
        Assert.Equal(0b10, v1[8] >> 6);
        var v4 = source.GenerateUuid(4);
        Assert.Equal(4, v4.ToByteArray(bigEndian: true)[6] >> 4);
        Assert.Equal(4, v4.ToString("D")[14] - '0');
        Assert.NotEqual(Guid.Empty, source.GenerateUuid(null));
    }

    [Fact]
    public void BytesAndStringsMayBeEmpty()
    {
        using var run = new RawRun();
        var source = run.Source();
        Assert.Empty(source.GenerateBytes(0, 0));
        Assert.Equal(3, source.GenerateBytes(3, 3).Length);
        Assert.Equal("", source.GenerateString(source.TextGenerator(0, 0, null, 0, Abi.NoMaxCodepoint, null, null, null, null)));
        var letters = source.TextGenerator(2, 2, "ascii", 0, Abi.NoMaxCodepoint, ["Lu"], ["Ll"], "", "A");
        var drawn = source.GenerateString(letters);
        Assert.Equal(2, drawn.Length);
        Assert.All(drawn, c => Assert.True(char.IsUpper(c) && c != 'A' && c < 128));
        var digits = source.TextGenerator(1, 1, null, '0', '9', null, null, null, null);
        Assert.Matches("^[0-9]$", source.GenerateString(digits));
    }

    [Fact]
    public void StringGeneratorsAreOwnedByTheirEngine()
    {
        using var run = new RawRun();
        var source = run.Source();
        var regex = source.RegexGenerator("[a-c]{2}", true, null);
        Assert.Matches("^[a-c]{2}$", source.GenerateString(regex));
        var alphabet = source.TextGenerator(1, 1, null, 'x', 'x', null, null, null, null);
        var constrained = source.RegexGenerator(".", true, alphabet);
        Assert.Equal("x", source.GenerateString(constrained));
        Assert.True(source.OwnsStringGenerator(regex));
        Assert.False(source.OwnsStringGenerator(new StringGeneratorHandle(new DelegatingLibhegel(Engines.Real), 0)));
    }

    [Fact]
    public void InternetGeneratorsMayRejectAndRetry()
    {
        Assert.Contains("@", Runs.Accepted(tc => tc.GenerateString(tc.EmailGenerator())));
        Assert.Contains("://", Runs.Accepted(tc => tc.GenerateString(tc.UrlGenerator())));
        Assert.InRange(Runs.Accepted(tc => tc.GenerateString(tc.DomainGenerator(8))).Length, 1, 8);
    }

    [Fact]
    public void GrammarsLoadImportsThroughTheCallback()
    {
        using var run = new RawRun();
        var source = run.Source();
        var requested = new List<string>();
        var grammar = source.Grammar("start: INT\n%import common.INT", null, 100, 10, null, [], path =>
        {
            requested.Add(path);
            return null;
        });
        Assert.Equal(["common.lark"], requested);
        Assert.True(source.OwnsGrammar(grammar));
        Assert.False(source.OwnsGrammar(new GrammarHandle(new DelegatingLibhegel(Engines.Real), 0)));
        Assert.Matches("^[0-9]+$", source.GenerateGrammar(grammar, _ => throw new InvalidOperationException()));
        var relative = source.Grammar("start: WORD\n%import .words.WORD", null, 100, 10, null, [], path => path == "words.lark" ? "WORD: \"hi\"" : null);
        Assert.Equal("hi", source.GenerateGrammar(relative, _ => throw new InvalidOperationException()));
    }

    [Fact]
    public void GrammarsDrawExplicitTerminalsThroughTheCallback()
    {
        using var run = new RawRun();
        var source = run.Source();
        var letters = source.TextGenerator(1, 1, null, '-', 'z', null, null, null, null);
        var grammar = source.Grammar("start: NAME \"-\" NAME\nNAME: /[a-z]+/", "start", 100, 10, letters, ["NAME"], _ => null);
        var indices = new List<int>();
        var text = source.GenerateGrammar(grammar, index =>
        {
            indices.Add(index);
            return "name";
        });
        Assert.Equal("name-name", text);
        Assert.Equal([0, 0], indices);
    }

    [Fact]
    public void InvalidGrammarsAreUsageErrors()
    {
        using var run = new RawRun();
        var source = run.Source();
        var error = Assert.Throws<HegelUsageException>(() => source.Grammar("start: (", null, 100, 10, null, [], _ => null));
        Assert.NotEqual("", error.Message);
    }

    [Fact]
    public void GrammarDrawErrorsAreReported()
    {
        var lib = new RcLib { DrawNextRc = Abi.EInternal };
        using var run = new RawRun();
        var source = run.Source(lib);
        var grammar = source.Grammar("start: \"a\"", null, 100, 10, null, [], _ => null);
        var error = Assert.Throws<HegelException>(() => source.GenerateGrammar(grammar, _ => ""));
        Assert.Equal("hegel_grammar_draw_next failed (rc=-8): fake error", error.Message);
    }

    [Fact]
    public void SpansAreSkippedWhileUnwinding()
    {
        var lib = new RcLib { LeafRc = Abi.ERetry };
        using var run = new RawRun();
        var source = run.Source(lib);
        source.StartSpan(Label.Recursive);
        source.StopSpan(false);
        Assert.Equal(1, lib.StopSpans);
        var recursion = source.NewRecursion(2, 2);
        Assert.False(source.RecursionLeaf(recursion));
        source.StopSpan(false);
        Assert.Equal(1, lib.StopSpans);
        lib.LeafRc = Abi.Ok;
        source.RecursionRetry(recursion);
        source.StopSpan(false);
        Assert.Equal(2, lib.StopSpans);
        source.RecursionFree(recursion);
    }

    [Fact]
    public void RecursionFinishReportsMispricing()
    {
        var lib = new RcLib { FinishRc = Abi.ERetry };
        using var run = new RawRun();
        var source = run.Source(lib);
        var recursion = source.NewRecursion(2, 2);
        Assert.False(source.RecursionFinish(recursion));
        lib.FinishRc = Abi.EInternal;
        Assert.Throws<HegelException>(() => source.RecursionFinish(recursion));
        lib.RetryRc = Abi.EAssume;
        Assert.Throws<AssumeRejected>(() => source.RecursionRetry(recursion));
        source.RecursionFree(recursion);
    }

    [Fact]
    public void RecursionProtocolWithTheRealEngine()
    {
        using var run = new RawRun();
        var source = run.Source();
        var recursion = source.NewRecursion(2, 4);
        var leaves = 0;
        bool Draw(ulong depth)
        {
            if (source.RecursionBranch(recursion, depth))
            {
                return Draw(depth + 1) && Draw(depth + 1);
            }
            if (!source.RecursionLeaf(recursion))
            {
                return false;
            }
            leaves++;
            return true;
        }
        while (true)
        {
            leaves = 0;
            if (!Draw(0))
            {
                source.RecursionRetry(recursion);
                continue;
            }
            if (source.RecursionFinish(recursion))
            {
                break;
            }
        }
        source.RecursionFree(recursion);
        Assert.InRange(leaves, 1, 4);
    }

    [Fact]
    public void PoolsRememberTheirVariables()
    {
        using var run = new RawRun();
        var source = run.Source();
        var pool = source.NewPool();
        var first = source.PoolAdd(pool);
        var second = source.PoolAdd(pool);
        Assert.NotEqual(first, second);
        var consumed = source.PoolGenerate(pool, true);
        Assert.Contains(consumed, new[] { first, second });
        var remaining = source.PoolGenerate(pool, false);
        Assert.NotEqual(consumed, remaining);
        source.PoolRestore(pool, consumed);
        Assert.Throws<HegelUsageException>(() => source.PoolRestore(pool, consumed));
        source.ReleasePools();
        source.ReleasePools();
    }

    [Fact]
    public void DrawingFromAnEmptyPoolIsARejection()
    {
        using var run = new RawRun();
        var source = run.Source();
        var pool = source.NewPool();
        Assert.Throws<AssumeRejected>(() => source.PoolGenerate(pool, true));
        Assert.True(source.IsAborted);
        source.PoolRestore(pool, 123);
    }

    [Fact]
    public void StateMachinesFollowTheProtocol()
    {
        using var run = new RawRun();
        var source = run.Source();
        var machine = source.NewStateMachine(["a", "b"], [0, 1], [1.0, 2.0], ["inv", "sampled"], [true, false], 1, 1, 3);
        Assert.Equal(1, machine.Concurrency);
        var rejected = false;
        long group;
        while ((group = source.StateMachineNextGroup(machine.Id)) != Abi.StateMachineDone)
        {
            Assert.Contains(group, new long[] { 0, 1 });
            long rule;
            while ((rule = source.StateMachineNextRule(machine.Id, 0)) != Abi.StateMachineDone)
            {
                Assert.Equal(group, rule);
                if (!rejected)
                {
                    rejected = true;
                    source.StateMachineRuleRejected(machine.Id, 0);
                    Assert.Throws<HegelUsageException>(() => source.StateMachineRuleRejected(machine.Id, 0));
                }
            }
            Assert.True(source.StateMachineShouldCheckInvariant(machine.Id, 0));
            source.StateMachineShouldCheckInvariant(machine.Id, 1);
        }
        Assert.Throws<HegelUsageException>(() => source.StateMachineShouldCheckInvariant(machine.Id, 2));
        source.StateMachineFree(machine.Id);
        Assert.Throws<HegelUsageException>(() => source.NewStateMachine([], [], null, [], [], 1, 1, 3));
    }

    [Fact]
    public void ClonesHaveTheirOwnFlagsAndBlocksShareThem()
    {
        var lib = new RcLib();
        using var run = new RawRun();
        var source = run.Source(lib);
        var clone = source.Clone();
        lib.IntegerRc = Abi.EStopTest;
        Assert.Throws<StopTest>(() => clone.GenerateInteger(0, 1));
        Assert.True(clone.IsAborted);
        Assert.False(source.IsAborted);
        var block = source.Block(2);
        Assert.Throws<StopTest>(() => block.GenerateInteger(0, 1));
        Assert.True(source.IsAborted);
        block.Release();
        clone.Release();
    }

    [Fact]
    public void NotesTolerateDeadRegions()
    {
        var lib = new RcLib { NoteRc = Abi.EInvalidHandle };
        using var run = new RawRun();
        var source = run.Source(lib);
        source.Note("ignored");
        lib.NoteRc = Abi.EInternal;
        Assert.Throws<HegelException>(() => source.Note("reported"));
    }

    [Fact]
    public void NotesAndPrintersWriteToTheRegion()
    {
        using var run = new RawRun();
        var source = run.Source();
        source.Note("one");
        var regions = new Printer.Regions();
        var printer = source.Printer(regions);
        printer.Text("two");
        printer.HardBreak();
        var root = source.Printer(regions);
        Assert.Equal("one\ntwo\n", root.Read());
        root.Free();
        printer.Free();
    }

    [Fact]
    public void WorkersMustNotBeNegative()
    {
        using var run = new RawRun();
        var source = run.Source();
        source.SetWorker(0);
        Assert.Throws<HegelException>(() => source.SetWorker(-1));
    }

    [Fact]
    public void TargetsAndEventsReachTheEngine()
    {
        var lib = new RcLib();
        using var run = new RawRun();
        var source = run.Source(lib);
        source.Target(1.0, "");
        source.Target(2.0, "named");
        source.Event("event");
        source.EventValue("value", 3.5);
        Assert.Equal([(3.5, "value")], lib.EventValues);
        Assert.Throws<HegelUsageException>(() => source.Target(double.NaN, "nan"));
    }

    [Fact]
    public void CollectionsAndRepeatsTalkToTheEngine()
    {
        using var run = new RawRun();
        var source = run.Source();
        var collection = source.NewCollection(2, 2);
        Assert.True(source.CollectionMore(collection));
        source.CollectionReject(collection, "why");
        Assert.True(source.CollectionMore(collection));
        Assert.True(source.CollectionMore(collection));
        Assert.False(source.CollectionMore(collection));
        source.CollectionFree(collection);
        var repeat = source.NewRepeat();
        if (source.RepeatMore(repeat))
        {
            source.RepeatFailed(repeat);
        }
        source.RepeatFree(repeat);
    }

    [Fact]
    public void FloatsAndBooleansRespectTheirBounds()
    {
        using var run = new RawRun();
        var source = run.Source();
        Assert.False(source.GenerateBoolean(0.0));
        Assert.True(source.GenerateBoolean(1.0));
        Assert.Equal(2.5, source.GenerateFloat(64, 2.5, 2.5, false, false, false, false, double.Epsilon));
        var single = source.GenerateFloat(32, 0, 1, false, false, true, true, float.Epsilon);
        Assert.InRange(single, 0, 1);
        Assert.NotEqual(0, single);
        Assert.NotEqual(1, single);
        Assert.Equal(4, source.GenerateIpv4().Length);
        Assert.Equal(16, source.GenerateIpv6().Length);
        source.ReleaseThreadContext();
    }
}
