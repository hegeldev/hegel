RELEASE_TYPE: minor

This release makes the engine the source of every setting a TypeScript test does not set itself, adds the settings that were missing, changes what a failing or aborted run throws and how a failure is reported, and adds the test-case features the other Hegel frontends have.

**Settings.** A run now starts from the settings the engine resolves (the default profile, `hegel.toml` and the `HEGEL_*` environment variables) and sets only the fields you give. Previously every run overwrote the test count, verbosity, derandomization and multiple-failure reporting with the frontend's own defaults, so `HEGEL_TEST_CASES` or a `hegel.toml` profile had no effect on them, and the frontend detected CI itself. CI and Antithesis detection is now the engine's: on a CI server the shipped `ci` profile applies (derandomized, database off, `too_slow` suppressed).

- Every field of `Settings` is optional.
- New settings: `profile`, `phases` (with the new `Phase` enum), `printBlob`, `stopOnFirstFailure`, `nondeterminismStrictness` (`NondeterminismStrictness`), `backend` (`Backend`), `showStatistics` and `unboundedChoices`.
- `seed` accepts a `bigint`, so the whole 64-bit range, and `null` clears a seed set by a profile.
- New `hegel.registerProfile(name, settings)` and `hegel.setDefaultProfile(name)`.
- `Database.unset` is replaced by `Database.default`, which resets the database to the engine's default location over whatever the profile says. Leave `database` out to keep the profile's choice.
- `suppressHealthCheck: []` now turns every health check back on, where before it changed nothing.

**Test identity.** A test's database key is now its file, relative to the working directory (in a browser, the path of its URL), plus its name, derived from where `hegel.test` was called, instead of the source text of the test function, which changed with every edit to the test body. The name is the test function's own name; for an anonymous one it is the name Vitest or Jest gives the running test (`"describe > test"`), else the enclosing function's name, else the line it was called from. When several `hegel.test` calls on different lines borrow the same runner or enclosing-function name, each after the first gets its position appended (`"describe > test (2)"`), so they keep separate saved failures. So a key, and the seed `derandomize` derives from it, stays the same when the project is checked out somewhere else. Saved failures from earlier versions are not replayed. The same location is passed to the engine, which reports the test's verdict to Antithesis when running inside it; the frontend no longer writes `sdk.jsonl` itself. The `Hegel` builder and the `TestLocation` type are now exported, with `.databaseKey(key)` and `.testLocation(location)` to override either:

```ts
new hegel.Hegel((tc) => {
  tc.draw(gs.integers());
})
  .settings({ testCases: 500 })
  .databaseKey("integers")
  .runSync();
```

**Errors.** A failing test now throws its own error, with its class, `cause` and an assertion library's expected and actual values intact, instead of a new `Error` whose message was the original message followed by the failure's origin. With `reportMultipleFailures`, several distinct failures are thrown together as an `AggregateError` rather than only the last one. A failure the engine could only confirm as nondeterministic prints its caveat, and reports the error seen while searching if replaying it does not fail again. A run the engine aborts throws a `RunError`: `FailedHealthCheckError` (with the failed `healthCheck`), `FlakyError`, `UnsatisfiableError`, or a plain `RunError`.

**Failure reports.** A failing example is now printed the same way on Node and in browsers, laid out by the engine: each drawn value as a line of JavaScript (`const draw_1 = [0, 0];`), numbered in draw order, with the notes between them, and with the `printBlob` setting (on by default) the line that replays the failure. Values print as JavaScript expressions (`-0`, `1n`, `new Map([[1, "a"]])`, `new Date("…")`, `Point { x: 1 }`), with their structure given to the engine's layout: an array, object, `Map` or `Set` too long for the line breaks one item per line, indented, with a trailing comma. A class can print its own instances with a `[hegel.prettyPrint](printer)` method, writing through the new `hegel.PrettyPrinter` (text, break points, groups, comments and nested values). Previously Node printed values with `util.inspect` and browsers with `console.error`'s own formatting. The test body now runs once per test case: the frontend no longer replays the failing example a final time to print it, and `TestCase.isLastRun` is removed. A stateful test reports each step as `Step N: <rule> {`, with the step's draws and notes indented beneath it, and names a failing rule or invariant (`Rule pop failed:`, `Invariant nonNegative failed:`).

```text
const draw_1 = [0, 0];

To reproduce this failure, replay it with the Hegel builder:
    .reproduceFailure("AXicY2VgYGBkZOBiZEBhMAAAAd8AIQ==")
```

**Replaying and explicit examples.** New builder methods on `Hegel`:

- `.reproduceFailure(blob)` replays the failure a printed blob encodes instead of generating test cases. Call it once per blob to replay several. A run none of whose blobs fails any more throws the new `DidNotReproduceError`.
- `.example(...values)` and `.namedExample({ name: value })` run the test with the given values before generating any: positional values go to the test's draws in order, named ones to the draw printed under that name. A failing example is reported as `Explicit example(...) failed:` and fails the test; one that calls `tc.assume(false)` or `tc.reject()` throws an error saying that explicit test cases must not be rejected; every value must be drawn. Its report indents a stateful step's draws as a generated failure's does. An explicit example has no engine behind it, so `tc.drawSilent`, `tc.repeat` and generators that need the engine (a state machine, say) throw in one, saying they are not supported in explicit examples.

**New `TestCase` methods.**

- `tc.draw(generator, name)` prints the value as `const name = …;` (`name_2`, `name_3`, … when drawn again), and `tc.drawSilent(generator)` draws a value without printing it.
- `tc.reject()` rejects the test case, like `tc.assume(false)`.
- `tc.target(score, label?)` guides generation toward test cases that maximise `score`.
- `tc.event(label)` and `tc.eventValue(label, value)` record what happened in a test case; the `showStatistics` setting reports them at the end of the run.
- `tc.repeat(body)` (and `tc.repeatAsync(body)`) runs `body` an engine-chosen number of times and then ends the test case, each repetition reported under `// Repetition #N`.
- `tc.clone()` returns a handle drawing from an independent choice stream of the same test case, for concurrent tasks, and `tc.setWorker(index)` attributes its output to a worker (`[worker 1 +0.116ms] …`).

Shrinking labels are now derived by the engine from each generator's kind and its components, replacing a fixed table of sixteen numbers that gave every generator of a kind the same label.

**New generators and options.**

- `gs.recursive(base, extend, { maxDepth, maxLeaves })` generates recursively structured data, such as trees or JSON documents. The engine decides where a value branches and keeps it within the limits, so recursive values are distributed the same way as in the other Hegel frontends, and they shrink toward a single leaf.
- `gs.deferred(() => generator)` defines a generator in terms of generators defined after it, including itself, for mutually recursive definitions.
- `gs.permutations(elements)` generates reorderings of a list, shrinking toward the original order.
- `gs.none()` always generates `null`.
- `gs.arrays` takes `uniqueBy`, a key function or an array of them whose keys must be distinct across the array. Keys are compared structurally, as `unique` compares elements, so a key may be an array or an object.
- `gs.floats({ width: 32 })` generates only values a 32-bit float holds exactly.
- `gs.fromRegex` takes `alphabet`, a string of characters or a `gs.characters()` generator, restricting every character of the string, padding included.

```ts
type Json = null | boolean | number | string | Json[];
const json = gs.recursive<Json>(
  gs.oneOf<Json>(gs.none(), gs.booleans(), gs.floats(), gs.text()),
  (children) => gs.arrays(children, { maxSize: 5 }),
);
```

`gs.bigIntegers` with only one bound no longer caps the other side at ±2^127: a missing bound now lies 2^127 beyond the given one (or beyond zero, when that is further), so `gs.bigIntegers({ minValue: 2n ** 200n })` generates values instead of failing.

`gs.arrays(..., { unique: true })` over elements without a schema (mapped or filtered ones, say) now compares elements structurally: primitives as by `Set`, and arrays, plain objects, `Map`s, `Set`s, `Date`s and typed arrays by their contents; other objects, such as class instances, by identity. Previously it compared their `JSON.stringify` output, which threw on bigint elements, treated every `Map` and `Set` as equal to every other, and took time quadratic in the array's length.

A filter on `Pool.valuesConsumed()` now consumes only the value it yields: values its attempts reject go back into the pool. Previously each rejected attempt removed its value from the pool as well.

On Node, text and regex generators follow the Unicode version of Node's own tables (`process.versions.unicode`), so a character category means the same to the generators as to JavaScript's regular expressions.

`gs.text()` and `gs.binary()` without a `maxSize` now take the engine's default maximum, as every Hegel frontend does: 100 characters or bytes, or `minSize + 100` when `minSize` is above 100. Previously they had no upper bound. Pass a `maxSize` for longer values. A filter now tries as many values as the engine says before rejecting the test case, the same number in every frontend.

New `gs.fromLark(grammar, options)` generates strings of a grammar written in [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html)'s syntax, as the other Hegel frontends do, on Node and in browsers. The options are `start` (the rule to generate), `explicit` (generators for terminals whose values the test supplies, such as `%declare`d ones), `alphabet` (the characters terminals may contain, as a string or a `gs.characters()` generator), `maxSize`, `maxDepth` and, on Node, `importPath` (the directory `%import`ed grammar files are read from; Lark's bundled `common`, `python`, `unicode` and `lark` grammars are always available).

```ts
const expressions = gs.fromLark(`
  ?start: sum
  ?sum: product | sum "+" product
  ?product: atom | product "*" atom
  ?atom: NUMBER | "(" sum ")"
  %import common.NUMBER
`);
```
