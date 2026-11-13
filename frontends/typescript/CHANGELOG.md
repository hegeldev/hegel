# Changelog

## 0.5.3 - 2026-11-10

This release updates the bundled libhegel engine to 0.51.1.

## 0.5.2 - 2026-11-09

This patch lets a long state-machine run explore more than one configuration. Between steps, a state machine now occasionally redraws its value distribution and which of its rules are enabled. Previously these were fixed for the whole test case, so a single long-running test case kept one configuration for its whole run.

## 0.5.1 - 2026-11-03

This release updates the bundled libhegel engine to 0.50.0.

## 0.5.0 - 2026-11-03

This release makes the engine the source of every setting a TypeScript test does not set itself, adds the settings that were missing, changes what a failing or aborted run throws and how a failure is reported, makes printing a drawn value the job of the generator that drew it (so `tc.draw` now needs a generator that can print), and adds the test-case features the other Hegel frontends have.

**Settings.** A run now starts from the settings the engine resolves (the default profile, `hegel.toml` and the `HEGEL_*` environment variables) and sets only the fields you give. Previously every run overwrote the test count, verbosity, derandomization and multiple-failure reporting with the frontend's own defaults, so `HEGEL_TEST_CASES` or a `hegel.toml` profile had no effect on them, and the frontend detected CI itself. CI and Antithesis detection is now the engine's: on a CI server the shipped `ci` profile applies (derandomized, database off, `too_slow` suppressed).

- Every field of `Settings` is optional.
- New settings: `profile`, `phases` (with the new `Phase` enum), `printBlob`, `stopOnFirstFailure`, `nondeterminismStrictness` (`NondeterminismStrictness`), `backend` (`Backend`), `showStatistics` and `unboundedChoices`.
- `seed` accepts a `bigint`, so the whole 64-bit range, and `null` clears a seed set by a profile.
- New `hegel.registerProfile(name, settings)` and `hegel.setDefaultProfile(name)`.
- New `hegel.resolveSettings(settings)` resolves a `Settings` now, as a test given it would when it runs, and returns every setting as the engine resolved it (a `ResolvedSettings`, a plain object with every field present). A test given one runs with exactly those values whatever `registerProfile` or `setDefaultProfile` do afterwards: neither is retroactive. An unknown profile or a malformed `hegel.toml` or `HEGEL_*` variable throws from `resolveSettings` instead of from the test.
- `Database.unset` is replaced by `Database.default`, which resets the database to the engine's default location over whatever the profile says. Leave `database` out to keep the profile's choice.
- `suppressHealthCheck: []` now turns every health check back on, where before it changed nothing.
- `testCases` must be a non-negative safe integer, and `seed` an integer in `[0, 2^64)`: anything else throws a `RangeError` before the run starts, the same on Node as in browsers. Previously Node wrapped a negative `testCases` around to 2^64 − 1 and truncated a fractional one.

**Test identity.** A test's database key is now its file, relative to the working directory (in a browser, the path of its URL), plus its name, derived from where `hegel.test` was called, instead of the source text of the test function, which changed with every edit to the test body. The name is the test function's own name; for an anonymous one it is the name Vitest or Jest gives the running test (`"describe > test"`), else the enclosing function's name, else the line it was called from. When several `hegel.test` calls in one runner test or enclosing function borrow its name, they are numbered in the order their lines first run, the first keeping the plain key and each later one carrying its number (`tests/a.test.ts::describe > test (2)`), so each keeps its own saved failures. Their keys do not depend on which other runner tests ran, nor, in a fresh process, on line numbers, but skipping or adding an earlier call in the same runner test renumbers the calls after it, as does moving a call's line in a process that keeps Hegel loaded across reruns; give the test function a name, or set its key, to avoid this. A file URL's percent-encoding is decoded first, so a path with spaces is made relative too. Where there is no `Error.captureStackTrace` to find the call site (Firefox), there is no location, and the key is the test function's name, or none for an anonymous one. So a key, and the seed `derandomize` derives from it, stays the same when the project is checked out somewhere else. Saved failures from earlier versions are not replayed. The same location is passed to the engine, which reports the test's verdict to Antithesis when running inside it; the frontend no longer writes `sdk.jsonl` itself. The `Hegel` builder and the `TestLocation` type are now exported, with `.databaseKey(key)` and `.testLocation(location)` to override either:

```ts
new hegel.Hegel((tc) => {
  tc.draw(gs.integers());
})
  .settings({ testCases: 500 })
  .databaseKey("integers")
  .runSync();
```

**Errors.** A failing test now throws its own error, with its class, `cause` and an assertion library's expected and actual values intact, instead of a new `Error` whose message was the original message followed by the failure's origin. With `reportMultipleFailures`, several distinct failures are thrown together as an `AggregateError` whose message is `Property-based test failed with N distinct failures.`, rather than only the last one. A failure the engine could only confirm as nondeterministic prints its caveat on a `note: ` line, as the other Hegel frontends do, and reports the error seen while searching if replaying it does not fail again. A run the engine aborts throws a `RunError`: `FailedHealthCheckError` (with the failed `healthCheck`), `FlakyError`, `UnsatisfiableError`, or a plain `RunError`.

**Failure reports.** A failing example is now printed the same way on Node and in browsers, laid out by the engine: each drawn value as a line of JavaScript (`const draw_1 = [0, 0];`), numbered in draw order, with the notes between them, and with the `printBlob` setting (on by default) the line that replays the failure. Previously Node printed values with `util.inspect` and browsers with `console.error`'s own formatting. The test body now runs once per test case: the frontend no longer replays the failing example a final time to print it, and `TestCase.isLastRun` is removed. A stateful test reports each step as `Step N: <rule> {`, with the step's draws and notes indented beneath it, and names a failing rule or invariant (`Rule pop failed:`, `Invariant nonNegative failed:`).

```text
const draw_1 = [0, 0];

To reproduce this failure, replay it with the Hegel builder:
    .reproduceFailure("AXicY2VgYGBkZOBiZEBhMAAAAd8AIQ==")
```

**Printing is the generator's job.** Each generator prints the values it draws, as it draws them, through the engine's layout: an array, object, `Map` or `Set` too long for the line breaks one item per line, indented, with a trailing comma. Leaves print their value as JavaScript (`-0`, `1n`, `new Date("…")`). Collections, tuples, records, `optional`, `oneOf`, `filter`, `flatMap`, `recursive` and `deferred` print element by element through the generators they are built from, so an element's own printing shows up inside its container. `map`, `just`, `sampledFrom`, `permutations` and `composite` print the value they produce as JavaScript (a `composite`'s inner draws are not printed), and instances of a class print as `Point { x: 1 }`, or through the class's `[hegel.prettyPrint](printer)` method where it has one.

- New `generator.printWith((value, printer) => …)` prints a generator's values your own way, and `mapped.printAsCall(name)` prints a `map`'s input as a call (`makeKey(3)`) instead of its output.
- New `gs.PrintableGenerator`, a generator that prints as it draws: implement `doDrawAndPrint(tc, printer)`, drawing inner generators with the new `tc.drawAndPrint(generator, printer)` so that they print in place. `gs.Generator`'s `doDraw` is now documented for writing generators that draw silently, and `generator.printAsValue()` makes one printable.
- New `hegel.PrettyPrinter` for writing a printed form: text (a newline in it starts a new line), break points, groups, `seq`, `separator`, `ifBreak`, `hardBreak`, `comment` (printed as `  // text`), `shiftIndent`, `reflow`, `value`, `speculate` (output a rejected draw can retract), `deferred` (a hole written later, for a value the test only learns how to describe after drawing it, such as a stub function that records its calls), `shouldPrint` and `PrettyPrinter.noop()`. `hegel.Document` is a standalone document for trying printing code out outside a test.

This changes what can be drawn with `tc.draw`: it now takes a `gs.PrintableGenerator`, and throws a `TypeError` for a generator that cannot print. Every built-in generator is printable whenever the generators it is built from are, but a generator you wrote by extending `gs.Generator` and implementing only `doDraw` is not: give it a printer with `.printAsValue()` or `.printWith(…)`, extend `gs.PrintableGenerator` instead, or draw it with `tc.drawSilent`. A helper function declared to return `Generator<T>` must be declared to return `PrintableGenerator<T>` for its result to be drawn with `tc.draw`.

```ts
class Evens extends gs.Generator<number> {
  doDraw(tc: hegel.TestCase): number {
    return tc.drawSilent(gs.integers()) & ~1;
  }
}

tc.draw(new Evens()); // TypeError: Evens cannot print the values it draws. …
tc.draw(new Evens().printAsValue()); // const draw_1 = 0;
```

`gs.optional` now chooses between `null` and a value with one boolean choice, as every other Hegel frontend does, instead of picking one of two branches. A failure blob for a test that draws an `optional` is now the same blob the other frontends print for the same draws, and failures saved before this release that involve an `optional` no longer replay as they did.

A `gs.maps` whose keys collide after a `map` now keeps the first of the colliding entries, where it kept the last value under the first key's position.

**Replaying and explicit examples.** New builder methods on `Hegel`:

- `.reproduceFailure(blob)` replays the failure a printed blob encodes instead of generating test cases. Call it once per blob to replay several. A run none of whose blobs fails any more throws the new `DidNotReproduceError`.
- `.example(...values)` and `.namedExample({ name: value })` run the test with the given values before generating any: positional values go to the test's draws in order, named ones to the draw printed under that name. A failing example fails the test, and its draws and notes are printed, laid out by the engine, as a generated failure's are. An example that calls `tc.assume(false)` or `tc.reject()` throws an error beginning `Explicit test cases must not be rejected`. Misusing one is an error in the test, thrown as it is rather than reported as a failing example: giving a value that is never drawn, too few values, or a name no draw uses, and calling `tc.drawSilent` or a generator that needs the engine (a state machine, say). In an explicit example `tc.repeat(body)` runs `body` once, under `// Repetition #1`.

**New `TestCase` methods.**

- `tc.draw(generator, name)` prints the value as `const name_1 = …;` (`name_2`, `name_3`, … when drawn again under the same name), and `tc.drawSilent(generator)` draws a value without printing it. Each name counts on its own, and an unnamed draw counts under `draw`, so the first draw under a name is always `_1` and `.namedExample` gives its value to that printed name (`{ name_1: value }`).
- `tc.reject()` rejects the test case, like `tc.assume(false)`.
- `tc.target(score, label?)` guides generation toward test cases that maximise `score`.
- `tc.event(label)` and `tc.eventValue(label, value)` record what happened in a test case; the `showStatistics` setting reports them at the end of the run.
- `tc.repeat(body)` (and `tc.repeatAsync(body)`) runs `body` an engine-chosen number of times and then ends the test case, each repetition reported under `// Repetition #N`.
- `tc.clone()` returns a handle drawing from an independent choice stream of the same test case, for concurrent tasks (using one after the test body has returned throws, on Node as in browsers), and `tc.setWorker(index)` attributes its output to a worker (`[worker 1 +0.116ms] …`).

Shrinking labels are now derived by the engine from each generator's kind and its components, replacing a fixed table of sixteen numbers that gave every generator of a kind the same label. A generator you write by extending `gs.Generator` or `gs.PrintableGenerator` is labelled by the name of its class (`Evens` above), so two such generators shrink apart; an anonymous class keeps the shared `hegel.generator` label.

**Spans.** Every draw made through the test case, with `tc.draw`, `tc.drawSilent` or `tc.drawAndPrint`, now runs the generator inside a span labelled with the generator's label, and the generators themselves open none of their own: `gs.arrays`, `gs.maps`, `gs.tuples`, `gs.record`, `gs.oneOf`, `gs.optional`, `gs.recursive`, `gs.fromLark` and a `composite` used to open a span of their own around their draws, and `gs.maps` one more around each entry. The engine now sees the same shape of spans for the same draws in every Hegel frontend, which is what its shrinker groups and reorders. A generator of your own that draws its component generators through `tc.drawSilent` or `tc.drawAndPrint` gets a span for each of them with no code of its own. The spans a test case records are part of its failure blob, so a `.reproduceFailure(blob)` printed by an earlier version, or a failure the database saved under one, no longer replays.

**New generators and options.**

- `gs.recursive(base, extend, { maxDepth, maxLeaves })` generates recursively structured data, such as trees or JSON documents. The engine decides where a value branches and keeps it within the limits, so recursive values are distributed the same way as in the other Hegel frontends, and they shrink toward a single leaf.
- `gs.deferred(() => generator)` defines a generator in terms of generators defined after it, including itself, for mutually recursive definitions.
- `gs.permutations(elements)` generates reorderings of a list, shrinking toward the original order.
- `gs.subsequences(elements, { minSize, maxSize })` generates arrays of some of a list's elements, each used at most once, in their original order; `gs.samples(elements, { minSize, maxSize, replacement })` generates arrays of elements chosen from a list, with replacement by default, or without it (`replacement: false`) for a reordered subset. Both shrink toward fewer elements, taken from earlier in the list.
- `gs.weightedBooleans(p)` generates `true` with probability `p` and `false` otherwise, shrinking toward `false`.
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

With `verbosity: Verbosity.Verbose` or `Verbosity.Debug`, a test case that stops short now says why, as the other Hegel frontends do: `Test case stopped: failed assumption` or `Test case stopped: out of data`.

This release also replaces the `excludeMin` and `excludeMax` flags of `gs.floats()` with `minValueExclusive` and `maxValueExclusive`, which take the bound directly, as the other Hegel frontends do:

```ts
// before
gs.floats({ minValue: 0, maxValue: 1, excludeMin: true, excludeMax: true });

// after
gs.floats({ minValueExclusive: 0, maxValueExclusive: 1 });
```

Giving both `minValue` and `minValueExclusive` (or both `maxValue` and `maxValueExclusive`) is an error. `gs.floats()` also gains `allowSubnormal`: subnormal ("denormalised") values are drawn whenever the bounds admit any, as before, and `allowSubnormal: false` keeps them out, for code that may run with flush-to-zero floating point. Asking for subnormals with bounds that exclude them all, or excluding them from a range that holds nothing else, is an error.

`gs.binary()` now returns a plain `Uint8Array` on Node too, where it returned a `Buffer`, so a drawn byte string prints as `Uint8Array.from([…])` on every host, as the README said.

- `tc.repeat` and `tc.repeatAsync` now have the engine decide how many repetitions run and which one failed, through libhegel's new `hegel_repeat_t` functions, rather than drawing those choices in TypeScript. The choices are the same, so an existing reproduce blob still replays.

**Stateful testing.** A rule can now carry a weight, as in the other Hegel frontends: write it as `{ apply: (tc, state) => …, weight: 3 }` instead of a bare function, and the engine offers it that many times as often as a rule of weight 1 (every rule's weight is 1 by default). A weight must be finite and positive; any other value throws before the run starts. Concurrent state machines, which the Rust, Java, C++ and OCaml frontends offer, are not supported in JavaScript, because the host is single-threaded; the 0.4.10 changelog entry and `BROWSER.md` said they were waiting on a newer libhegel, which was never the reason.

New `hegel.engineVersion()` returns the version of the libhegel engine the package ships, and `hegel.EngineError`, the error thrown when the engine or its bindings fault, is now exported so it can be caught by type.

A `hegel.Document` that is never finished now releases its engine document when it is garbage collected, instead of keeping it for the life of the process.

## 0.4.12 - 2026-10-28

This release updates the bundled libhegel engine to 0.49.0.

## 0.4.11 - 2026-10-22

This patch moves Hegel for TypeScript into the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo, next to the engine it drives. The package is used exactly as before, but it is now built and tested against the in-tree libhegel, so this release also updates the bundled engine from 0.45.4 to 0.48.0. The engine changes a TypeScript user can see:

- Strings passed to the engine may now contain NUL characters: a database key, the reason given to a rejected draw, or a `gs.fromRegex()` pattern no longer throws for `"\0"`.
- `gs.fromRegex()` now fails when it is built, with an error naming the pattern, if no string can match it (for example `(?!)`), instead of rejecting every draw until the run fails a health check. Patterns may use `\z` as a spelling of `\Z`.
- Integer shrinking no longer stalls short of the minimum on some sparse sets of failing values, generation over small finite spaces no longer over-produces the simplest value, and a failure found while shrinking another is now shrunk before it is reported. Seeded runs may generate different values from this version on.
- Every distinct failure a run finds is persisted to its database, and a run that found several reports only the smallest unless `reportMultipleFailures` is on.
- A float draw whose bounds leave only an infinity while infinities are disallowed is rejected as an invalid argument instead of failing as an internal error.

## 0.4.10 - 2026-10-14

This release adds stateful (model-based) testing, in the new `hegel.stateful` module, on both the native and the browser entry.

Describe a state machine as a plain object mapping rule names to rule functions and invariant names to invariant functions, then run it inside a test with `hegel.stateful.run(tc, machine, initialState)` (or `hegel.stateful.runAsync` for asynchronous rules). The engine chooses the sequence of rules, bounds it by the configurable `stepCount` (50 by default), samples the invariants between steps (or checks an invariant marked `alwaysCheck` after every step), and shrinks failing sequences like any other input; the failure report lists each step by rule name. `hegel.stateful.Pool` lets rules draw values that earlier rules produced, with `valuesReusable()` and `valuesConsumed()` generators.

Rule weights are not included in this release. Concurrent machines, which the Rust and OCaml clients offer, are not supported in JavaScript: the host is single-threaded, so there is nothing to run a round's rules on at the same time.

## 0.4.9 - 2026-10-14

This patch bumps our pinned libhegel ([hegel-rust](https://github.com/hegeldev/hegel-rust)) from [0.42.4](https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v0.42.4) to [0.45.4](https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v0.45.4).

## 0.4.8 - 2026-10-14

Fix the getting-started examples, which passed the result of `hegel.test(...)` to `test` instead of a function. They now use `test("name", () => hegel.test(...))`.

## 0.4.7 - 2026-09-24

This release adds browser support through the existing `@hegeldev/hegel` imports. Bundlers select a new browser entry through the package's `browser` export condition; it loads libhegel's published WebAssembly build during module evaluation (top-level await, so it needs an ES2022 target) and exposes the same `test()` and `testAsync()`. Node, Bun and Deno continue to use the native library. Generation and shrinking run on the browser's main thread, and browser test settings reject filesystem database paths (`Database.fromPath`). See `BROWSER.md` for bundler configuration.

It also adds `uuids()`, a generator for UUID strings (optionally of a given version).

## 0.4.6 - 2026-09-17

This patch bumps our pinned libhegel ([hegel-rust](https://github.com/hegeldev/hegel-rust)) from [0.32.5](https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v0.32.5) to [0.42.4](https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v0.42.4), and follows hegel-rust's new release tags: libhegel binaries now hang off `libhegel-v<version>` tags, so the fetch script, the bump workflow and the FFI alignment guide all use those.

Aligning to the new engine changes two things users can see:

- `times()` and `datetimes()` now draw with nanosecond precision. Values on a whole microsecond keep their previous `HH:MM:SS.ffffff` form; other values print all nine fractional digits.
- libhegel now resolves a default settings profile (from `HEGEL_DEFAULT_PROFILE` or a `hegel.toml`) when a run starts. If that fails — an unknown profile name, a malformed `hegel.toml` — `hegel.test` now throws the engine's diagnostic instead of crashing on a null handle.

It also fixes the mapping of the `Quiet` and `Normal` verbosity levels, which libhegel 0.42 renumbered.

## 0.4.5 - 2026-08-13

This patch bumps our pinned libhegel ([hegel-rust](hegeldev/hegel-rust)) from [0.32.4](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.4) to [0.32.5](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.5).

## 0.4.4 - 2026-08-13

This patch bumps our pinned libhegel ([hegel-rust](hegeldev/hegel-rust)) from [0.23.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.23.0) to [0.32.4](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.4).

## 0.4.3 - 2026-07-24

Hegel now officially supports Bun 1.2.5+ and Deno 2+. (Deno requires `--allow-ffi --allow-read --allow-env`).

## 0.4.2 - 2026-07-23

The native libhegel shared libraries are now distributed as per-platform npm
packages (`@hegeldev/hegel-linux-x64`, `@hegeldev/hegel-darwin-arm64`, ...)
instead of all being bundled inside the main package. Your package manager
automatically installs the single package matching your platform. No action is
required.

## 0.4.1 - 2026-07-21

Remove the `Property test failed:` prefix from the error thrown when a property fails.

## 0.4.0 - 2026-07-09

This release changes the default value of `fullmatch` in `fromRegex` from `false` to `true`.

## 0.3.1 - 2026-06-29

This patch adds the `reportMultipleFailures` setting. When enabled, a run keeps
generating after the first failure to surface additional *distinct* failures
(each with a different origin); when disabled, the run stops after the first
failing example. It defaults to `false`.

```ts
hegel.test(fn, { reportMultipleFailures: true });
```

## 0.3.0 - 2026-06-26

hegel-typescript now uses [libhegel](https://github.com/hegeldev/hegel-rust) — the native
Rust engine — directly via FFI, instead of spawning the `hegel-core` Python
server and talking to it over a socket protocol.

The public API is unchanged. Two user-visible requirement changes:

- Hegel no longer needs Python or `uv`.
- Hegel now requires Node 20.11+ (the native FFI layer uses a loader that
  depends on a recent Node).

## 0.2.3 - 2026-05-26

This patch bumps our pinned hegel-core from [0.6.0](https://github.com/hegeldev/hegel-core/releases/tag/v0.6.0) to [0.9.1](https://github.com/hegeldev/hegel-core/releases/tag/v0.9.1).

## 0.2.2 - 2026-05-14

We now automatically derive a database key based on the source code of the function. This allows Hegel to automatically replay previous failures. See https://github.com/hegeldev/hegel-typescript/issues/36.

## 0.2.1 - 2026-05-09

This release makes `generators` reachable as a namespace from `@hegeldev/hegel`:

```typescript
// A
import * as hegel from "@hegeldev/hegel";
hegel.generators.integers()

// B, still works as before:
import * as gs from "@hegeldev/hegel/generators";
gs.integers()
```

We still recommend option B.

This release also removes a number of private APIs from the public exports of `@hegeldev/hegel`.

## 0.2.0 - 2026-05-04

This release changes `hegel.test` to execute immediately when called, instead of returning a callable which must be called to run the property-based test.

For example, here's how to migrate `vitest` tests to this release:

```typescript
// before
test("my test", hegel.test(...))

// after
test("my test", () => hegel.test(...))
```

This release also adds `hegel.testAsync`, for use with async tests:

```typescript
test("my async test", () =>
  hegel.testAsync(async (tc) => {
    const id = tc.draw(gs.integers({ minValue: 1 }));
    await fetchUser(id);
  }),
);
```

## 0.1.5 - 2026-04-30

Internal refactor.

## 0.1.4 - 2026-04-29

Internal refactor of `oneOf`.

## 0.1.3 - 2026-04-28

Bump our pinned `hegel-core` version from `0.4.0` to [`0.4.14`](https://github.com/hegeldev/hegel-core/releases/tag/v0.4.14).

## 0.1.2 - 2026-04-25

Loosen the type of `sampledFrom` and `text({categories: ...})` to accept `readonly` arrays.

## 0.1.1 - 2026-04-22

Internal refactor in preparation for release.

## 0.1.0 - 2026-04-21

Initial release!
