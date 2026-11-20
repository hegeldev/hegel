# Changelog

## 0.4.1 - 2026-11-20

This patch sets a lower bound for the probability of checking a sampled invariant.

## 0.4.0 - 2026-11-19

This release moves every date and time generator onto the engine's temporal draws and into a temporal namespace, under the names every Hegel frontend shares: the generators are on `Generators.Temporal`, and their types are in the namespace `Hegel.Temporal`. It adds `Instants()`, `Spans()` and `ZonedDateTimes()`, options on the existing generators, and zone sources, which say whose time zone data a draw reads. A few changes can break existing code or tests.

The generators are `Generators.Temporal.Dates()` (`DateOnly`), `Times()` (`TimeOnly`), `DateTimes()` (`DateTime`), `Instants()` (`DateTimeOffset` with a zero offset), `Offsets()` (`TimeSpan`), `Durations()` (`TimeSpan`), `Spans()` (`CalendarSpan`), `Zones()` (`TimeZoneInfo`) and `ZonedDateTimes()` (`ZonedDateTime`). The six date and time generators that were directly on `Generators` are deprecated in their favour and keep working with the defaults they had, and `DateTimeGenerator.WithTimeZones` is deprecated in favour of `ZonedDateTimes()`:

```csharp
// before
Generators.Dates()
Generators.Times()
Generators.DateTimes()
Generators.TimeSpans()
Generators.UtcOffsets()
Generators.TimeZones()
Generators.DateTimes().WithTimeZones(Generators.TimeZones())

// after
Generators.Temporal.Dates()
Generators.Temporal.Times()
Generators.Temporal.DateTimes()
Generators.Temporal.Durations().Min(TimeSpan.Zero)
Generators.Temporal.Offsets()
Generators.Temporal.Zones()
Generators.Temporal.ZonedDateTimes().Zones(Generators.Temporal.Zones()).Map(zoned => zoned.ToDateTimeOffset())
```

In F#, the generators are in the module `Gen.Temporal` (`date`, `time`, `dateTime`, `dateTimeOffset`, `instant`, `offset`, `duration`, `span`, `zone ()` and `zonedDateTime ()`) and their types in the namespace `Hegel.FSharp.Temporal`. `Gen.dateOnly`, `Gen.timeOnly`, `Gen.dateTime`, `Gen.dateTimeOffset`, `Gen.timeSpan`, `Gen.utcOffset` and `Gen.timeZone` are deprecated in the same way.

What can break:

- `DateGenerator`, `TimeGenerator` and `DateTimeGenerator` are now in the namespace `Hegel.Temporal`, so code that names one of those types needs `using Hegel.Temporal;` (F# code needs nothing: `Hegel.FSharp` still has them).
- The classes `TimeSpanGenerator` and `UtcOffsetGenerator` are gone. `TimeSpans()` returns a `DurationGenerator` and `UtcOffsets()` an `OffsetGenerator`, which have the same `Min` and `Max` methods, and `TimeZones()` returns a `ZoneGenerator`, so only code that names one of the old types needs changing (and code compiled against an earlier version needs recompiling).
- A bad argument to a date or time generator is now an `ArgumentException` carrying the engine's message, such as `generate_date requires min_value <= max_value, got [2020-12-31, 2020-01-01]`. It used to be an `ArgumentOutOfRangeException`, which is a subclass, so a test that expects that exact type or the old message needs updating.
- `Durations()` draws durations of either sign, over the whole range of `TimeSpan`. The deprecated `TimeSpans()` still starts at zero.
- `Offsets().Min` and `Max` take any whole number of seconds up to 25:59:59 either side of UTC, where `UtcOffsets()` refused a bound beyond 14:00 or with seconds in it. The defaults are unchanged (whole minutes from -14:00 to +14:00, all a `DateTimeOffset` accepts), so a bound beyond 14:00 needs the other bound given too, and a bound with seconds needs `.Granularity(OffsetGranularity.Seconds)` to be reached.
- `Zones()`, and so the deprecated `TimeZones()` and `Generators.ForType<TimeZoneInfo>()`, draws the zones of a zone source: by default every IANA name that `TimeZoneInfo.FindSystemTimeZoneById` finds on this system, link names such as `Asia/Calcutta` included and legacy names such as `EST5EDT` left out, shrinking toward `UTC`. `TimeZones()` used to draw from `TimeZoneInfo.GetSystemTimeZones()`, whose ids on Windows are Windows ids, shrinking toward the first id in alphabetical order.

What is new:

- `Dates()` takes `.Weekdays(DayOfWeek.Saturday, DayOfWeek.Sunday)` and `.Months(1, 2, 12)`.
- `Times()`, `DateTimes()`, `Instants()` and `Durations()` take `.Precision(Precision.Seconds)`, for values that are whole microseconds, milliseconds, seconds, minutes or hours. The default, `Precision.Ticks`, is .NET's own 100 ns resolution.
- `Offsets()` takes `.Granularity(...)`: whole hours, quarter-hours, minutes (the default) or seconds.
- `Spans()` draws `CalendarSpan`, a new struct holding a count of each unit from years to nanoseconds, which .NET has no type for. `.Years(0, 5).Months(0, 11)` picks the units drawn and their bounds, and `.Sign(...)` whether every unit shares one sign (the default), each has its own, or none is negative.
- `Zones()` takes `.Names("Europe/London", "Asia/Tokyo")`, `.AllowLinks(false)`, `.AllowLegacy(true)` and `.Source(source)`.
- `ZonedDateTimes()` draws `ZonedDateTime`, a new struct holding a wall-clock time (`LocalDateTime`), its `Zone`, the `Offset` in force, the `Instant` it names and a `Kind` that says whether the zone's clocks showed that time once, twice (`WallTimeKind.Earlier`, `Later`) or never (`Skipped`). One draw in twenty on average lands on or beside one of the zone's transitions, so repeated times are reached without help; skipped ones are drawn once `.AllowImaginary(true)` asks for them. `.Min` and `.Max` bound the wall clock when given a `DateTime` and the instant when given a `DateTimeOffset`; `.Ambiguous(Ambiguous.Earlier)` fixes which reading of a repeated time is drawn; `.Zones(generator)` takes any generator of `TimeZoneInfo`.
- `ZoneSource` is the time zone data those two generators read. `ZoneSource.Host`, the default, is this system's zones with the offsets `TimeZoneInfo` reports, so a drawn value agrees with what code using `TimeZoneInfo` computes. `ZoneSource.Bundled` is the IANA database built into the engine, the same on every machine. `ZoneSource.FromTimeZones(...)` draws your own `TimeZoneInfo` objects, `TimeZoneInfo.CreateCustomTimeZone` ones included, and `ZoneSource.CreateBuilder()` builds a source from TZif files, `TimeZoneInfo` objects, offset functions (`ZoneOracle`), links and a loader that supplies zones on demand. `.Source(source)` gives one to a generator and `ZoneSource.SetDefault(source)` to every generator made afterwards.
- `Generators.ForType<T>()` knows `ZonedDateTime` and `CalendarSpan`, and both print in a failing test case as the code that builds them.

The generators check their settings with the engine as each is given. A call that completes a mistake among the settings given so far throws, whatever order they came in: `Dates().Min(a).Max(b)` and `Dates().Max(b).Min(a)` both throw at the second call when `a` is after `b`. A setting that conflicts only with a default which a later call could still change, such as `Offsets().Min(TimeSpan.FromHours(15))` on its own, is reported by the first draw.

## 0.3.0 - 2026-11-18

This release moves the printing of a test case's draws and notes into the engine, which keeps the memory of a long-running test case flat at every verbosity. A run with `Verbosity.Verbose` now streams each test case's output to the reporter's `EngineOutput` as it is printed, so a stateful test prints step by step (and a concurrent one round by round) instead of all at once when the test case ends. Failure reports at the default verbosity are unchanged.

A verbose run now also prints one line after each failing test case: the file and line in your test that threw, then the exception's type and the first line of its message, such as `/src/Tests.cs:16:13: InvalidOperationException: too big: 331`. `Verbosity.Debug` prints the full exception, stack trace included, instead.

`Reporter.Silent` now tells the engine the run has no output destination, so a silent run spends nothing on verbose or debug output.

A run with `Verbosity.Quiet` now leaves `Failure.Output` empty.

`TestCase.IsFinal` is removed. The engine now decides which test cases' output it uses, and that set (every test case of a verbose run, and the runs a failure report is built from) no longer matches what the name promised.

## 0.2.3 - 2026-11-16

This release updates the bundled libhegel engine to 0.51.6.

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
