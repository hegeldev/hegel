using System.Collections.Concurrent;
using System.Reflection;
using System.Runtime.ExceptionServices;
using Hegel.Native;

namespace Hegel;

/// <summary>Marks a method of a state machine as a rule: a step the test may take. It takes one <see cref="TestCase"/> parameter.</summary>
[AttributeUsage(AttributeTargets.Method, Inherited = false)]
public sealed class RuleAttribute : Attribute
{
    /// <summary>The group the rule belongs to; a concurrent round runs rules of one group.</summary>
    public string Group { get; init; } = "";

    /// <summary>How often the rule is chosen relative to the others. Positive, 1 by default.</summary>
    public double Weight { get; init; } = 1.0;
}

/// <summary>Marks a method of a state machine as an invariant, checked between steps. It takes one <see cref="TestCase"/> parameter.</summary>
[AttributeUsage(AttributeTargets.Method, Inherited = false)]
public sealed class InvariantAttribute : Attribute
{
    /// <summary>Check after every step, rather than when the engine chooses to.</summary>
    public bool AlwaysRun { get; init; }
}

/// <summary>How <see cref="Stateful.Run(object, TestCase, StatefulOptions)"/> drives a state machine.</summary>
public sealed record StatefulOptions
{
    private readonly int _stepCount = Stateful.DefaultStepCount;
    private readonly int _minConcurrency = 1;
    private readonly int _maxConcurrency = 1;

    /// <summary>How many steps to take. At least 1.</summary>
    public int StepCount
    {
        get => _stepCount;
        init => _stepCount = value >= 1 ? value : throw new ArgumentOutOfRangeException(nameof(StepCount), value, "StepCount must be at least 1");
    }

    /// <summary>The fewest workers to run rules concurrently on. At least 1.</summary>
    public int MinConcurrency
    {
        get => _minConcurrency;
        init => _minConcurrency = value >= 1 ? value : throw new ArgumentOutOfRangeException(nameof(MinConcurrency), value, "MinConcurrency must be at least 1");
    }

    /// <summary>The most workers to run rules concurrently on. At least 1; 1 runs the machine sequentially.</summary>
    public int MaxConcurrency
    {
        get => _maxConcurrency;
        init => _maxConcurrency = value >= 1 ? value : throw new ArgumentOutOfRangeException(nameof(MaxConcurrency), value, "MaxConcurrency must be at least 1");
    }
}

/// <summary>
/// Stateful testing: drive an object whose <see cref="RuleAttribute"/> methods are the steps the
/// test may take and whose <see cref="InvariantAttribute"/> methods must hold between them.
/// </summary>
public static class Stateful
{
    /// <summary>How many steps a run takes unless <see cref="StatefulOptions.StepCount"/> says otherwise.</summary>
    public const int DefaultStepCount = 50;

    /// <summary>The group of rules that name none.</summary>
    public const string AnonymousGroup = "<anonymous>";

    private static readonly StatefulOptions Default = new();

    /// <summary>Drive <paramref name="machine"/> for <see cref="DefaultStepCount"/> steps.</summary>
    public static void Run(object machine, TestCase tc) => Run(machine, tc, Default);

    /// <summary>Drive <paramref name="machine"/> for <paramref name="stepCount"/> steps.</summary>
    public static void Run(object machine, TestCase tc, int stepCount) => Run(machine, tc, Default with { StepCount = stepCount });

    /// <summary>Drive <paramref name="machine"/> as <paramref name="options"/> say.</summary>
    public static void Run(object machine, TestCase tc, StatefulOptions options)
    {
        ArgumentNullException.ThrowIfNull(machine);
        ArgumentNullException.ThrowIfNull(tc);
        ArgumentNullException.ThrowIfNull(options);
        if (options.MinConcurrency > options.MaxConcurrency)
        {
            throw new ArgumentException($"MinConcurrency ({options.MinConcurrency}) must not exceed MaxConcurrency ({options.MaxConcurrency})", nameof(options));
        }
        var rules = Annotated<RuleAttribute>(machine);
        var invariants = Annotated<InvariantAttribute>(machine);
        if (rules.Count == 0)
        {
            throw new ArgumentException($"{machine.GetType().FullName} has no [Rule] methods; a state machine needs at least one", nameof(machine));
        }
        var groupNames = new List<string>();
        var groups = Groups(rules, groupNames);
        var sm = tc.NewStateMachine(Names(rules), groups, Weights(rules), Names(invariants), AlwaysRun(invariants), options.MinConcurrency, options.MaxConcurrency, options.StepCount);
        try
        {
            if (options.MaxConcurrency == 1)
            {
                DriveSequential(machine, rules, invariants, tc, sm.Id);
            }
            else
            {
                DriveConcurrent(machine, rules, invariants, groupNames, tc, sm.Id, sm.Concurrency);
            }
        }
        finally
        {
            tc.StateMachineFree(sm.Id);
        }
    }

    private static void DriveSequential(object machine, List<MethodInfo> rules, List<MethodInfo> invariants, TestCase tc, nint machineId)
    {
        if (invariants.Count > 0)
        {
            tc.Note("Checking invariants on the initial state.");
        }
        CheckInvariants(machine, invariants, tc, machineId, false);
        var step = 0;
        while (true)
        {
            tc.StartSpan(Label.StatefulRule);
            if (tc.StateMachineNextGroup(machineId) == Abi.StateMachineDone)
            {
                tc.StopSpan(false);
                break;
            }
            var roundRejected = false;
            while (true)
            {
                var index = tc.StateMachineNextRule(machineId, 0);
                if (index == Abi.StateMachineDone)
                {
                    break;
                }
                var rule = rules[RuleIndex(index, rules)];
                step++;
                tc.Note($"Step {step}: {rule.Name} {{");
                var ruleTc = tc.Block(2);
                Exception? failure = null;
                try
                {
                    Invoke(machine, rule, ruleTc);
                }
                catch (AssumeRejected)
                {
                    if (tc.IsAborted)
                    {
                        throw;
                    }
                    tc.StateMachineRuleRejected(machineId, 0);
                    roundRejected = true;
                    ruleTc.Note("Rule stopped early due to violated assumption.");
                }
                catch (Exception e)
                {
                    tc.StopSpan(false);
                    failure = e;
                    throw;
                }
                finally
                {
                    ruleTc.EndBlock();
                    tc.Note("}");
                    NoteFailure(tc, $"Rule {rule.Name} failed:", failure);
                }
            }
            tc.StopSpan(roundRejected);
            CheckInvariants(machine, invariants, tc, machineId, true);
        }
        if (invariants.Count > 0)
        {
            tc.Note("Checking invariants on the final state.");
        }
        CheckInvariants(machine, invariants, tc, machineId, false);
    }

    private static void DriveConcurrent(object machine, List<MethodInfo> rules, List<MethodInfo> invariants, List<string> groupNames, TestCase tc, nint machineId, int concurrency)
    {
        tc.Note($"Concurrency level: {concurrency}");
        if (invariants.Count > 0)
        {
            tc.Note("Checking invariants on the initial state.");
        }
        CheckInvariants(machine, invariants, tc, machineId, false);
        var workers = new Worker[concurrency];
        for (var w = 0; w < concurrency; w++)
        {
            workers[w] = new Worker(w, machine, rules, machineId, tc);
        }
        var handles = new List<TestCase>();
        try
        {
            foreach (var worker in workers)
            {
                worker.Start();
            }
            var round = 0;
            while (true)
            {
                var group = tc.StateMachineNextGroup(machineId);
                if (group == Abi.StateMachineDone)
                {
                    break;
                }
                round++;
                tc.Note($"---------------- Round {round}: group \"{groupNames[(int)group]}\" ----------------");
                foreach (var worker in workers)
                {
                    handles.Add(tc.ForWorker(worker.Index));
                }
                for (var w = 0; w < concurrency; w++)
                {
                    workers[w].Inbox.Add(handles[w]);
                }
                var events = new WorkerEvent[concurrency];
                for (var w = 0; w < concurrency; w++)
                {
                    events[w] = workers[w].Await();
                }
                foreach (var handle in handles)
                {
                    handle.Release();
                }
                handles.Clear();
                ResolveRound(events, tc);
                CheckInvariants(machine, invariants, tc, machineId, true);
            }
            if (invariants.Count > 0)
            {
                tc.Note("Checking invariants on the final state.");
            }
            CheckInvariants(machine, invariants, tc, machineId, false);
        }
        finally
        {
            foreach (var worker in workers)
            {
                worker.Stop();
            }
            foreach (var handle in handles)
            {
                handle.Release();
            }
        }
    }

    private static void ResolveRound(WorkerEvent[] events, TestCase tc)
    {
        foreach (var e in events)
        {
            if (e.Kind == WorkerEvent.Kinds.Control)
            {
                ExceptionDispatchInfo.Throw(e.Error!);
            }
        }
        var overrun = events.Any(e => e.Kind == WorkerEvent.Kinds.Overrun);
        var invalid = events.Any(e => e.Kind == WorkerEvent.Kinds.Invalid);
        Exception? winner = null;
        for (var w = 0; w < events.Length; w++)
        {
            var e = events[w];
            if (e.Kind != WorkerEvent.Kinds.Failed)
            {
                continue;
            }
            if (overrun || invalid || winner != null)
            {
                tc.Note($"Dropped concurrent failure from worker {w}: {e.Error}");
            }
            else
            {
                winner = e.Error;
            }
        }
        if (overrun)
        {
            throw new StopTest();
        }
        if (invalid)
        {
            throw new AssumeRejected();
        }
        if (winner != null)
        {
            ExceptionDispatchInfo.Throw(winner);
        }
    }

    private sealed record WorkerEvent(WorkerEvent.Kinds Kind, Exception? Error)
    {
        internal enum Kinds
        {
            Done,
            Control,
            Overrun,
            Invalid,
            Failed,
        }

        internal static readonly WorkerEvent DoneEvent = new(Kinds.Done, null);
        internal static readonly WorkerEvent OverrunEvent = new(Kinds.Overrun, null);
        internal static readonly WorkerEvent InvalidEvent = new(Kinds.Invalid, null);

        internal static WorkerEvent Control(Exception error) => new(Kinds.Control, error);

        internal static WorkerEvent Failed(Exception failure) => new(Kinds.Failed, failure);
    }

    private sealed class Worker
    {
        private static readonly object StopCommand = new();

        internal readonly int Index;
        internal readonly BlockingCollection<object> Inbox = [];
        private readonly BlockingCollection<WorkerEvent> _outbox = [];
        private readonly Thread _thread;
        private readonly object _machine;
        private readonly List<MethodInfo> _rules;
        private readonly nint _machineId;
        private readonly TestCase _root;

        internal Worker(int index, object machine, List<MethodInfo> rules, nint machineId, TestCase root)
        {
            Index = index;
            _machine = machine;
            _rules = rules;
            _machineId = machineId;
            _root = root;
            _thread = new Thread(Loop) { Name = $"hegel-worker-{index}", IsBackground = true };
        }

        internal void Start() => _thread.Start();

        private void Loop()
        {
            try
            {
                while (true)
                {
                    var command = Inbox.Take();
                    if (ReferenceEquals(command, StopCommand))
                    {
                        return;
                    }
                    _outbox.Add(RunRound((TestCase)command));
                }
            }
            finally
            {
                _outbox.Add(WorkerEvent.Control(new HegelException($"internal error: concurrent worker {Index} exited without reporting its round")));
                _root.ReleaseThreadContext();
            }
        }

        private WorkerEvent RunRound(TestCase wtc)
        {
            try
            {
                while (true)
                {
                    var index = wtc.StateMachineNextRule(_machineId, Index);
                    if (index == Abi.StateMachineDone)
                    {
                        return WorkerEvent.DoneEvent;
                    }
                    var rule = _rules[RuleIndex(index, _rules)];
                    wtc.Note($"Rule: {rule.Name} {{");
                    var ruleTc = wtc.Block(2);
                    Exception? failure = null;
                    try
                    {
                        Invoke(_machine, rule, ruleTc);
                    }
                    catch (AssumeRejected)
                    {
                        if (wtc.IsAborted)
                        {
                            throw;
                        }
                        wtc.StateMachineRuleRejected(_machineId, Index);
                        ruleTc.Note("Rule stopped early due to violated assumption.");
                    }
                    catch (Exception e)
                    {
                        failure = e;
                        throw;
                    }
                    finally
                    {
                        ruleTc.EndBlock();
                        wtc.Note("}");
                        NoteFailure(wtc, $"Rule {rule.Name} failed:", failure);
                    }
                }
            }
            catch (AssumeRejected)
            {
                return WorkerEvent.InvalidEvent;
            }
            catch (StopTest)
            {
                return WorkerEvent.OverrunEvent;
            }
            catch (Exception e) when (e is HegelException or HegelUsageException)
            {
                return WorkerEvent.Control(e);
            }
            catch (Exception e)
            {
                return WorkerEvent.Failed(e);
            }
        }

        internal WorkerEvent Await() => _outbox.Take();

        internal void Stop()
        {
            Inbox.Add(StopCommand);
            _thread.Join();
        }
    }

    private static void CheckInvariants(object machine, List<MethodInfo> invariants, TestCase tc, nint machineId, bool sampled)
    {
        for (var i = 0; i < invariants.Count; i++)
        {
            if (sampled && !tc.StateMachineShouldCheckInvariant(machineId, i))
            {
                continue;
            }
            var invariant = invariants[i];
            var invariantTc = tc.Block(2);
            Exception? failure = null;
            try
            {
                Invoke(machine, invariant, invariantTc);
            }
            catch (Exception e)
            {
                failure = e;
                throw;
            }
            finally
            {
                invariantTc.EndBlock();
                NoteFailure(tc, $"Invariant {invariant.Name} failed:", failure);
            }
        }
    }

    private static void NoteFailure(TestCase tc, string trailer, Exception? failure)
    {
        if (failure != null && failure is not AssumeRejected && failure is not StopTest)
        {
            tc.Note(trailer);
        }
    }

    private static int RuleIndex(long index, List<MethodInfo> rules)
    {
        if (index < 0 || index >= rules.Count)
        {
            throw new HegelException($"internal error: state machine chose out-of-range rule index {index}");
        }
        return (int)index;
    }

    private static void Invoke(object machine, MethodInfo method, TestCase tc)
    {
        try
        {
            method.Invoke(machine, [tc]);
        }
        catch (TargetInvocationException e) when (e.InnerException != null)
        {
            ExceptionDispatchInfo.Throw(e.InnerException);
        }
    }

    private static List<MethodInfo> Annotated<TAttribute>(object machine) where TAttribute : Attribute
    {
        var methods = new List<MethodInfo>();
        var seen = new HashSet<string>();
        for (var type = machine.GetType(); type != null && type != typeof(object); type = type.BaseType)
        {
            foreach (var m in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly))
            {
                var parameters = m.GetParameters();
                var takesTestCase = parameters.Length == 1 && parameters[0].ParameterType == typeof(TestCase);
                if (!m.IsDefined(typeof(TAttribute), false))
                {
                    if (takesTestCase)
                    {
                        seen.Add(m.Name);
                    }
                    continue;
                }
                if (!takesTestCase)
                {
                    throw new ArgumentException($"[{typeof(TAttribute).Name.Replace("Attribute", "")}] method {type.FullName}.{m.Name} must take a single TestCase parameter", nameof(machine));
                }
                if (seen.Add(m.Name))
                {
                    methods.Add(m);
                }
            }
        }
        methods.Sort((a, b) => string.CompareOrdinal(a.Name, b.Name));
        return methods;
    }

    private static List<string> Names(List<MethodInfo> methods) => methods.Select(m => m.Name).ToList();

    private static long[] Groups(List<MethodInfo> rules, List<string> groupNames)
    {
        var ids = new long[rules.Count];
        for (var i = 0; i < ids.Length; i++)
        {
            var group = rules[i].GetCustomAttribute<RuleAttribute>()!.Group;
            if (group.Length == 0)
            {
                group = AnonymousGroup;
            }
            var id = groupNames.IndexOf(group);
            if (id < 0)
            {
                id = groupNames.Count;
                groupNames.Add(group);
            }
            ids[i] = id;
        }
        return ids;
    }

    private static double[]? Weights(List<MethodInfo> rules)
    {
        var weights = new double[rules.Count];
        var weighted = false;
        for (var i = 0; i < weights.Length; i++)
        {
            var w = rules[i].GetCustomAttribute<RuleAttribute>()!.Weight;
            if (!(double.IsFinite(w) && w > 0))
            {
                throw new ArgumentException($"[Rule] Weight of {rules[i].Name} must be finite and positive, got {Printer.Double(w)}", "machine");
            }
            weights[i] = w;
            weighted |= w != 1.0;
        }
        return weighted ? weights : null;
    }

    private static bool[] AlwaysRun(List<MethodInfo> invariants) =>
        invariants.Select(i => i.GetCustomAttribute<InvariantAttribute>()!.AlwaysRun).ToArray();
}
