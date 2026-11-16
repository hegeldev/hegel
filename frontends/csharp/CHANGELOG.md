# Changelog

## 0.2.2 - 2026-11-13

This patch fixes a number of bugs in how values are printed and how generators are derived.

Printed test cases that were not valid code:

- A `BigInteger` draw now prints as `BigInteger.Parse("…")` in C#. It used to print as bare digits, which do not compile once they pass `ulong.MaxValue` and are an `int` or `long` below that.
- An undefined negative enum value now prints as `(Colour)(-1)` in C#, and in F# an undefined value of an enum that is not `int32`-backed prints as `LanguagePrimitives.EnumOfValue<byte, Small> (7uy)`.
- In F#, cases of a `[<RequireQualifiedAccess>]` union print qualified (`Shape.Circle(1.0)`), anonymous records drawn by `Gen.auto` and `Gen.record` print as `{| A = 0 |}` (`struct {| A = 0 |}` for struct ones), a union case represented as `null` prints as the case, and field and case names that are not plain identifiers print in double backticks.

Derived generators:

- `Generators.ForType<T>()` now reports a record that refers to itself through a generic record (`record A(B<int> Next)` with `record B<T>(A Back)`) as a usage error. Drawing from it used to overflow the stack and kill the test process.
- A record constructor parameter and the property it sets are now one component when their names differ only in case, as in `public Person(string name) { Name = name; }`. The property used to be drawn a second time and assigned after construction, overwriting the argument and any `With` override of it.
- The constructor a record is built with is now a public one, and the primary constructor when there are several. A longer or private secondary constructor used to be chosen instead.

Other fixes:

- `Hegel.FSharp` and `Hegel.Xunit` now depend on exactly the `Hegel` version they were built with. They used to accept any later one, which `Hegel.FSharp` cannot load against if `Hegel`'s internals have changed.
- `Database.At("")` now throws `ArgumentException`: the empty path used to silently disable the database.
- A test case count above `long.MaxValue`, from `HEGEL_TEST_CASES` or `Settings.Set`, now reads back as `long.MaxValue`. It used to throw `OverflowException`.
- The error for inverted `Dates()` and `Times()` bounds no longer formats them in the current culture's calendar.

## 0.2.1 - 2026-11-13

This patch adds `PauseFaults` to `[Invariant]`. An invariant declared with `[Invariant(PauseFaults = true)]` runs every check (on the initial state, at the sampled join points and on the final state, sequentially or with `MaxConcurrency` above one) with fault injection paused: inside [Antithesis](https://antithesis.com/) the engine runs the fault-control program before the invariant and again after it, even when the invariant throws, so no fault lands in the middle of the check. It combines with `AlwaysRun`, and outside Antithesis it has no effect.

```csharp
[Invariant(PauseFaults = true)]
internal void ReplicasAgree(TestCase tc) => Assert.Single(nodes.Select(n => n.State).Distinct());
```

## 0.2.0 - 2026-11-13

Hegel for .NET is now the Hegel project's own frontend, continuing the `Hegel` package's version numbering with a rewrite against the current libhegel engine. The package is driven through the engine's C ABI, with the API the other Hegel frontends share.

For C#, `Property.Test` runs a property written against a `TestCase` (`Draw`, `Assume`, `Note`, `Target`, `Event`, `Repeat`), shrinks any failure to a minimal counterexample, prints the minimal test case's draws as C# and rethrows the failing assertion so any test framework reports it. `Settings` is an immutable record resolved from the engine's profiles, `hegel.toml` and `HEGEL_*` environment variables, with `Examples` for explicit test cases and `ReproduceFailure` for replaying a reported blob. `Generators` covers integers of every width, `BigInteger`, floats and doubles with the engine's bounds and special-value options, booleans, text (Unicode categories, codecs, alphabets), regular expressions, Lark grammars, emails, URLs, domains, IP addresses, `Guid`s, dates, times, `DateTime`s, `DateTimeOffset`s, `TimeSpan`s and time zones, lists, arrays, sets, dictionaries, tuples, permutations, subsequences, samples, `Nullable`/`Optional`, `OneOf`, `SampledFrom`, `Just`, `Composite`, `Recursive`, `Deferred`, records through `Generators.Records<T>()` and whole types through `Generators.ForType<T>()`. `Stateful.Run` drives a state machine whose `[Rule]` and `[Invariant]` methods it calls, sequentially or concurrently, with `Pool<T>` and `ConcurrentPool<T>` tracking the values rules produce.

The new `Hegel.FSharp` package is the F# frontend over the same engine: `Hegel.test` and `Hegel.run` take the test body as an F# function, the `Gen` module holds the generators (the scalars, plus `Gen.list`, `Gen.array`, `Gen.set`, `Gen.mapOf`, `Gen.option`, `Gen.voption`, `Gen.tuple2` to `Gen.tuple5`, `Gen.oneOf`, `Gen.item`, `Gen.map`, `Gen.filter`, `Gen.bind` and the `gen { }` builder), `Gen.auto<'T>` derives generators for records, unions, tuples and collections (recursive unions through their base cases) and `Gen.record<'T>` for one record with field overrides, and a failing test case prints as F# (`let xs_1 = [0; 0]`), including the values of C# generators drawn from F#.

The `Dialect` type is how a frontend chooses the language draws print in (`Dialect.Use`), and an assembly marked `[assembly: HegelInfrastructure]` is left out when Hegel reports where a failure originated.

`Hegel.Xunit` is a third package, for xUnit.net v2: `[HegelTest]` on a method taking a `TestCase` makes the method itself the property, one test in xUnit's test tree, with the shrunk test case in the test's output and the assertion as its failure. The attribute's `Profile` names the settings profile the test resolves from and `TestCases` sets its budget; `Skip` and `DisplayName` work as on a `[Fact]`. The method's own file and line are its test location, so the database key and Antithesis see it where it is written.

`Generator<T>` also has `Select`, `Where` and `SelectMany`, so generators can be built with C# query expressions. A later `from` clause can use any earlier value, which makes dependent generators read top to bottom instead of as nested `FlatMap` calls:

```csharp
var withIndex =
    from xs in Generators.Lists(Generators.Integers()).MinSize(1)
    from i in Generators.Integers().Min(0).Max(xs.Count - 1)
    select (xs, i);
```

`select` is `Map`, `where` is `Filter` (with the same rejection cost), and `let` clauses work too. A failing test case prints the query's final value.
