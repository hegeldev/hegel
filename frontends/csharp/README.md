> [!IMPORTANT]
> We're excited you're checking out Hegel! Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel/issues/new).
>
> As part of our beta, we may make breaking changes if it makes Hegel a better property-based testing library. If that instability bothers you, please check back in a few months for a stable release!
>
> See https://hegel.dev/compatibility for more details.

# Hegel for C# and F#

* [Website](https://hegel.dev)
* [NuGet: `Hegel`](https://www.nuget.org/packages/Hegel) for C#, [`Hegel.FSharp`](https://www.nuget.org/packages/Hegel.FSharp) for F#

Hegel is a property-based testing library for .NET. Hegel is based on [Hypothesis](https://github.com/hypothesisworks/hypothesis), using the [Hegel protocol](https://hegel.dev/).

Instead of writing tests with hand-picked example inputs, you describe a *property* that should hold for all inputs and let Hegel generate inputs to try to falsify it. When it finds a failing input it automatically **shrinks** it to a minimal counterexample.

## Installation

Hegel ships as the `Hegel` package, for .NET 8 and later; xUnit projects add `Hegel.Xunit` on top of it for the `[HegelTest]` attribute, and F# projects `Hegel.FSharp` (see [F#](#f) below):

```bash
dotnet add package Hegel --version 0.1.4
dotnet add package Hegel.Xunit --version 0.1.4
```

or in your test project:

```xml
<PackageReference Include="Hegel" Version="0.1.4" />
<PackageReference Include="Hegel.Xunit" Version="0.1.4" />
```

The engine is bundled in the `Hegel` package for Linux (x86-64 and arm64), macOS (Apple Silicon), and Windows (x86-64 and arm64); nothing is downloaded at runtime. Hegel works with any test framework: `Property.Test` runs inside your test method and throws the failing assertion, so xUnit, NUnit, MSTest and TUnit all report a property failure the way they report any other. Under xUnit, `Hegel.Xunit` lets you write the property as the test method itself.

## Quickstart

Here's a quick example of how to write a Hegel test, with xUnit and `Hegel.Xunit`:

```csharp
using Hegel;
using Hegel.Xunit;
using Xunit;

public class SortTests
{
    static List<int> MySort(List<int> xs) => new SortedSet<int>(xs).ToList(); // oops: a set drops duplicates

    [HegelTest]
    public void SortPreservesLength(TestCase tc)
    {
        var xs = tc.Draw(Generators.Lists(Generators.Integers()), "xs");
        Assert.Equal(xs.Count, MySort(xs).Count);
    }
}
```

This test will fail when run with `dotnet test`! Hegel will produce a minimal failing test case for us, written as the C# you would type to reproduce it, in the test's output, and fail the test with the assertion's own failure:

```
Xunit.Sdk.EqualException: Assert.Equal() Failure: Values differ
Expected: 2
Actual:   1

Standard Output Messages:
 var xs_1 = new List<int> {0, 0};
```

Hegel reports the minimal example showing that our sort is incorrectly dropping duplicates: `[0, 0]`, two equal elements, which `MySort` collapses into one. If we replace the `SortedSet`-based body of `MySort` with a sort that keeps duplicates, this test will then pass.

A `[HegelTest]` method takes one `TestCase` and returns `void`, and is one test in xUnit's test tree; `Skip` and `DisplayName` work as on a `[Fact]`. The attribute takes a `Profile` name (a `hegel.toml` profile or one registered with `Settings.RegisterProfile`) and a `TestCases` count:

```csharp
[HegelTest(Profile = "ci", TestCases = 1000)]
public void SortPreservesLength(TestCase tc) { ... }
```

With any other test framework, or for a setting only known at runtime, call `Property.Test` from an ordinary test method instead. It runs the property and throws the failing assertion, so the test fails the same way; the minimal test case is printed to standard error:

```csharp
[Fact]
public void SortPreservesLength() => Property.Test(tc =>
{
    var xs = tc.Draw(Generators.Lists(Generators.Integers()), "xs");
    Assert.Equal(xs.Count, MySort(xs).Count);
});
```

## Writing properties

A `[HegelTest]` method, or the `Action<TestCase>` given to `Property.Test`, receives a `TestCase`, which is where you ask for inputs:

- `tc.Draw(generator, "xs")` draws a value and prints it under the name you give, numbered per draw (`xs_1` above); a draw without a name prints as `draw_1`.
- `tc.Assume(condition)` discards the current test case when the condition is false, so you can rule out inputs the property does not apply to. Prefer a generator that produces only valid inputs when you can; `tc.Reject()` discards unconditionally.
- `tc.Note(message)` adds a line to the transcript printed with a failure.
- `tc.Target(value)` tells Hegel to try to maximise `value`, which guides generation towards extreme inputs; `tc.Event(label)` and `tc.EventValue(label, value)` record what the test case did, for the statistics `Settings.ShowStatistics` prints.
- `tc.Repeat(body)` runs `body` a generated number of times, which is how to write a loop whose length Hegel controls and shrinks.

The failure Hegel throws is the minimal failing test case's own exception, with its stack trace, so `Assert.Equal` in the quickstart fails exactly as it would in an example-based test. A property that finds several distinct failures reports the smallest unless `Settings.ReportMultipleFailures` is on, in which case it throws a `MultipleFailuresException` listing them all.

## Generators

Every generator is a `Generator<T>`. `Generators` is the catalogue; the generators it returns are immutable, so the builder methods return new generators:

```csharp
Generators.Integers().Min(0).Max(100)
Generators.Doubles().Min(0).AllowNan(false).AllowInfinity(false)
Generators.Text().MaxSize(20).Alphabet("abc")
Generators.Text().Categories("Lu", "Nd")
Generators.FromRegex(@"[a-z]+@[a-z]+\.com")
Generators.Lists(Generators.Integers()).MinSize(1).Unique()
Generators.Maps(Generators.Text(), Generators.Integers())
Generators.Tuples(Generators.Integers(), Generators.Booleans())
Generators.SampledFrom(["red", "green", "blue"])
Generators.OneOf(Generators.Integers(), Generators.Just(-1))
Generators.Nullable(Generators.Integers())
Generators.Dates(), Generators.DateTimes().WithTimeZones(Generators.TimeZones())
```

Generators compose with `Map`, `Filter` and `FlatMap`:

```csharp
var evens = Generators.Integers().Map(n => n * 2);
var nonEmpty = Generators.Text().Filter(s => s.Length > 0);
var pairs = Generators.Integers().FlatMap(n => Generators.Tuples(Generators.Just(n), Generators.Integers().Min(n)));
```

`Generators.Composite` builds a generator out of draws, for a value that several generators contribute to:

```csharp
var ranges = Generators.Composite(tc =>
{
    var lo = tc.Draw(Generators.Integers());
    var hi = tc.Draw(Generators.Integers().Min(lo));
    return (lo, hi);
});
```

`Generators.ForType<T>()` derives a generator from a type: the scalars, `Nullable<T>`, enums, arrays, `List<T>`, `HashSet<T>`, `Dictionary<K, V>`, tuples, and records, recursively through their primary constructors. `Generators.Records<T>()` does the same for one record and lets you override a component with `.With("Name", generator)`. `Generators.Recursive(base, extend)` builds trees, and `Generators.Deferred<T>()` lets a generator refer to itself.

Values are printed in a failure as the C# that constructs them. Your own types print with `ToString()`; a generator you write can print its draws itself by extending `PrintableGenerator<T>`, or you can attach a printer to any generator with `PrintWith`.

## Settings

`Settings` is an immutable record. Its defaults come from the engine: the active profile, `hegel.toml` and the `HEGEL_*` environment variables, so a setting changed with `with` overrides those for one test:

```csharp
[Fact]
public void ManyCases() => Property.Test(tc => { ... }, new Settings { TestCases = 1000 });
```

(A `[HegelTest]` method takes its settings from the attribute's `Profile` and `TestCases` instead.)

- `TestCases`, `Seed`, `Derandomize`, `Phases`, `Verbosity`, `Database`, `SuppressHealthCheck`, `ReportMultipleFailures`, `StopOnFirstFailure`, `ShowStatistics`, `PrintBlob`, `UnboundedChoices`, `NondeterminismStrictness` and `Backend` mirror the engine's settings.
- `Examples` lists explicit test cases that run before the generated ones: `Example.Of(1, 2)` gives positional values to the draws in order, and `Example.With("xs", values)` names a draw.
- `ReproduceFailure` replays the reproduction blobs a failure printed (with `PrintBlob` on), for a failure found elsewhere, such as in CI.
- `Settings.RegisterProfile(name, settings)` and `Settings.SetDefaultProfile(name)` register named profiles, as `hegel.toml` does; `Settings.FromProfile(name)` loads one.

Failures are saved to the database (a `.hegel/` directory found from the working directory by default) and replayed first on the next run, so a fixed test confirms its fix and a regression is found immediately.

## Stateful testing

`Stateful.Run` drives a state machine: an object whose `[Rule]` methods are the operations and whose `[Invariant]` methods are checked after every step. Each takes the `TestCase`, so a rule draws its own arguments:

```csharp
class CounterMachine
{
    private readonly Counter _counter = new();
    private int _expected;

    [Rule]
    public void Increment(TestCase tc)
    {
        var by = tc.Draw(Generators.Integers().Min(1).Max(10));
        _counter.Add(by);
        _expected += by;
    }

    [Rule]
    public void Reset(TestCase tc)
    {
        _counter.Reset();
        _expected = 0;
    }

    [Invariant]
    public void MatchesModel(TestCase tc) => Assert.Equal(_expected, _counter.Value);
}

[Fact]
public void CounterIsConsistent() => Property.Test(tc => Stateful.Run(new CounterMachine(), tc));
```

Hegel chooses which rules to run and in what order, and shrinks a failing sequence to the fewest steps that still fail. `[Rule(Group = "...", Weight = ...)]` groups rules that must not run concurrently and biases the choice; `[Invariant(AlwaysRun = true)]` keeps an invariant in the run even when the engine would skip it. `StatefulOptions` sets the step count and, with `MaxConcurrency` above one, runs rules on several threads at once to find races. A `Pool<T>` (or `ConcurrentPool<T>` for concurrent machines) remembers the values rules produce, so later rules can draw from them with `pool.Reusable()` or take them with `pool.Consuming()`.

## F#

`Hegel.FSharp` adds an F# surface over the same engine: the `Hegel` entry type, the `Gen` catalogue, the `gen { }` builder and automatic generators for F# types. A failing test case prints as the F# you would type to reproduce it, whichever generators it drew from.

```bash
dotnet add package Hegel.FSharp --version 0.1.4
```

```fsharp
open Hegel.FSharp
open Xunit

let mySort (xs: int list) = xs |> Set.ofList |> Set.toList // oops: a set drops duplicates

[<Fact>]
let ``sort preserves length`` () =
    Hegel.test (fun tc ->
        let xs = tc.Draw(Gen.list Gen.int, "xs")
        Assert.Equal(xs.Length, (mySort xs).Length))
```

```
let xs_1 = [0; 0]

Xunit.Sdk.EqualException: Assert.Equal() Failure: Values differ
Expected: 2
Actual:   1
```

`Hegel.FSharp` names every type of the `Hegel` package too, so that one `open` is all an F# test file needs. `Hegel.test` takes the body last, after optional `Settings` and a `Reporter`, so a settings override reads `Hegel.test (Settings(TestCases = 1000L), fun tc -> ...)`; `Hegel.run` returns the `RunReport` instead of throwing. The `TestCase` is the C# one, so `tc.Draw`, `tc.Assume`, `tc.Note`, `tc.Target`, `tc.Repeat` and `Stateful.Run` all work as above.

`Gen` holds the generators. The scalars are the C# ones, narrowed by their builder methods, and the structured ones draw F# values:

```fsharp
Gen.int.Min(0).Max(100)
Gen.string.MaxSize(20)
(Gen.list Gen.int).MinSize(1).Unique()  // int list; .UniqueBy(f) too
Gen.array Gen.bool                      // bool[]
Gen.set Gen.int                         // Set<int>
Gen.mapOf Gen.string Gen.int            // Map<string, int>
Gen.option Gen.int                      // int option; Gen.voption for ValueOption
Gen.tuple2 Gen.int Gen.string           // int * string, up to tuple5
Gen.item [ "red"; "green"; "blue" ]
Gen.oneOf [ Gen.int; Gen.constant -1 ]
Gen.regex "[a-z]+@[a-z]+\\.com"
```

Generators compose with `Gen.map`, `Gen.filter` and `Gen.bind`, or in a `gen { }` block, which draws from each generator it binds:

```fsharp
let ranges =
    gen {
        let! lo = Gen.int
        let! hi = Gen.int.Min(lo)
        return lo, hi
    }
```

`Gen.auto<'T>` derives a generator for an F# type: records, unions, tuples, options, lists, arrays, sets, maps and the scalars, recursing through a union's cases with its non-recursive ones as the base cases, so `Gen.auto<Tree>` builds trees. `Gen.record<'T>` derives one record and overrides a field with `.With("Name", generator)`; `Gen.recursive` and `Gen.deferred ()` are the C# combinators for building recursive generators by hand.

Values print as F#: `[1; 2]`, `[|1; 2|]`, `Some(1)`, `set [1; 2]`, `Map [(1, "a")]`, `{ X = 1; Y = 2 }`, `Node(Leaf, 1, Leaf)`, `1.5f`, `10L`. A C# generator drawn inside `Hegel.test` prints its value as F# too.

## Learn more

See [hegel.dev](https://hegel.dev) for the guide, and the repository's [CONTRIBUTING.md](../../CONTRIBUTING.md) to get involved.
