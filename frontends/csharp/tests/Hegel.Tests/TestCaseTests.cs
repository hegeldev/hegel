using System.Numerics;
using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class TestCaseTests
{
    private sealed class Unprintable : Generator<int>
    {
        public override int DoDraw(TestCase tc) => (int)tc.GenerateInteger(5, 5);
    }

    private static Settings Explicit(params Example[] examples) => Engines.Quiet with { Examples = examples, Verbosity = Verbosity.Normal };

    [Fact]
    public void DrawNamesUseTheLabelAndACounter()
    {
        var failure = Runs.Failing(tc =>
        {
            tc.Draw(Generators.Integers(), "x");
            tc.Draw(Generators.Integers(), "x");
            tc.Draw(Generators.Integers());
            tc.Draw(Generators.Integers());
        });
        Assert.Equal(["var x_1 = 0;", "var x_2 = 0;", "var draw_1 = 0;", "var draw_2 = 0;"], failure.Output);
        Assert.Equal(["x_1", "x_2", "draw_1", "draw_2"], failure.Draws.Keys);
        Assert.All(failure.Draws.Values, value => Assert.Equal(0, value));
    }

    [Fact]
    public void DrawLabelMustNotContainANewline()
    {
        var error = Assert.Throws<HegelUsageException>(() => Property.Run(tc => tc.Draw(Generators.Integers(), "a\nb"), Engines.Quiet, Reporter.Silent));
        Assert.Equal("draw label \"a\\nb\" must not contain a newline", error.Message);
    }

    [Fact]
    public void DrawRequiresAPrintableGenerator()
    {
        var error = Assert.Throws<HegelUsageException>(() => Property.Run(tc => tc.Draw(new Unprintable()), Engines.Quiet, Reporter.Silent));
        Assert.StartsWith("Hegel.Tests.TestCaseTests+Unprintable cannot print the values it draws: derive from PrintableGenerator", error.Message);
        Assert.Contains("tc.DrawSilent(...)", error.Message);
    }

    [Fact]
    public void DrawSilentIsNotPrintedOrRecorded()
    {
        var failure = Runs.Failing(tc =>
        {
            Assert.Equal(5, tc.DrawSilent(new Unprintable()));
            tc.Draw(Generators.Just(1), "shown");
            tc.DrawSilent(Generators.Just(2));
        });
        Assert.Equal(["var shown_1 = 1;"], failure.Output);
        Assert.Equal(["shown_1"], failure.Draws.Keys);
    }

    [Fact]
    public void DrawInsideAGeneratorPrintsAsPartOfTheOuterDraw()
    {
        var pair = Generators.Composite((tc, printer) =>
        {
            var a = tc.Draw(Generators.Just(1));
            var b = tc.DrawAndPrint(Generators.Just(2), printer);
            return (a, b);
        });
        var failure = Runs.Failing(tc => Assert.Equal((1, 2), tc.Draw(pair, "pair")));
        Assert.Equal(["var pair_1 = 2;"], failure.Output);
        Assert.Equal(["pair_1"], failure.Draws.Keys);
    }

    [Fact]
    public void DrawAndPrintRequiresAPrintableGenerator()
    {
        var error = Assert.Throws<HegelUsageException>(() => Property.Run(tc => tc.DrawAndPrint(new Unprintable(), Printer.Noop), Engines.Quiet, Reporter.Silent));
        Assert.Contains("cannot print the values it draws", error.Message);
    }

    [Fact]
    public void OrdinaryRunsAreNotFinalAndPrintNothing()
    {
        var finals = 0;
        var report = Runs.Passing(tc =>
        {
            tc.Draw(Generators.Integers(), "x");
            tc.Note("quiet");
            Assert.Empty(tc.Output());
            if (tc.IsFinal)
            {
                finals++;
            }
        }, Engines.Quiet with { TestCases = 5 });
        Assert.Equal(0, finals);
        Assert.True(report.Statistics.Valid > 0);
    }

    [Fact]
    public void TheFailingReplayIsFinal()
    {
        var finals = 0;
        Runs.Failing(tc =>
        {
            if (tc.IsFinal)
            {
                finals++;
            }
        });
        Assert.True(finals >= 1);
    }

    [Fact]
    public void AssumeAndRejectDiscardTheTestCase()
    {
        var report = Runs.Passing(tc =>
        {
            var x = tc.Draw(Generators.Integers().Min(0).Max(3));
            tc.Assume(x != 1);
            if (x == 2)
            {
                tc.Reject();
            }
            Assert.NotEqual(1, x);
            Assert.NotEqual(2, x);
        }, Engines.Quiet with { TestCases = 30 });
        Assert.True(report.Statistics.Invalid > 0);
        Assert.True(report.Statistics.Valid > 0);
    }

    [Fact]
    public void NotesAppearInTheOutput()
    {
        var failure = Runs.Failing(tc =>
        {
            tc.Note("first\nsecond");
            tc.Draw(Generators.Just("v"), "s");
            tc.Note("third");
        });
        Assert.Equal(["first", "second", "var s_1 = \"v\";", "third"], failure.Output);
    }

    private sealed class CountingLib : DelegatingLibhegel
    {
        internal CountingLib() : base(Engines.Real) { }

        internal int Notes { get; private set; }
        internal int Targets { get; private set; }
        internal int Events { get; private set; }
        internal int EventValues { get; private set; }
        internal int Frees { get; private set; }
        internal int RepeatFailures { get; private set; }
        internal int IntegerRc { get; set; } = Abi.Ok;
        internal bool FailSetWorker { get; set; }

        public override int Note(nint testCase, string text)
        {
            Notes++;
            return base.Note(testCase, text);
        }

        public override int Target(nint testCase, double value, string label)
        {
            Targets++;
            return base.Target(testCase, value, label);
        }

        public override int Event(nint testCase, string label)
        {
            Events++;
            return base.Event(testCase, label);
        }

        public override int EventValue(nint testCase, double value, string label)
        {
            EventValues++;
            return base.EventValue(testCase, value, label);
        }

        public override void TestCaseFree(nint testCase)
        {
            Frees++;
            base.TestCaseFree(testCase);
        }

        public override int RepeatFailed(nint testCase, nint repeat)
        {
            RepeatFailures++;
            return base.RepeatFailed(testCase, repeat);
        }

        public override int GenerateInteger(nint testCase, long min, long max, out long value)
        {
            if (IntegerRc != Abi.Ok)
            {
                value = 0;
                return IntegerRc;
            }
            return base.GenerateInteger(testCase, min, max, out value);
        }

        public override void TestCaseSetWorker(nint testCase, long workerIndex)
        {
            if (FailSetWorker)
            {
                throw new HegelException("no workers");
            }
            base.TestCaseSetWorker(testCase, workerIndex);
        }
    }

    [Fact]
    public void NotesAreSkippedWhenNothingIsEmitted()
    {
        var lib = new CountingLib();
        using var run = new RawRun();
        run.Case(captured: false, lib: lib).Note("dropped");
        Assert.Equal(0, lib.Notes);
        run.Case(captured: false, verbose: true, lib: lib).Note("kept");
        Assert.Equal(1, lib.Notes);
        run.Case(captured: true, lib: lib).Note("kept");
        Assert.Equal(2, lib.Notes);
    }

    [Fact]
    public void TargetEventAndEventValueReachTheEngine()
    {
        var lib = new CountingLib();
        using var run = new RawRun();
        var tc = run.Case(lib: lib);
        tc.Target(1.5);
        tc.Target(2.5, "score");
        tc.Event("happened");
        tc.EventValue("size", 3);
        Assert.Equal(2, lib.Targets);
        Assert.Equal(1, lib.Events);
        Assert.Equal(1, lib.EventValues);
    }

    [Fact]
    public void ClonesShareNamesAndTheirDrawsAreRecorded()
    {
        var failure = Runs.Failing(tc =>
        {
            var clone = tc.Clone();
            var thread = new Thread(() => clone.Draw(Generators.Just(1), "x"));
            thread.Start();
            thread.Join();
            tc.Draw(Generators.Just(2), "x");
        });
        Assert.Equal(["x_1", "x_2"], failure.Draws.Keys.Order());
        Assert.Equal(["var x_1 = 1;", "var x_2 = 2;"], failure.Output);
    }

    [Fact]
    public void ClonesOfExplicitCasesShareTheExample()
    {
        var failure = Runs.Failing(tc =>
        {
            Assert.Equal(1, tc.Clone().Draw(Generators.Integers()));
            Assert.Equal(2, tc.Draw(Generators.Integers()));
            tc.Clone().SetWorker(3);
        }, Explicit(Example.Of(1, 2)));
        Assert.Equal(["var draw_1 = 1;", "var draw_2 = 2;"], failure.Output);
        Assert.Null(failure.ReproduceBlob);
    }

    [Fact]
    public void ExplicitCasesRejectSilentDraws()
    {
        var error = Assert.Throws<InvalidExampleException>(() => Property.Run(tc => tc.DrawSilent(Generators.Integers()), Explicit(Example.Of(1)), Reporter.Silent));
        Assert.Equal("DrawSilent is not supported in explicit test cases", error.Message);
    }

    [Fact]
    public void ExplicitCasesNoteAndIgnoreTargets()
    {
        var failure = Runs.Failing(tc =>
        {
            tc.Note("hello");
            tc.Target(1);
            tc.Event("e");
            tc.EventValue("v", 2);
            Assert.True(tc.IsFinal);
            tc.Draw(Generators.Integers(), "n");
        }, Explicit(Example.Of(7)));
        Assert.Equal(["hello", "var n_1 = 7;"], failure.Output);
        Assert.Equal(7, failure.Draws["n_1"]);
    }

    [Fact]
    public void ExplicitCasesRunRepeatOnce()
    {
        var runs = 0;
        var failure = Runs.Failing(tc =>
        {
            tc.Repeat(() =>
            {
                runs++;
                tc.Draw(Generators.Integers(), "n");
                throw new InvalidOperationException("inside");
            });
        }, Explicit(Example.Of(4)));
        Assert.Equal(1, runs);
        Assert.Equal(["// Repetition #1", "var n_1 = 4;"], failure.Output);
        Assert.IsType<InvalidOperationException>(failure.Exception);
    }

    [Fact]
    public void ExplicitRepeatEndsTheTestCase()
    {
        var after = false;
        var report = Runs.Passing(tc =>
        {
            tc.Repeat(() => tc.Draw(Generators.Integers()));
            after = true;
        }, Explicit(Example.Of(4)) with { TestCases = 1 });
        Assert.False(after);
        Assert.True(report.Passed);
    }

    [Fact]
    public void SetWorkerPrefixesTheOutput()
    {
        var failure = Runs.Failing(tc =>
        {
            tc.SetWorker(2);
            tc.Note("working");
        });
        var line = Assert.Single(failure.Output);
        Assert.Matches(@"^\[worker 2 \+\d+\.\d+ms\] working$", line);
    }

    [Fact]
    public void RepeatRunsTheBodyInIndentedBlocks()
    {
        var failure = Runs.Failing(tc =>
        {
            tc.Repeat(() =>
            {
                var x = tc.Draw(Generators.Integers().Min(0).Max(10), "x");
                Assert.True(x < 5);
            });
        });
        Assert.Equal(["// Repetition #1", "  var x_1 = 5;"], failure.Output);
        Assert.Equal(["x_1"], failure.Draws.Keys);
    }

    [Fact]
    public void RepeatEndsTheTestCase()
    {
        var after = false;
        var repetitions = new List<int>();
        Runs.Passing(tc =>
        {
            var count = 0;
            tc.Repeat(() => count++);
            repetitions.Add(count);
            after = true;
        }, Engines.Quiet with { TestCases = 10 });
        Assert.False(after);
        Assert.Empty(repetitions);
    }

    [Fact]
    public void RejectedRepetitionsAreSkipped()
    {
        var rejected = 0;
        var accepted = 0;
        Runs.Passing(tc =>
        {
            tc.Repeat(() =>
            {
                var x = tc.Draw(Generators.Integers().Min(0).Max(1));
                if (x == 1)
                {
                    rejected++;
                    tc.Reject();
                }
                accepted++;
            });
        }, Engines.Quiet with { TestCases = 30 });
        Assert.True(rejected > 0);
        Assert.True(accepted > 0);
    }

    [Fact]
    public void RepeatPropagatesEngineStopsWithoutMarkingAFailure()
    {
        var lib = new CountingLib();
        using var run = new RawRun();
        var tc = run.Case(lib: lib);
        lib.IntegerRc = Abi.EStopTest;
        Assert.Throws<StopTest>(() => tc.Repeat(() => tc.GenerateInteger(0, 1)));
        Assert.Equal(0, lib.RepeatFailures);
        Assert.True(tc.IsAborted);
    }

    [Fact]
    public void RepeatPropagatesAbortingAssumptions()
    {
        var lib = new CountingLib();
        using var run = new RawRun();
        var tc = run.Case(lib: lib);
        lib.IntegerRc = Abi.EAssume;
        Assert.Throws<AssumeRejected>(() => tc.Repeat(() => tc.GenerateInteger(0, 1)));
        Assert.Equal(0, lib.RepeatFailures);
    }

    [Fact]
    public void RepeatMarksUserFailures()
    {
        var lib = new CountingLib();
        using var run = new RawRun();
        var tc = run.Case(lib: lib);
        Assert.Throws<InvalidOperationException>(() => tc.Repeat(() => throw new InvalidOperationException("user")));
        Assert.Equal(1, lib.RepeatFailures);
        Assert.Throws<HegelUsageException>(() => tc.Repeat(() => throw new HegelUsageException("usage")));
        Assert.Equal(1, lib.RepeatFailures);
    }

    [Fact]
    public void ControlExceptionsAreRecognised()
    {
        Assert.True(TestCase.IsControl(new StopTest()));
        Assert.True(TestCase.IsControl(new AssumeRejected()));
        Assert.True(TestCase.IsControl(new LoopDone()));
        Assert.True(TestCase.IsControl(new HegelUsageException("x")));
        Assert.True(TestCase.IsControl(new HegelException("x")));
        Assert.False(TestCase.IsControl(new InvalidOperationException()));
    }

    [Fact]
    public void BlocksIndentAndAreReleasedUnlessPinned()
    {
        using var run = new RawRun();
        var tc = run.Case();
        tc.Note("top");
        var released = tc.Block(2);
        released.Draw(Generators.Just(1), "a");
        released.EndBlock();
        var error = Assert.Throws<HegelUsageException>(() => released.Draw(Generators.Just(2), "b"));
        Assert.Equal(LiveDataSource.ReleasedMessage, error.Message);
        var pinned = tc.Block(4);
        pinned.Pin();
        pinned.Pin();
        pinned.EndBlock();
        pinned.Draw(Generators.Just(3), "c");
        tc.Pin();
        Assert.Equal(["top", "  var a_1 = 1;", "    var c_1 = 3;"], tc.Output());
        tc.ReleaseAll();
        error = Assert.Throws<HegelUsageException>(() => pinned.Draw(Generators.Just(4), "d"));
        Assert.Equal(LiveDataSource.ReleasedMessage, error.Message);
    }

    [Fact]
    public void ForWorkerGivesAnAttributedClone()
    {
        using var run = new RawRun();
        var tc = run.Case();
        var worker = tc.ForWorker(1);
        worker.Note("from worker");
        worker.Release();
        var line = Assert.Single(tc.Output());
        Assert.Matches(@"^\[worker 1 \+[\d.]+ms\] from worker$", line);
    }

    [Fact]
    public void ForWorkerReleasesTheCloneWhenAttributionFails()
    {
        var lib = new CountingLib { FailSetWorker = true };
        using var run = new RawRun();
        var tc = run.Case(lib: lib);
        Assert.Throws<HegelException>(() => tc.ForWorker(0));
        Assert.Equal(1, lib.Frees);
    }

    [Fact]
    public void ReleaseAllFreesClones()
    {
        var lib = new CountingLib();
        using var run = new RawRun();
        var tc = run.Case(lib: lib);
        tc.Clone();
        tc.Clone();
        tc.ReleaseAll();
        Assert.Equal(2, lib.Frees);
        tc.ReleaseAll();
        Assert.Equal(2, lib.Frees);
    }

    [Fact]
    public void EnginePrimitivesDrawThroughTheSource()
    {
        using var run = new RawRun();
        var tc = run.Case(captured: false);
        Assert.True(tc.GenerateBoolean(1.0));
        Assert.Equal(3, tc.GenerateInteger(3, 3));
        var big = BigInteger.Pow(2, 100);
        Assert.Equal(big, tc.GenerateIntegerBig(big, big));
        Assert.Equal(1.0, tc.GenerateFloat(64, 1.0, 1.0, false, false, false, false, double.Epsilon));
        Assert.Equal(2, tc.GenerateBytes(2, 2).Length);
        var text = tc.TextGenerator(1, 1, null, 'a', 'a', null, null, null, null);
        Assert.Equal("a", tc.GenerateString(text));
        Assert.True(tc.OwnsStringGenerator(text));
        Assert.Equal("b", tc.GenerateString(tc.RegexGenerator("b", true, null)));
        var date = new DateOnly(2001, 2, 3);
        Assert.Equal(date, tc.GenerateDate(date, date));
        var time = new TimeOnly(4, 5, 6, 7);
        Assert.Equal(time, tc.GenerateTime(time, time));
        var dateTime = date.ToDateTime(time);
        Assert.Equal(dateTime, tc.GenerateDatetime(dateTime, dateTime));
        Assert.Equal(4, tc.GenerateUuid(4).ToByteArray(bigEndian: true)[6] >> 4);
        Assert.NotEqual(Guid.Empty, tc.GenerateUuid(null));
        Assert.Equal(4, tc.GenerateIpv4().Length);
        Assert.Equal(16, tc.GenerateIpv6().Length);
        var grammar = tc.Grammar("start: \"x\"", null, 100, 10, null, [], _ => null);
        Assert.True(tc.OwnsGrammar(grammar));
        Assert.Equal("x", tc.GenerateGrammar(grammar, _ => throw new InvalidOperationException()));
        Assert.Equal(8, tc.Span(Label.List, () => 8));
        tc.StartSpan(Label.Set);
        tc.StopSpan(true);
        var collection = tc.NewCollection(1, 1);
        Assert.True(tc.CollectionMore(collection));
        tc.CollectionReject(collection, "duplicate");
        Assert.True(tc.CollectionMore(collection));
        tc.CollectionReject(collection, null);
        tc.CollectionFree(collection);
        var recursion = tc.NewRecursion(0, 1);
        Assert.False(tc.RecursionBranch(recursion, 0));
        Assert.True(tc.RecursionLeaf(recursion));
        Assert.True(tc.RecursionFinish(recursion));
        tc.RecursionFree(recursion);
        var pool = tc.NewPool();
        var id = tc.PoolAdd(pool);
        Assert.Equal(id, tc.PoolGenerate(pool, true));
        tc.PoolRestore(pool, id);
        Assert.Equal(id, tc.PoolGenerate(pool, false));
        var machine = tc.NewStateMachine(["r"], [0], null, ["i"], [true], 1, 1, 5);
        Assert.Equal(1, machine.Concurrency);
        if (tc.StateMachineNextGroup(machine.Id) != Abi.StateMachineDone && tc.StateMachineNextRule(machine.Id, 0) != Abi.StateMachineDone)
        {
            tc.StateMachineRuleRejected(machine.Id, 0);
        }
        Assert.True(tc.StateMachineShouldCheckInvariant(machine.Id, 0));
        tc.StateMachineFree(machine.Id);
        tc.ReleaseThreadContext();
    }

    [Fact]
    public void VerboseNonFinalDrawsArePrintedButNotRecorded()
    {
        using var run = new RawRun();
        var tc = run.Case(captured: false, verbose: true);
        Assert.Equal(1, tc.Draw(Generators.Just(1), "x"));
        Assert.Empty(tc.Draws());
        Assert.Equal(["var x_1 = 1;"], tc.Output());
    }

    [Fact]
    public void DrawNamesThatCollideAcrossBlocksAreSuffixed()
    {
        using var run = new RawRun();
        var tc = run.Case();
        tc.Draw(Generators.Just(1), "a");
        var block = tc.Block(2);
        block.Draw(Generators.Just(2), "a");
        block.Draw(Generators.Just(3), "a");
        block.EndBlock();
        Assert.Equal(["a_1", "a_1_2", "a_2"], tc.Draws().Keys);
        Assert.Equal([1, 2, 3], tc.Draws().Values.Cast<int>());
        Assert.Equal(["var a_1 = 1;", "  var a_1 = 2;", "  var a_2 = 3;"], tc.Output());
    }

    [Fact]
    public void InternetGeneratorsDrawThroughTheSource()
    {
        Assert.Contains('@', Runs.Accepted(tc => tc.GenerateString(tc.EmailGenerator())));
        Assert.Contains("://", Runs.Accepted(tc => tc.GenerateString(tc.UrlGenerator())));
        Assert.InRange(Runs.Accepted(tc => tc.GenerateString(tc.DomainGenerator(10))).Length, 1, 10);
    }

    [Fact]
    public void RecursionRetryIsForwarded()
    {
        using var run = new RawRun();
        var tc = run.Case(captured: false);
        var recursion = tc.NewRecursion(3, 2);
        tc.RecursionBranch(recursion, 0);
        tc.RecursionRetry(recursion);
        tc.RecursionFree(recursion);
    }
}
