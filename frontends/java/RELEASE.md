RELEASE_TYPE: minor

This release makes the engine's settings resolution the single source of truth for every setting, exposes the settings and profiles that had no Java API, types the exceptions for runs the engine aborts, and fixes three native memory leaks.

Settings the test does not give are no longer sent to the engine. Before, every run sent `verbosity` (normal) and `reportMultipleFailures` (off), overriding whatever a `hegel.toml` profile or the environment said; now they come from the engine's profile like everything else. To support this:

- `Verbosity` gains a `DEFAULT` constant, which is the default for `Settings` and `@HegelTest(verbosity = ...)`.
- `@HegelTest(reportMultipleFailures = ...)` is now an `OptBoolean` rather than a `boolean`. Replace `reportMultipleFailures = true` with `reportMultipleFailures = OptBoolean.TRUE`.

The default example-database key of a `@HegelTest` is now its fully-qualified class name and method name (`com.example.CartTest.totalsAddUp`) rather than the bare method name, so same-named tests in different classes no longer share stored failures. Failures stored under the old key are not replayed; a `@HegelTest(name = ...)` keeps its explicit key.

New settings, each also an `@HegelTest` attribute:

- `stopOnFirstFailure(boolean)`: turned off, a run spends its whole test-case budget before reporting.
- `showStatistics(boolean)`: print the engine's statistics block at the end of the run.
- `unboundedChoices(boolean)`: lift the engine's per-test-case limit on choices, for very long test cases such as long-running concurrent state machines.
- `testLocation(TestLocation)`: where the test lives, which the engine reports each run's verdict under inside Antithesis. `@HegelTest` locates a test at its method, and `Hegel.test` and `Hegel.run` at their caller, so this is only needed to override that.

Settings profiles are now reachable from Java:

```java
Settings.registerProfile("nightly", Settings.fromProfile("ci").testCases(10_000));
Settings.setDefaultProfile("nightly");

@HegelTest(profile = "ci")
void property(TestCase tc) { ... }
```

`Settings.fromProfile(name)` starts from the named profile instead of the default one, `Settings.registerProfile` registers a profile process-wide, and `Settings.setDefaultProfile` changes which profile `new Settings()` starts from.

This release also fixes three native memory leaks. Every list a test drew left its engine-side collection allocated, every `Pool` and `ConcurrentPool` stayed allocated after its test case ended, and every thread that called into the engine, such as each worker thread of a concurrent stateful run, left its engine error context behind when it exited. A thread's context is now freed once the thread has ended, including a thread of yours that draws through `TestCase.fork`. Long runs, and long concurrent state machines in particular, now hold steady memory.


An errored run now raises an exception typed by the engine's classification of the error rather than by its message. `HegelRunException` (a `HegelException`) is the new base type for a run the engine aborted, with subtypes `HealthCheckFailure` (which now says which check failed through `healthCheck()`), `FlakyTestException` (nondeterminism under `NondeterminismStrictness.ERROR`) and `UnsatisfiableException` (no valid test case was ever generated). Code that caught `HegelException` still catches all of them. `RunReport` gains `errorKind()` and `failedHealthCheck()`, and `healthCheckFailed()` no longer depends on the message's wording.

`Label.of` and `Label.combine` are now computed by the engine, and the labels of the standard generators (`Label.LIST`, `Label.ONE_OF`, `Label.OPTIONAL`, `Label.FILTER` and so on) now use the same names as the other Hegel frontends, so their values change. Labels are only identities, so this changes nothing but the values of those constants.

`dev.hegel.lowlevel.Libhegel` gains a method for each newly bound C function: `collectionFree`, `poolFree`, `releaseThreadContext`, `settingsNewForProfile`, `settingsRegisterProfile`, `setDefaultProfile`, `settingsGetVerbosity`, `settingsStopOnFirstFailure`, `settingsShowStatistics`, `settingsUnboundedChoices`, `settingsTestLocation`, `runResultErrorKind`, `labelFromName` and `labelCombine`. A third-party binding needs to implement them.

All output a test case prints now goes through the engine, as in the other Hegel frontends. `TestCase.note` hands the note to the engine, and a top-level draw prints into the engine's document for the case, which lays out a long list, map, array or record over several lines, one element per line, instead of on one long line. Strings and characters print as escaped Java literals, so a drawn string containing a newline stays on its line. In a stateful test each rule's draws and notes are indented under its `Step N: rule {` line and closed by `}`, and in a concurrent one the engine stamps each worker's lines with `[worker N +X.XXXms]`, timed from the start of the test case rather than of the machine.

The `Reporter` and `Failure` APIs change to match:

- `Reporter.draw(label, value, finalReplay)` and `Reporter.note(message, finalReplay)` are replaced by `Reporter.caseOutput(line, finalReplay)`, which receives the case's rendered output one line at a time. Under `Verbosity.VERBOSE` a case's lines now arrive once its body has finished rather than as each draw is made.
- `Failure.notes()` is replaced by `Failure.output()`, the counterexample's printed report as a list of lines. `Failure.draws()` still gives the drawn values.

`dev.hegel.lowlevel.Libhegel` gains `testCaseBlock`, `note`, `testCasePrinter`, `printerNew`, `printerFree`, `printerText`, `printerBreakable`, `printerHardBreak`, `printerBeginGroup`, `printerEndGroup`, `printerReflow`, `printerBeginSpeculative`, `printerCommitSpeculative`, `printerAbortSpeculative`, `printerResolve` and `printerValue`, for the engine's printer. A third-party binding needs to implement them.

A filtered consuming pool draw, `pool.consuming().filter(...)` on a `Pool` or `ConcurrentPool`, now consumes only the value it yields. Before, every value an attempt drew and the filter rejected was consumed too, and lost from the pool. Each attempt still consumes its value, so no attempt draws a value an earlier one rejected, and once the attempts are over the engine puts every rejected value back (`hegel_pool_restore`). Sets and maps now also free their engine-side collection after each draw, as lists already did.

`TestCase` gains the rest of the test-case API the other Hegel frontends have:

- `event(label)` and `eventValue(label, value)` record events and numeric observations for the engine's end-of-run statistics report, printed when `showStatistics` is on.
- `reject()` rejects the test case unconditionally, like `assume(false)`.
- `drawSilent(generator)` draws a value without printing it in the counterexample.
- `fork(worker)` returns a handle with its own independent stream of choices, for drawing from another thread. The engine attributes its output to worker `worker`.
- `repeat(body)` runs `body` until the engine decides the test case is long enough, so a test that is logically an infinite loop shrinks repetition by repetition.

`Generators.recursive(base, extend)` generates recursively defined data, such as trees or JSON documents, through the engine's recursion support: `base` generates the leaves and `extend` builds a branch from a generator of sub-values. The engine decides where the value branches, steers each value toward a size drawn from across its leaf budget, and lets the shrinker replace a tree with one of its subtrees. Limit the shape with `maxDepth` (default 32) and `maxLeaves` (default 100). `Generators.deferred()` remains for mutually recursive generators.

`dev.hegel.lowlevel.Libhegel` gains `poolRestore`, `newRecursion`, `recursionBranch`, `recursionLeaf`, `recursionRetry`, `recursionFinish`, `recursionFree`, `event`, `eventValue`, `generateIntegerBig`, `settingsGetPhases` and `settingsGetReportMultipleFailures`. A third-party binding needs to implement them.

New generators, matching the other Hegel frontends:

- `Generators.bigIntegers()` generates `BigInteger` values of any magnitude, narrowed with `min` and `max`. An unset bound reaches `2^127` values beyond the other bound, or beyond zero. `Generators.forType(BigInteger.class)` derives it.
- `lists(...).unique()` and `lists(...).uniqueBy(key)` generate lists whose elements, or their keys, are distinct.
- `Generators.permutations(elements)` generates reorderings of a fixed list, shrinking toward the original order.
- `Generators.arrays(element, length, newArray)` generates arrays of a fixed length, such as `arrays(integers(), 3, Integer[]::new)`.
- `Generators.nulls()` always generates `null`.
- `text().alphabet(chars)` draws only from the given characters, and `text().codec(name)` restricts to the characters `"ascii"`, `"latin-1"` (or `"iso-8859-1"`) or `"utf-8"` can encode. An alphabet cannot be combined with the other character restrictions.
- `fromRegex(pattern).alphabet(characters)` restricts every character of a generated string, padding included, to those `characters` can generate, such as `characters().codec("ascii")`.

`optional`, `optionalInts`, `optionalLongs` and `optionalDoubles` now draw under the engine's standard `hegel.optional` label, as the other frontends' optional generators do, rather than as a `oneOf`. Values still shrink toward empty, but stored failures involving optional values may no longer replay.

Explicit examples are now supported, so `Phase.EXPLICIT` is no longer reserved. `Settings.examples(Example...)`, or `@HegelTest(examples = {"methodName", ...})` naming methods that return an `Example` or an `Iterable` of them, gives values a test runs with before any are generated:

```java
Hegel.test(tc -> {
    List<Integer> xs = tc.draw(lists(integers()), "xs");
    assertEquals(xs, reverse(reverse(xs)));
}, new Settings().examples(Example.of(List.of()), Example.of().with("xs", List.of(1, 1))));
```

A draw takes the value `Example.with` gives for its name, or otherwise the example's next positional value. A failing example is reported before any generated failure; with `reportMultipleFailures` off it ends the run, and with it on it is reported alongside the engine's failures, merged by origin. An example that lacks a value for a draw, has values no draw takes, or fails an assumption is a usage error. When an explicit example fails and the engine's run then errors (a health check, say), the run is `FAILED` and `RunReport.error()` and `errorKind()` also describe the error.

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
