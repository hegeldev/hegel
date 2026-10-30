using System.Collections.Concurrent;
using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class StatefulTests
{
    private static Settings Quick => Engines.Quiet with { TestCases = 20 };

    private sealed class CapturingReporter : Reporter
    {
        internal readonly List<string> Lines = [];

        public override void CaseOutput(string line, bool finalReplay) => Lines.Add(line);
    }

    private sealed class Counter
    {
        internal int N;

        [Rule]
        internal void Increment(TestCase tc) => N++;

        [Rule]
        internal void Decrement(TestCase tc)
        {
            tc.Assume(N > 0);
            N--;
        }

        [Invariant]
        internal void NonNegative(TestCase tc) => Assert.True(N >= 0);
    }

    [Fact]
    public void CounterMachineHoldsUnderRandomRules()
    {
        var report = Property.Test(tc => Stateful.Run(new Counter(), tc), Quick);
        Assert.True(report.Passed);
    }

    private sealed class StepCounter
    {
        internal int Steps;

        [Rule]
        internal void Step(TestCase tc) => Steps++;
    }

    [Fact]
    public void StepCountBoundsTheStepsTaken()
    {
        var most = 0;
        Property.Test(tc =>
        {
            var machine = new StepCounter();
            Stateful.Run(machine, tc, 5);
            most = Math.Max(most, machine.Steps);
        }, Quick);
        Assert.InRange(most, 1, 5);
    }

    [Fact]
    public void DefaultOptionsAreFiftySequentialSteps()
    {
        var options = new StatefulOptions();
        Assert.Equal(50, options.StepCount);
        Assert.Equal(Stateful.DefaultStepCount, options.StepCount);
        Assert.Equal(1, options.MinConcurrency);
        Assert.Equal(1, options.MaxConcurrency);
    }

    [Fact]
    public void OptionsRejectNonPositiveValues()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new StatefulOptions { StepCount = 0 });
        Assert.Throws<ArgumentOutOfRangeException>(() => new StatefulOptions { MinConcurrency = 0 });
        Assert.Throws<ArgumentOutOfRangeException>(() => new StatefulOptions { MaxConcurrency = -1 });
    }

    [Fact]
    public void NullArgumentsAreRejected()
    {
        Assert.Throws<ArgumentNullException>(() => Stateful.Run(null!, null!));
        Assert.Throws<ArgumentNullException>(() => Stateful.Run(new Counter(), null!));
        var e = Assert.Throws<ArgumentNullException>(() => Property.Test(tc => Stateful.Run(new Counter(), tc, null!), Quick));
        Assert.Equal("options", e.ParamName);
    }

    [Fact]
    public void MinConcurrencyMustNotExceedMax()
    {
        var options = new StatefulOptions { MinConcurrency = 3, MaxConcurrency = 2 };
        var e = Assert.Throws<ArgumentException>(() => Property.Test(tc => Stateful.Run(new Counter(), tc, options), Quick));
        Assert.StartsWith("MinConcurrency (3) must not exceed MaxConcurrency (2)", e.Message);
    }

    private sealed class NoRules
    {
        [Invariant]
        internal void Fine(TestCase tc) { }
    }

    [Fact]
    public void AMachineWithoutRulesIsRejected()
    {
        var e = Assert.Throws<ArgumentException>(() => Property.Test(tc => Stateful.Run(new NoRules(), tc), Quick));
        Assert.Contains("NoRules has no [Rule] methods; a state machine needs at least one", e.Message);
    }

    private sealed class BadRule
    {
        [Rule]
        internal void Wrong(int x) { }
    }

    private sealed class BadInvariant
    {
        [Rule]
        internal void Fine(TestCase tc) { }

        [Invariant]
        internal void Wrong() { }
    }

    [Fact]
    public void AnnotatedMethodsMustTakeOneTestCase()
    {
        var rule = Assert.Throws<ArgumentException>(() => Property.Test(tc => Stateful.Run(new BadRule(), tc), Quick));
        Assert.Contains("[Rule] method Hegel.Tests.StatefulTests+BadRule.Wrong must take a single TestCase parameter", rule.Message);
        var invariant = Assert.Throws<ArgumentException>(() => Property.Test(tc => Stateful.Run(new BadInvariant(), tc), Quick));
        Assert.Contains("[Invariant] method Hegel.Tests.StatefulTests+BadInvariant.Wrong must take a single TestCase parameter", invariant.Message);
    }

    private sealed class Booming
    {
        private int _calls;

        [Rule]
        internal void Boom(TestCase tc)
        {
            var x = tc.Draw(Generators.Integers(), "x");
            if (++_calls >= 2)
            {
                throw new InvalidOperationException($"boom on {x}");
            }
        }
    }

    [Fact]
    public void AFailingRuleIsReportedWithItsSteps()
    {
        var report = Property.Run(tc => Stateful.Run(new Booming(), tc), Quick);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.IsType<InvalidOperationException>(failure.Exception);
        Assert.StartsWith("InvalidOperationException at StatefulTests.cs:", failure.Origin);
        Assert.Equal(
            ["Step 1: Boom {", "  var x_1 = 0;", "}", "Step 2: Boom {", "  var x_1 = 0;", "}", "Rule Boom failed:"],
            failure.Output);
        Assert.Equal(["x_1", "x_1_2"], failure.Draws.Keys);
        Assert.Throws<InvalidOperationException>(report.ThrowIfFailed);
    }

    private sealed class Capped
    {
        private int _n;

        [Rule]
        internal void Increment(TestCase tc) => _n++;

        [Invariant]
        internal void Small(TestCase tc) => Assert.True(_n <= 2, $"counter reached {_n}");
    }

    [Fact]
    public void AFailingInvariantIsFoundAndNoted()
    {
        var report = Property.Run(tc => Stateful.Run(new Capped(), tc), Quick);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        Assert.Equal("Checking invariants on the initial state.", failure.Output[0]);
        Assert.Equal("Step 1: Increment {", failure.Output[1]);
        Assert.Equal("Invariant Small failed:", failure.Output[^1]);
        Assert.Equal(3, failure.Output.Count(line => line == "}"));
    }

    [Fact]
    public void InvariantsAreCheckedOnTheInitialAndFinalState()
    {
        var reporter = new CapturingReporter();
        var report = Property.Run(tc => Stateful.Run(new Counter(), tc, 3), Quick with { Verbosity = Verbosity.Verbose }, reporter);
        Assert.True(report.Passed);
        Assert.Contains("Checking invariants on the initial state.", reporter.Lines);
        Assert.Contains("Checking invariants on the final state.", reporter.Lines);
        Assert.Contains("Rule stopped early due to violated assumption.", reporter.Lines.Select(l => l.TrimStart()));
    }

    private sealed class Observing
    {
        internal readonly List<int> Seen = [];
        private int _n;

        [Rule]
        internal void Increment(TestCase tc) => _n++;

        [Invariant(AlwaysRun = true)]
        internal void Observe(TestCase tc) => Seen.Add(_n);
    }

    [Fact]
    public void AlwaysRunInvariantsSeeEveryStep()
    {
        var report = Property.Test(tc =>
        {
            var machine = new Observing();
            Stateful.Run(machine, tc, 10);
            var steps = machine.Seen[^1];
            Assert.Equal(Enumerable.Range(0, steps + 1).Append(steps).ToList(), machine.Seen);
        }, Quick);
        Assert.True(report.Passed);
    }

    private sealed class Sampled
    {
        internal int Steps;
        internal int Checks;

        [Rule]
        internal void Step(TestCase tc) => Steps++;

        [Invariant]
        internal void Check(TestCase tc) => Checks++;
    }

    [Fact]
    public void SampledInvariantsRunAtMostOncePerStepPlusTheEnds()
    {
        var report = Property.Test(tc =>
        {
            var machine = new Sampled();
            Stateful.Run(machine, tc);
            Assert.InRange(machine.Checks, 2, machine.Steps + 2);
        }, Quick);
        Assert.True(report.Passed);
    }

    private class Base
    {
        internal int BaseA;
        internal int Hidden;
        internal int B;

        [Rule]
        protected virtual void A(TestCase tc) => BaseA++;

        [Rule]
        protected virtual void Overridden(TestCase tc) => Hidden++;

        [Rule]
        private void OnlyInBase(TestCase tc) => B++;
    }

    private sealed class Derived : Base
    {
        internal int DerivedA;

        [Rule]
        protected override void A(TestCase tc) => DerivedA++;

        protected override void Overridden(TestCase tc) { }
    }

    [Fact]
    public void RulesAreCollectedUpTheHierarchyOnceByName()
    {
        var totals = new Derived();
        var report = Property.Test(tc =>
        {
            var machine = new Derived();
            Stateful.Run(machine, tc);
            totals.BaseA += machine.BaseA;
            totals.DerivedA += machine.DerivedA;
            totals.Hidden += machine.Hidden;
            totals.B += machine.B;
        }, Quick);
        Assert.True(report.Passed);
        Assert.Equal(0, totals.BaseA);
        Assert.Equal(0, totals.Hidden);
        Assert.True(totals.DerivedA > 0);
        Assert.True(totals.B > 0);
    }

    private sealed class Weighted
    {
        internal int Heavy;
        internal int Light;

        [Rule(Weight = 20)]
        internal void HeavyRule(TestCase tc) => Heavy++;

        [Rule(Weight = 1)]
        internal void LightRule(TestCase tc) => Light++;
    }

    [Fact]
    public void WeightedRulesAreChosenMoreOften()
    {
        var dominated = false;
        var seen = new List<(int Heavy, int Light)>();
        Property.Test(tc =>
        {
            var machine = new Weighted();
            Stateful.Run(machine, tc);
            seen.Add((machine.Heavy, machine.Light));
            dominated |= machine.Light > 0 && machine.Heavy > 5 * machine.Light;
        }, Quick with { TestCases = 50, Seed = 1 });
        Assert.True(dominated, $"expected a case where both rules ran and the weight-20 rule dominated: {string.Join(", ", seen)}");
    }

    private sealed class ZeroWeight
    {
        [Rule(Weight = 0)]
        internal void Never(TestCase tc) { }
    }

    private sealed class NanWeight
    {
        [Rule(Weight = double.NaN)]
        internal void Undefined(TestCase tc) { }
    }

    private sealed class InfiniteWeight
    {
        [Rule(Weight = double.PositiveInfinity)]
        internal void Endless(TestCase tc) { }
    }

    [Fact]
    public void NonPositiveOrNonFiniteWeightsAreRejected()
    {
        var zero = Assert.Throws<ArgumentException>(() => Property.Test(tc => Stateful.Run(new ZeroWeight(), tc), Quick));
        Assert.StartsWith("[Rule] Weight of Never must be finite and positive, got 0.0", zero.Message);
        var nan = Assert.Throws<ArgumentException>(() => Property.Test(tc => Stateful.Run(new NanWeight(), tc), Quick));
        Assert.StartsWith("[Rule] Weight of Undefined must be finite and positive, got double.NaN", nan.Message);
        var inf = Assert.Throws<ArgumentException>(() => Property.Test(tc => Stateful.Run(new InfiniteWeight(), tc), Quick));
        Assert.StartsWith("[Rule] Weight of Endless must be finite and positive, got double.PositiveInfinity", inf.Message);
    }

    private sealed class Grouped
    {
        private int _letters;
        private int _numbers;

        [Rule(Group = "letters")]
        internal void A(TestCase tc) => _letters++;

        [Rule(Group = "letters")]
        internal void B(TestCase tc) => _letters++;

        [Rule(Group = "numbers")]
        internal void One(TestCase tc) => _numbers++;

        [Rule]
        internal void Reset(TestCase tc)
        {
            _letters = 0;
            _numbers = 0;
        }

        [Invariant]
        internal void Counts(TestCase tc) => Assert.True(_letters >= 0 && _numbers >= 0);
    }

    [Fact]
    public void GroupedMachinePassesSequentially()
    {
        Assert.True(Property.Test(tc => Stateful.Run(new Grouped(), tc), Quick).Passed);
    }

    private sealed class Drawing
    {
        [Rule]
        internal void Flip(TestCase tc) => tc.Draw(Generators.Booleans());
    }

    private sealed class RejectingBooleans(ILibhegel inner, int rc, int times, bool onWorkers) : DelegatingLibhegel(inner)
    {
        private int _remaining = times;

        public override int GenerateBoolean(nint testCase, double p, out bool value)
        {
            var worker = Thread.CurrentThread.Name?.StartsWith("hegel-worker-", StringComparison.Ordinal) == true;
            if (worker == onWorkers && Interlocked.Decrement(ref _remaining) >= 0)
            {
                value = false;
                return rc;
            }
            return base.GenerateBoolean(testCase, p, out value);
        }
    }

    [Fact]
    public void AnEngineRejectionInsideARuleAbortsTheCase()
    {
        using var _ = Engines.Override(new RejectingBooleans(Engines.Real, Abi.EAssume, 3, false));
        var report = Property.Test(tc => Stateful.Run(new Drawing(), tc), Quick);
        Assert.True(report.Passed);
        Assert.True(report.Statistics.Invalid >= 1);
    }

    private sealed class WrongRuleIndex(ILibhegel inner) : DelegatingLibhegel(inner)
    {
        public override int StateMachineNextRule(nint testCase, nint stateMachine, long workerIndex, out long rule)
        {
            var rc = base.StateMachineNextRule(testCase, stateMachine, workerIndex, out rule);
            if (rc == Abi.Ok && rule != Abi.StateMachineDone)
            {
                rule = 99;
            }
            return rc;
        }
    }

    [Fact]
    public void AnOutOfRangeRuleIndexIsAnInternalError()
    {
        using var _ = Engines.Override(new WrongRuleIndex(Engines.Real));
        var e = Assert.Throws<HegelException>(() => Property.Test(tc => Stateful.Run(new Counter(), tc), Quick));
        Assert.Equal("internal error: state machine chose out-of-range rule index 99", e.Message);
    }

    [Fact]
    public void AnOutOfRangeRuleIndexOnAWorkerIsAnInternalError()
    {
        using var _ = Engines.Override(new WrongRuleIndex(Engines.Real));
        var options = new StatefulOptions { MinConcurrency = 2, MaxConcurrency = 2 };
        var e = Assert.Throws<HegelException>(() => Property.Test(tc => Stateful.Run(new Counter(), tc, options), Quick));
        Assert.Equal("internal error: state machine chose out-of-range rule index 99", e.Message);
    }

    private sealed class ConcurrentCounter
    {
        private int _n;

        [Rule]
        internal void Increment(TestCase tc) => Interlocked.Increment(ref _n);

        [Rule]
        internal void Decrement(TestCase tc)
        {
            tc.Assume(Volatile.Read(ref _n) > 0);
            Interlocked.Decrement(ref _n);
        }

        [Invariant]
        internal void NonNegative(TestCase tc) => Assert.True(Volatile.Read(ref _n) >= 0);
    }

    [Fact]
    public void ConcurrentCounterHoldsAtEveryConcurrencyLevel()
    {
        var reporter = new CapturingReporter();
        var report = Property.Run(
            tc => Stateful.Run(new ConcurrentCounter(), tc, new StatefulOptions { MaxConcurrency = 3, StepCount = 10 }),
            Quick with { Verbosity = Verbosity.Verbose },
            reporter);
        Assert.True(report.Passed);
        Assert.Contains("Concurrency level: 3", reporter.Lines);
        Assert.Contains("Checking invariants on the initial state.", reporter.Lines);
        Assert.Contains("Checking invariants on the final state.", reporter.Lines);
        Assert.Contains(reporter.Lines, line => WorkerLine.IsMatch(line) && line.EndsWith("Rule stopped early due to violated assumption.", StringComparison.Ordinal));
    }

    private static readonly System.Text.RegularExpressions.Regex WorkerLine = new(@"^\[worker \d+ \+\d+(\.\d+)?ms\] ");

    private sealed class Probe
    {
        internal readonly ConcurrentDictionary<int, bool> Threads = new();
        internal int InFlight;
        internal bool Overlapped;

        [Rule]
        internal void Touch(TestCase tc)
        {
            Threads[Environment.CurrentManagedThreadId] = true;
            if (Interlocked.Increment(ref InFlight) > 1)
            {
                Overlapped = true;
            }
            Thread.Sleep(1);
            Interlocked.Decrement(ref InFlight);
        }
    }

    [Fact]
    public void WorkersRunRulesOffTheDrivingThread()
    {
        var driver = Environment.CurrentManagedThreadId;
        var overlapped = false;
        var workers = new HashSet<int>();
        Property.Test(tc =>
        {
            var probe = new Probe();
            Stateful.Run(probe, tc, new StatefulOptions { MinConcurrency = 3, MaxConcurrency = 3, StepCount = 12 });
            Assert.DoesNotContain(driver, probe.Threads.Keys);
            overlapped |= probe.Overlapped;
            workers.Add(probe.Threads.Count);
        }, Quick with { TestCases = 10 });
        Assert.Contains(3, workers);
        Assert.True(overlapped);
    }

    [Fact]
    public void SequentialRunsStayOnTheCallingThread()
    {
        var driver = Environment.CurrentManagedThreadId;
        Property.Test(tc =>
        {
            var probe = new Probe();
            Stateful.Run(probe, tc, 5);
            Assert.Equal([driver], probe.Threads.Keys);
        }, Quick);
    }

    private sealed class ConcurrentBoom
    {
        [Rule]
        internal void Boom(TestCase tc)
        {
            var x = tc.Draw(Generators.Integers(), "x");
            throw new InvalidOperationException($"concurrent boom {x}");
        }
    }

    [Fact]
    public void AWorkerFailureIsReportedWithItsRoundAndDroppedRivals()
    {
        var reporter = new CapturingReporter();
        var options = new StatefulOptions { MinConcurrency = 2, MaxConcurrency = 2, StepCount = 6 };
        var report = Property.Run(tc => Stateful.Run(new ConcurrentBoom(), tc, options), Quick with { Verbosity = Verbosity.Verbose }, reporter);
        Assert.Equal(RunStatus.Failed, report.Status);
        var failure = Assert.Single(report.Failures);
        var e = Assert.IsType<InvalidOperationException>(failure.Exception);
        Assert.StartsWith("concurrent boom", e.Message);
        Assert.Equal("Concurrency level: 2", failure.Output[0]);
        Assert.Equal("---------------- Round 1: group \"<anonymous>\" ----------------", failure.Output[1]);
        Assert.Contains(failure.Output, line => WorkerLine.IsMatch(line) && line.EndsWith("Rule: Boom {", StringComparison.Ordinal));
        Assert.Contains(failure.Output, line => WorkerLine.IsMatch(line) && line.EndsWith("Rule Boom failed:", StringComparison.Ordinal));
        Assert.Contains(failure.Output, line => WorkerLine.IsMatch(line) && line.EndsWith("  var x_1 = 0;", StringComparison.Ordinal));
        Assert.Contains(reporter.Lines, line => line.StartsWith("Dropped concurrent failure from worker 1: System.InvalidOperationException: concurrent boom", StringComparison.Ordinal));
        Assert.Throws<InvalidOperationException>(report.ThrowIfFailed);
    }

    private sealed class GroupedRounds
    {
        [Rule(Group = "letters")]
        internal void A(TestCase tc) { }

        [Rule(Group = "numbers")]
        internal void One(TestCase tc) { }
    }

    [Fact]
    public void ConcurrentRoundsAreHeadedByTheirGroup()
    {
        var reporter = new CapturingReporter();
        var options = new StatefulOptions { MinConcurrency = 2, MaxConcurrency = 2, StepCount = 8 };
        var report = Property.Run(tc => Stateful.Run(new GroupedRounds(), tc, options), Quick with { Verbosity = Verbosity.Verbose }, reporter);
        Assert.True(report.Passed);
        Assert.Contains(reporter.Lines, line => line.Contains("Round 1: group \"letters\"") || line.Contains("Round 1: group \"numbers\""));
        Assert.Contains(reporter.Lines, line => line.Contains(": group \"letters\" "));
        Assert.Contains(reporter.Lines, line => line.Contains(": group \"numbers\" "));
    }

    private sealed class BadLabel
    {
        [Rule]
        internal void Draw(TestCase tc) => tc.Draw(Generators.Integers(), "a\nb");
    }

    [Fact]
    public void AUsageErrorOnAWorkerPropagates()
    {
        var options = new StatefulOptions { MinConcurrency = 2, MaxConcurrency = 2 };
        var e = Assert.Throws<HegelUsageException>(() => Property.Test(tc => Stateful.Run(new BadLabel(), tc, options), Quick));
        Assert.Contains("must not contain a newline", e.Message);
    }

    [Fact]
    public void EngineStopsAndRejectionsOnWorkersEndTheCase()
    {
        var options = new StatefulOptions { MinConcurrency = 2, MaxConcurrency = 2, StepCount = 4 };
        using (Engines.Override(new RejectingBooleans(Engines.Real, Abi.EStopTest, 2, true)))
        {
            var report = Property.Test(tc => Stateful.Run(new Drawing(), tc, options), Quick);
            Assert.True(report.Passed);
            Assert.True(report.Statistics.Overrun >= 1);
        }
        using (Engines.Override(new RejectingBooleans(Engines.Real, Abi.EAssume, 2, true)))
        {
            var report = Property.Test(tc => Stateful.Run(new Drawing(), tc, options), Quick);
            Assert.True(report.Passed);
            Assert.True(report.Statistics.Invalid >= 1);
        }
    }
}
