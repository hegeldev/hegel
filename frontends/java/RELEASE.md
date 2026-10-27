RELEASE_TYPE: minor

This release makes the engine's settings resolution the single source of truth for every setting, exposes the settings and profiles that had no Java API, types the exceptions for runs the engine aborts, replays every stored failure it is given, and fixes several native memory leaks.

Settings the test does not give are no longer sent to the engine. Before, every run sent `verbosity` (normal) and `reportMultipleFailures` (off), overriding whatever a `hegel.toml` profile or the environment said; now they come from the engine's profile like everything else. To support this:

- `Verbosity` gains a `DEFAULT` constant, which is the default for `Settings` and `@HegelTest(verbosity = ...)`.
- `@HegelTest(reportMultipleFailures = ...)` is now an `OptBoolean` rather than a `boolean`. Replace `reportMultipleFailures = true` with `reportMultipleFailures = OptBoolean.TRUE`.

The default example-database key of a `@HegelTest` is now the fully-qualified name of the test class running it and the method name (`com.example.CartTest.totalsAddUp`) rather than the bare method name, so same-named tests in different classes, and a test method inherited by several test classes, no longer share stored failures. Failures stored under the old key are not replayed; a `@HegelTest(name = ...)` keeps its explicit key.

New settings, each also an `@HegelTest` attribute:

- `stopOnFirstFailure(boolean)`: turned off, a run spends its whole test-case budget before reporting.
- `showStatistics(boolean)`: print the engine's statistics block at the end of the run.
- `unboundedChoices(boolean)`: lift the engine's per-test-case limit on choices, for very long test cases such as long-running concurrent state machines.
- `testLocation(TestLocation)`: where the test lives, which the engine reports each run's verdict under inside Antithesis. `@HegelTest` locates a test at its method, taking the source file and the first line of the method's body from its class file (line 0 if the class was compiled without line numbers), and `Hegel.test` and `Hegel.run` at their caller, so this is only needed to override that.

`Settings.suppressHealthCheck(...)` now replaces any earlier suppressions instead of adding to them, and only sends a suppression set when it is called, so a `hegel.toml` profile or `HEGEL_*` variable that suppresses health checks applies unless the test says otherwise. Calling it with no checks runs every health check even where the profile suppresses some. `HealthCheck` gains `NONE`, which suppresses nothing, so `@HegelTest(suppressHealthCheck = HealthCheck.NONE)` can do the same; an empty array, the annotation's default, still leaves the profile's suppressions in place.

`Settings.set(key, value)` sets any engine setting from its string form, as `hegel.toml` and the `HEGEL_*` environment variables spell it, such as `set("test_cases", "500")` or `set("suppress_health_check", "too_slow")`. The engine parses the value, so this reaches every setting it has, which suits settings read from a command line or a configuration file. String settings apply after the typed ones, in the order given. `dev.hegel.lowlevel.Libhegel` gains `settingsSetFromString` for it, which a third-party binding needs to implement.

`Database.defaultLocation()` (`@HegelTest(database = Database.DEFAULT_LOCATION)`) asks for the default database location, `.hegel/examples`, even under a profile that disables the database, such as `ci` or `workload`. `Database.unset()` still keeps the profile's database.

Settings profiles are now reachable from Java:

```java
Settings.registerProfile("nightly", Settings.fromProfile("ci").testCases(10_000));
Settings.setDefaultProfile("nightly");

@HegelTest(profile = "ci")
void property(TestCase tc) { ... }
```

`Settings.fromProfile(name)` starts from the named profile instead of the default one, `Settings.registerProfile` registers a profile process-wide, and `Settings.setDefaultProfile` changes which profile `new Settings()` starts from.

Stored failures are replayed differently:

- `Settings.reproduceFailure` takes any number of blobs (`reproduceFailure(String...)`), and `@HegelTest(reproduceFailure = ...)` is now a `String[]`; a single blob is still written `reproduceFailure = "..."`. Every blob is replayed, and the test fails with every failure they reproduce.
- When none of them fails, the run throws the new `DidNotReproduceException` (a `HegelException`) rather than a plain `HegelException`.
- A replay the engine aborts, such as on a failed health check, raises the same typed `HegelRunException` as any other run, rather than a plain `HegelException`.
- The reproducer printed with a failure names the right place for the blob: `reproduceFailure = "..."` for an `@HegelTest`, and `.reproduceFailure("...")` on the test's `Settings` for `Hegel.test` and `Hegel.run`.

This release also fixes several native memory leaks. Every list a test drew left its engine-side collection allocated, every `Pool` and `ConcurrentPool` stayed allocated after its test case ended, every thread that called into the engine, such as each worker thread of a concurrent stateful run, left its engine error context behind when it exited, and each stateful rule and each `repeat` repetition held engine memory until its test case ended. A thread's context is now freed once the thread has ended, including a thread of yours that draws through `TestCase.clone`. Long runs, and long state machines and repeated test bodies in particular, now hold steady memory. A `TestCase` a stateful rule was given can no longer be used once the rule has ended: drawing through it fails the run with an `IllegalArgumentException`.

An argument the engine rejects while a test case runs, such as `text().codec("nonsense")`, now fails the run at once with an `IllegalArgumentException` carrying the engine's explanation. Before, it was treated as a failure of the property, shrunk and reported as a counterexample. An `IllegalArgumentException` the test itself throws is still a failure of the property.

An errored run now raises an exception typed by the engine's classification of the error rather than by its message. `HegelRunException` (a `HegelException`) is the new base type for a run the engine aborted, with subtypes `HealthCheckFailure` (which now says which check failed through `healthCheck()`), `FlakyTestException` (nondeterminism under `NondeterminismStrictness.ERROR`) and `UnsatisfiableException` (no valid test case was ever generated). Code that caught `HegelException` still catches all of them. `RunReport` gains `errorKind()` and `failedHealthCheck()`, and `healthCheckFailed()` no longer depends on the message's wording.

`Label.of` and `Label.combine` are now computed by the engine, and the labels of the standard generators (`Label.LIST`, `Label.ONE_OF`, `Label.OPTIONAL`, `Label.FILTER` and so on) now use the same names as the other Hegel frontends, so their values change. Labels are only identities, so this changes nothing but the values of those constants. The constants are fixed values and need no engine, so when the engine cannot be loaded, building generators still works and the run fails with a `LibhegelException` saying why.

`Generator` gains `label()`, the label identifying a generator to the engine, as in the other Hegel frontends. A composite generator such as `lists(integers())` or `optional(text())` combines its own kind with its components' labels and draws in a span with that label, so a list of integers and a list of strings are told apart. A generator that does not override `label()` is labelled `Label.GENERATOR`. Each attempt of a filter now draws in a `Label.FILTER_ATTEMPT` span, and each pool draw in a `Label.POOL_VALUES_REUSABLE` or `Label.POOL_VALUES_CONSUMED` span. `Label` gains those constants and `DEFERRED` and `FROM_LARK`, and loses `LIST_ELEMENT`, `SET_ELEMENT`, `MAP_ENTRY` and `ENUM_VARIANT`, which no generator used.

`dev.hegel.lowlevel.Libhegel` gains a method for each newly bound C function: `collectionFree`, `poolFree`, `releaseThreadContext`, `settingsNewForProfile`, `settingsRegisterProfile`, `setDefaultProfile`, `settingsGetVerbosity`, `settingsStopOnFirstFailure`, `settingsShowStatistics`, `settingsUnboundedChoices`, `settingsTestLocation`, `runResultErrorKind`, `labelFromName` and `labelCombine`. A third-party binding needs to implement them.

All output a test case prints now goes through the engine, as in the other Hegel frontends, and printing a drawn value is now the generator's job rather than the value's. `TestCase.note` hands the note to the engine, and a top-level draw prints as a declaration, `var x = 11;` rather than `x = 11;`, into the engine's document for the case, which lays out a long list, map, array or record over several lines, one element per line, instead of on one long line. In a stateful test each rule's draws and notes are indented under its `Step N: rule {` line and closed by `}`, and each rule, like each repetition of `repeat`, names its draws afresh, so a rule that draws `x` prints `var x = ...;` every time it runs rather than `x_2`, `x_3` and so on; and in a concurrent one the engine stamps each worker's lines with `[worker N +X.XXXms]`, timed from the start of the test case rather than of the machine.

The generator describes each value as it draws it. The new `PrintableGenerator` interface draws and prints in one step, and every built-in generator implements it:

- Lists, sets, maps, arrays, tuples, optionals, records (`records` and `forType`), `oneOf`, `filter`, `flatMap`, `recursive` and `deferred` print as they draw, element by element and component by component, so an element's own printing shows up inside its container. A rejected filter attempt or duplicate element prints nothing.
- The leaf generators, and `map`, `just`, `sampledFrom` and `composite`, print the value they produce as Java: strings and characters as escaped literals, so a drawn string containing a newline stays on its line, records (including `Tuple2` to `Tuple8`) as a call of their constructor, `new Tuple2("a b", "")`, and optionals as `Optional.of("a b")` or `Optional.empty()`, rather than as their `toString()`; collections print as `[a, b]` and maps as `{k: v}`. Generated records, tuples and optionals print the same way, each component as its own generator prints it. A composite's own draws are not printed, only what it returns.

`TestCase.draw` now needs a printable generator. A hand-written `Generator`, such as `tc -> 1`, draws silently: drawing it with `draw`, or drawing a list, record override or other structure containing it, now fails the run with an `IllegalArgumentException` explaining how to fix it, where before its value was printed through `toString()`. Make it printable, or draw it with `drawSilent`:

```java
Generator<Point> points = tc -> new Point(tc.drawSilent(integers()), tc.drawSilent(integers()));

tc.draw(points.printAsRepr(), "p");                                         // var p = new Point(0, 0);
tc.draw(points.printWith((p, printer) -> printer.text("origin()")), "p");   // var p = origin();
tc.draw(integers().map(Key::of).printAsCall("Key.of"), "k");                // var k = Key.of(3);
```

To print a value as it is built, implement `PrintableGenerator` and draw each part with the new `TestCase.drawAndPrint`, so that it prints itself in place, or use the new `Generators.composite((tc, printer) -> ...)`:

```java
PrintableGenerator<Point> points = (tc, p) -> {
    p.beginGroup("Point.of(");
    int x = tc.drawAndPrint(integers(), p);
    p.separator();
    int y = tc.drawAndPrint(integers(), p);
    p.endGroup(")");
    return Point.of(x, y);
};

Point point = tc.draw(points, "point");   // var point = Point.of(3, -1);
```

`Printer` is the engine's layout engine: `text`, `repr`, `reflow`, `breakable`, `ifBreak`, `hardBreak`, `beginGroup`/`endGroup` and `group`, `seq`, `separator`, `shiftIndent`, `comment`, `speculate` (print an attempt and commit or abort it), `deferred` (a region filled in after the draw, for a value whose description is only known once it is used) and `shouldPrint`. `Printer.noop()` discards everything, and `Document` lays out a standalone document, for trying printing code outside a test.

Other API changes that come with this:

- `Generator.doDraw` is now public API, the silent draw a hand-written generator implements, and `Generator` is a functional interface.
- `Generator.map` returns `MappedGenerator`, and `filter` and `flatMap` return `PrintableGenerator`. The `Generators` factories that returned `Generator`, and `Pool.reusable()`, `Pool.consuming()` and their `ConcurrentPool` counterparts, now return `PrintableGenerator`. Code is source-compatible but must be recompiled.
- The constructors of `OptionalGenerator` and the typed-tuple form of `TupleGenerator` take the name of the type they print.
- A draw that throws now uses up its name, so the next draw with the same label is numbered as if it had succeeded.

With several distinct failures, the `AssertionError` that aggregates them now begins `Property-based test failed with N distinct failures.`, the wording of the header printed above them and of the other Hegel frontends, rather than `Hegel found N distinct failing examples:`. Under `Verbosity.QUIET` that header, and the blank line between failures, are no longer printed.

The `Reporter` and `Failure` APIs change to match:

- `Reporter.draw(label, value, finalReplay)` and `Reporter.note(message, finalReplay)` are replaced by `Reporter.caseOutput(line, finalReplay)`, which receives the case's rendered output one line at a time. Under `Verbosity.VERBOSE` a case's lines now arrive once its body has finished rather than as each draw is made.
- `Failure.notes()` is replaced by `Failure.output()`, the counterexample's printed report as a list of lines. `Failure.draws()` still gives the drawn values.

`dev.hegel.lowlevel.Libhegel` gains `testCaseBlock`, `note`, `testCasePrinter`, `printerNew`, `printerFree`, `printerText`, `printerBreakable`, `printerHardBreak`, `printerBeginGroup`, `printerEndGroup`, `printerReflow`, `printerBeginSpeculative`, `printerCommitSpeculative`, `printerAbortSpeculative`, `printerResolve`, `printerValue`, `printerIfBreak`, `printerComment`, `printerShiftIndent`, `printerDeferred`, `printerIsLive`, `printerOptionsNew`, `printerOptionsSetMaxWidth`, `printerOptionsFree` and `printerNew(options)`, for the engine's printer. A third-party binding needs to implement them.

A filtered consuming pool draw, `pool.consuming().filter(...)` on a `Pool` or `ConcurrentPool`, now consumes only the value it yields. Before, every value an attempt drew and the filter rejected was consumed too, and lost from the pool. Each attempt still consumes its value, so no attempt draws a value an earlier one rejected, and once the attempts are over the engine puts every rejected value back (`hegel_pool_restore`). Sets and maps now also free their engine-side collection after each draw, as lists already did.

`TestCase` gains the rest of the test-case API the other Hegel frontends have:

- `event(label)` and `eventValue(label, value)` record events and numeric observations for the engine's end-of-run statistics report, printed when `showStatistics` is on.
- `reject()` rejects the test case unconditionally, like `assume(false)`.
- `drawSilent(generator)` draws a value without printing it in the counterexample.
- `clone()` returns a handle with its own independent stream of choices, for drawing from another thread, and `setWorker(worker)` makes the engine attribute a handle's output to worker `worker`.
- `repeat(body)` runs `body` until the engine decides the test case is long enough, so a test that is logically an infinite loop shrinks repetition by repetition.

`Generators.recursive(base, extend)` generates recursively defined data, such as trees or JSON documents, through the engine's recursion support: `base` generates the leaves and `extend` builds a branch from a generator of sub-values. The engine decides where the value branches, steers each value toward a size drawn from across its leaf budget, and lets the shrinker replace a tree with one of its subtrees. Limit the shape with `maxDepth` (default 32) and `maxLeaves` (default 100). `Generators.deferred()` remains for mutually recursive generators.

`dev.hegel.lowlevel.Libhegel` gains `poolRestore`, `newRecursion`, `recursionBranch`, `recursionLeaf`, `recursionRetry`, `recursionFinish`, `recursionFree`, `event`, `eventValue`, `generateIntegerBig`, `settingsGetPhases` and `settingsGetReportMultipleFailures`. A third-party binding needs to implement them.

New generators, matching the other Hegel frontends:

- `Generators.bigIntegers()` generates `BigInteger` values, narrowed with `min` and `max`, which may be of any magnitude. An unset bound reaches `2^127` values beyond the other bound, or beyond zero when that is further, so `bigIntegers()` alone draws from `[-2^127, 2^127)`, as in the other Hegel frontends. `Generators.forType(BigInteger.class)` derives it.
- `lists(...).unique()` and `lists(...).uniqueBy(key)` generate lists whose elements, or their keys, are distinct, comparing arrays by their contents.
- `Generators.permutations(elements)` generates reorderings of a fixed list, shrinking toward the original order.
- `Generators.subsequences(elements)` generates sublists of a fixed list that keep its order, and `Generators.samples(elements)` lists of elements chosen from it, with replacement by default or without it after `.withoutReplacement()`. Both take `.minSize(n)` and `.maxSize(n)`, and shrink toward fewer elements from earlier in the list.
- `Generators.weightedBooleans(p)` generates `true` with probability `p`, which must be between 0 and 1.
- `Generators.arrays(element, length, newArray)` generates arrays of a fixed length, such as `arrays(integers(), 3, Integer[]::new)`.
- `Generators.nulls()` always generates `null`.
- `text().alphabet(chars)` draws only from the given characters, and `text().codec(name)` restricts to the characters `"ascii"`, `"latin-1"` (or `"iso-8859-1"`) or `"utf-8"` can encode. An alphabet cannot be combined with the other character restrictions.
- `fromRegex(pattern).alphabet(characters)` restricts every character of a generated string, padding included, to those `characters` can generate, such as `characters().codec("ascii")`.

`Generators.forType(cls)` and `records(cls)` now reject a record that refers back to itself through its components, directly or through other records and collections, such as `record Node(int value, List<Node> children)`. Deriving a generator for one recursed without bound; it now fails with a `HegelException` naming the record and the cycle, and pointing at `Generators.recursive`, `Generators.deferred()` and `records(Node.class).with(...)`, which remain the ways to build such a generator.

`text()` now leaves character validation to the engine. `text().categories("Cs")` and `text().categories("C")` were rejected when the generator was built because they name surrogates; the engine never generates surrogates, so `categories("C")` now generates the other control, format, private-use and unassigned characters, and a set of categories that leaves no characters fails when drawn from with an `IllegalArgumentException`, as an unknown codec does.

`optional`, `optionalInts`, `optionalLongs` and `optionalDoubles` now draw under the engine's standard `hegel.optional` label, as the other frontends' optional generators do, rather than as a `oneOf`. Values still shrink toward empty, but stored failures involving optional values may no longer replay.

Explicit examples are now supported, so `Phase.EXPLICIT` is no longer reserved. `Settings.examples(Example...)`, or `@HegelTest(examples = {"methodName", ...})` naming methods that return an `Example` or an `Iterable` of them, gives values a test runs with before any are generated:

```java
Hegel.test(tc -> {
    List<Integer> xs = tc.draw(lists(integers()), "xs");
    assertEquals(xs, reverse(reverse(xs)));
}, new Settings().examples(Example.of(List.of()), Example.of().with("xs", List.of(1, 1))));
```

A draw takes the value `Example.with` gives for its name, or otherwise the example's next positional value. A failing example is reported before any generated failure; with `reportMultipleFailures` off it ends the run, and with it on it is reported alongside the engine's failures, merged by origin. An example that lacks a value for a draw, has values no draw takes, or fails an assumption (`assume` or `reject`, inside `repeat` or not) is a usage error: explicit test cases must not be rejected. Inside an example a clone draws from the same example, `setWorker` does nothing and `repeat` runs its body once; targets and events are ignored, and a silent draw, which the example has no value for, is a usage error. Under `Verbosity.QUIET` a failing example's `Failure.output()` is empty, as a generated failure's is. When an explicit example fails and the engine's run then errors (a health check, say), the run is `FAILED` and `RunReport.error()` and `errorKind()` also describe the error.

`Generators.fromLark(grammar)` generates strings matching a grammar written in the syntax of the [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing library, as the other Hegel frontends' grammar generators do:

```java
String expression = tc.draw(fromLark("""
        ?start: sum
        ?sum: product | sum "+" product
        ?product: atom | product "*" atom
        ?atom: NUMBER | "(" sum ")"
        %import common.NUMBER
        %import common.WS
        %ignore WS
        """).maxSize(200));
```

`LarkGenerator` configures it: `start(rule)` picks the start rule, `explicit(name, generator)` draws a terminal's values from a generator of yours (required for a terminal the grammar `%declare`s), `alphabet(characters)` constrains the characters terminals draw from their patterns, `maxSize` and `maxDepth` bound the strings, and `importPath(dir)` sets the directory `%import`ed grammars are read from. The engine splices explicit terminals' values in, keeps the whole string under `maxSize`, and rejects the test case when the values cannot be made to fit.

Text and binary generators with no `maxSize` now leave the maximum length to the engine, which applies the same default as before (100, or `minSize + 100` for a larger minimum), and `filter` takes its number of attempts from the engine, so both stay in step with the other Hegel frontends. `Abi` gains `MAX_SIZE_DEFAULT`, `FILTER_ATTEMPTS` and `E_NOT_FOUND`.

`dev.hegel.lowlevel.Libhegel` gains `grammarBuilderNew`, `grammarBuilderFile`, `grammarBuilderProvide`, `grammarBuilderFree`, `grammarNew`, `grammarFree`, `grammarDrawNew`, `grammarDrawNext`, `grammarDrawProvide`, `grammarDrawResult` and `grammarDrawFree`. A third-party binding needs to implement them.

A draw label containing a newline now fails the run with an `IllegalArgumentException`. Errors the engine reports while printing a test case's output, or for `TestCase.note`, are now raised instead of ignored, except that an error printing a failing test case's output is attached to the test's own exception as a suppressed exception rather than replacing it. Output written after the case's document has been read, such as by a straggling thread, is still silently dropped.

With `Verbosity.VERBOSE` or `Verbosity.DEBUG`, the printing reporter now says why a test case stopped short, as the other Hegel frontends do: `Test case stopped: failed assumption` or `Test case stopped: out of data`. An explicit example is no longer introduced with a `Running explicit Example(...)` line through `Reporter.caseOutput`: its drawn values and notes are reported like a generated case's.

This release also replaces the `excludeMin(boolean)` and `excludeMax(boolean)` methods of `DoubleGenerator` and `FloatGenerator` with `minValueExclusive(v)` and `maxValueExclusive(v)`, which take the bound directly, as the other Hegel frontends do:

```java
// before
doubles().min(0).max(1).excludeMin(true).excludeMax(true)

// after
doubles().minValueExclusive(0).maxValueExclusive(1)
```

`min` and `minValueExclusive` set the same bound, so whichever is called last wins (likewise `max` and `maxValueExclusive`). An exclusive bound can no longer be set without a bound value, so that `IllegalArgumentException` no longer exists; the remaining validation (an exclusive `+Infinity` minimum, an exclusive `-Infinity` maximum, or exclusive bounds on a single-point range) is unchanged, and its messages now name the new methods. The public six-argument constructors of `DoubleGenerator` and `FloatGenerator` are replaced by no-argument ones; `Generators.doubles()` and `Generators.floats()` are the usual way to get one.

A `java.util.Set` now prints as `Set.of(1, 2)`, where it printed as `[1, 2]`, indistinguishable from a list, and a `UUID` prints as `UUID.fromString("f47ac10b-58cc-4372-a567-0e02b2c3d479")`, where it printed bare. This applies to the values `sets()` and `uuids()` draw and to sets and UUIDs inside any other printed value.

`TestCase.repeat` now has the engine decide how many repetitions run and which one failed, through libhegel's new `hegel_repeat_t` functions, rather than drawing those choices itself. The choices are the same, so an existing reproduce blob still replays.

**Stateful testing.** `Stateful.run` now finds `@Rule` and `@Invariant` methods declared in a machine's superclasses, not only in its own class, so a machine can extend a base machine. A method that takes a `TestCase` hides a method of the same name declared further up the hierarchy, so an override is a rule only if it repeats the annotation; rules and invariants are still ordered by name. A failing test's report now names the rule or invariant the failure escaped from (`Rule pop failed:`, `Invariant sizesAgree failed:`) right before the failure itself, and an invariant's draws and notes are indented like a rule's, as in the other Hegel frontends.
