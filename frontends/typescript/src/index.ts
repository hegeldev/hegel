/**
 * Hegel is a property-based testing library for TypeScript, built on libhegel,
 * the [Hegel](https://hegel.dev/) engine: a native library on Node, Bun and
 * Deno, and the same engine compiled to WebAssembly in browsers.
 *
 * # Getting started
 *
 * This guide walks you through the basics of installing Hegel and writing your
 * first tests.
 *
 * ## Install Hegel
 *
 * Add `@hegeldev/hegel` to your project as a dev dependency:
 *
 * ```bash
 * npm install --save-dev @hegeldev/hegel
 * ```
 *
 * Hegel supports Node 20.11+, Bun 1.2.5+, Deno 2+, and modern ESM browsers.
 * Browser module loading initializes Wasm with top-level await.
 *
 * ## Write your first test
 *
 * You're now ready to write your first test. We'll use
 * [Vitest](https://vitest.dev/) as the test runner for the purposes of this
 * guide. Create a new test file:
 *
 * ```ts
 * import { test } from "vitest";
 * import * as hegel from "@hegeldev/hegel";
 * import * as gs from "@hegeldev/hegel/generators";
 *
 * test("integer self equality", () =>
 *   hegel.test((tc) => {
 *     const n = tc.draw(gs.integers());
 *     if (n !== n) {
 *       throw new Error("integer was not equal to itself");
 *     }
 *   }));
 * ```
 *
 * Now run the test using `npx vitest run`. You should see that this test
 * passes.
 *
 * Let's look at what's happening in more detail. {@link test} runs your test
 * many times (100, by default). The test function receives a {@link TestCase},
 * which provides a {@link TestCase.draw | draw} method for drawing different
 * values. This test draws a random integer and checks that it should be equal
 * to itself.
 *
 * Next, try a test that fails:
 *
 * ```ts
 * test("integers always below 50", () =>
 *   hegel.test((tc) => {
 *     const n = tc.draw(gs.integers());
 *     if (n >= 50) {
 *       throw new Error(`n=${n} is too large`);
 *     }
 *   }));
 * ```
 *
 * This test asserts that any integer is less than 50, which is obviously
 * incorrect. Hegel will find a test case that makes this assertion fail, and
 * then shrink it to find the smallest counterexample — in this case, `n = 50`.
 *
 * To fix this test, you can constrain the integers you generate with the
 * `minValue` and `maxValue` options:
 *
 * ```ts
 * test("bounded integers always below 50", () =>
 *   hegel.test((tc) => {
 *     const n = tc.draw(gs.integers({ minValue: 0, maxValue: 49 }));
 *     if (n >= 50) {
 *       throw new Error(`n=${n} is too large`);
 *     }
 *   }));
 * ```
 *
 * Run the test again. It should now pass.
 *
 * ## Use generators
 *
 * Hegel provides a rich library of generators that you can use out of the box.
 * There are primitive generators, such as `integers`, `floats`, and `text`,
 * and combinators that allow you to make generators out of other generators,
 * such as `arrays` and `maps`.
 *
 * For example, you can use `arrays` to generate an array of integers:
 *
 * ```ts
 * test("append increases length", () =>
 *   hegel.test((tc) => {
 *     const xs = tc.draw(gs.arrays(gs.integers()));
 *     const initialLength = xs.length;
 *     xs.push(tc.draw(gs.integers()));
 *     if (xs.length <= initialLength) {
 *       throw new Error("length did not increase");
 *     }
 *   }));
 * ```
 *
 * This test checks that appending an element to a random array of integers
 * should always increase its length.
 *
 * You can also build composite values out of multiple generators. The simplest
 * way is to draw fields directly inside the test body:
 *
 * ```ts
 * interface Person {
 *   age: number;
 *   name: string;
 * }
 *
 * test("person", () =>
 *   hegel.test((tc) => {
 *     const person: Person = {
 *       age: tc.draw(gs.integers({ minValue: 0, maxValue: 120 })),
 *       name: tc.draw(gs.text({ minSize: 1, maxSize: 50 })),
 *     };
 *     // use person in your test
 *     void person;
 *   }));
 * ```
 *
 * For composite values you want to reuse across tests, build a generator with
 * `composite` (imperative — call `tc.draw()` on inner generators inside a
 * builder function) or `record` (declarative — pass a schema mapping field
 * names to generators). Both produce a generator that supports `.map()`,
 * `.filter()`, and `.flatMap()` like any other generator.
 *
 * ```ts
 * const personGen = gs.record({
 *   age: gs.integers({ minValue: 0, maxValue: 120 }),
 *   name: gs.text({ minSize: 1, maxSize: 50 }),
 * });
 *
 * test("person via record", () =>
 *   hegel.test((tc) => {
 *     const person = tc.draw(personGen);
 *     void person;
 *   }));
 * ```
 *
 * Note that you can feed the results of one `draw` into subsequent calls — this
 * is what `composite` is for. For example, say that you extend the `Person`
 * interface to include a `drivingLicense` boolean field, where the field
 * depends on `age`:
 *
 * ```ts
 * interface Person {
 *   age: number;
 *   name: string;
 *   drivingLicense: boolean;
 * }
 *
 * const personGen = gs.composite<Person>((tc) => {
 *   const age = tc.draw(gs.integers({ minValue: 0, maxValue: 120 }));
 *   const name = tc.draw(gs.text({ minSize: 1, maxSize: 50 }));
 *   const drivingLicense = age >= 18 ? tc.draw(gs.booleans()) : false;
 *   return { age, name, drivingLicense };
 * });
 * ```
 *
 * For recursive data, such as trees or JSON documents, use
 * `gs.recursive`: give it the generator of leaves, and a function that builds
 * one level of structure from a generator of sub-values. The engine decides
 * where values branch and keeps them within `maxDepth` and `maxLeaves`; use
 * `gs.deferred` for definitions that refer to each other.
 *
 * ```ts
 * type Json = null | boolean | number | string | Json[];
 * const json = gs.recursive<Json>(
 *   gs.oneOf<Json>(gs.none(), gs.booleans(), gs.floats(), gs.text()),
 *   (children) => gs.arrays(children, { maxSize: 5 }),
 * );
 * ```
 *
 * ## Debug your failing test cases
 *
 * Use the {@link TestCase.note | note} method to attach debug information:
 *
 * ```ts
 * test("addition is commutative", () =>
 *   hegel.test((tc) => {
 *     const x = tc.draw(gs.integers());
 *     const y = tc.draw(gs.integers());
 *     tc.note(`x + y = ${x + y}, y + x = ${y + x}`);
 *     if (x + y !== y + x) {
 *       throw new Error("addition is not commutative");
 *     }
 *   }));
 * ```
 *
 * When a test fails, Hegel prints the smallest failing example it found:
 * each drawn value as a line of JavaScript, in the order drawn, with the
 * notes between them, then the line to replay it with. For a test like the
 * one above, that looks like:
 *
 * ```text
 * const draw_1 = 0;
 * const draw_2 = -1;
 * x + y = -1, y + x = -1
 *
 * To reproduce this failure, replay it with the Hegel builder:
 *     .reproduceFailure("AXicY2BgYGQAAAAJAAI")
 * ```
 *
 * Pass a name to `draw` to print a value under it (`tc.draw(gs.integers(),
 * "x")` prints `const x_1 = ...;`, a second draw under the name `const x_2 =
 * ...;`), and use {@link TestCase.drawSilent |
 * drawSilent} for a value not worth printing. Notes and drawn values are
 * only printed for the failing example (and for every test case of a
 * verbose run), so they cost nothing while Hegel searches.
 *
 * Each value is printed by the generator that drew it, as it draws it, so
 * `draw` takes a {@link generators.PrintableGenerator}: every built-in
 * generator is one whenever the generators it is built from are. Give a
 * generator a printed form of your own with
 * {@link generators.Generator.printWith | printWith}, and see
 * {@link PrettyPrinter} for the printer it writes to.
 *
 * To replay a failure, pass the blob to {@link Hegel.reproduceFailure |
 * reproduceFailure}; to always run some inputs first, give them as
 * {@link Hegel.example | explicit examples}:
 *
 * ```ts
 * new hegel.Hegel((tc) => {
 *   const x = tc.draw(gs.integers());
 *   const y = tc.draw(gs.integers());
 *   // ...
 * })
 *   .example(0, -1)
 *   .reproduceFailure("AXicY2BgYGQAAAAJAAI") // the blob printed by the failure
 *   .runSync();
 * ```
 *
 * ## Guide the search
 *
 * {@link TestCase.target | target} steers generation toward test cases that
 * maximise a score, {@link TestCase.event | event} and
 * {@link TestCase.eventValue | eventValue} record what happened (reported with
 * the `showStatistics` setting), and {@link TestCase.repeat | repeat} runs a
 * workload an engine-chosen number of times, shrinking the number of
 * repetitions along with what each drew. For concurrent code, give each task
 * its own {@link TestCase.clone | clone} of the test case, so that what one
 * draws does not depend on how the tasks interleave.
 *
 * ## Change the number of test cases
 *
 * By default Hegel runs 100 test cases. To override this, pass a
 * {@link Settings} override as the second argument to {@link test}:
 *
 * ```ts
 * test("integers many", () =>
 *   hegel.test(
 *     (tc) => {
 *       const n = tc.draw(gs.integers());
 *       if (n !== n) {
 *         throw new Error("integer was not equal to itself");
 *       }
 *     },
 *     { testCases: 500 },
 *   ));
 * ```
 *
 * Settings you leave out keep the values the engine resolves from the
 * settings profile: `development` locally, `ci` on a CI server (derandomized,
 * with the database off) and `workload` inside Antithesis, with `hegel.toml`
 * and the `HEGEL_*` environment variables applied over it. Register your own
 * profiles with {@link registerProfile} and select one with the `profile`
 * setting or {@link setDefaultProfile}:
 *
 * ```ts
 * hegel.registerProfile("nightly", { testCases: 10_000 });
 * hegel.setDefaultProfile(process.env.NIGHTLY ? "nightly" : null);
 * ```
 *
 * A test resolves the profile when it runs. To resolve it once, ahead of the
 * tests, {@link resolveSettings} returns every setting as the engine resolves
 * it now, as a plain object to pass to {@link test} or spread into another:
 *
 * ```ts
 * const nightly = hegel.resolveSettings({ profile: "nightly" });
 * test("deep", () => hegel.test(property, { ...nightly, seed: 42n }));
 * ```
 *
 * Neither {@link registerProfile} nor {@link setDefaultProfile} changes
 * settings resolved before it was called.
 *
 * {@link Hegel} configures a test with builder methods instead, including the
 * key its failures are saved under (by default its file and name) and its
 * location for Antithesis (by default where it was defined).
 *
 * ## Understand what a run throws
 *
 * A failing test throws its own error, from the smallest failing example, so
 * assertion libraries show their usual diff; with
 * `reportMultipleFailures`, several distinct failures are thrown together as
 * an `AggregateError`. A run the engine aborts throws a {@link RunError}: a
 * {@link FailedHealthCheckError}, {@link FlakyError} or
 * {@link UnsatisfiableError}, or a plain `RunError` for anything else. A run
 * replaying failure blobs none of which fails any more throws a
 * {@link DidNotReproduceError}.
 *
 * ## Test stateful systems
 *
 * Some bugs only show up after a particular *sequence* of operations. For
 * those, describe the operations as the rules of a state machine and let
 * Hegel search over sequences of them — see the {@link stateful} module:
 *
 * ```ts
 * const counterMachine: hegel.stateful.StateMachine<{ value: number }> = {
 *   rules: {
 *     increment: (_tc, counter) => {
 *       counter.value += 1;
 *     },
 *     reset: (_tc, counter) => {
 *       counter.value = 0;
 *     },
 *   },
 *   invariants: {
 *     nonNegative: (_tc, counter) => {
 *       if (counter.value < 0) throw new Error("counter went negative");
 *     },
 *   },
 * };
 *
 * test(
 *   "counter",
 *   hegel.test((tc) => {
 *     hegel.stateful.run(tc, counterMachine, { value: 0 });
 *   }),
 * );
 * ```
 *
 * ## Learning more
 *
 * - Browse the `@hegeldev/hegel/generators` module for the full list of
 *   available generators.
 * - See {@link stateful} for stateful (model-based) testing.
 * - See {@link Settings} for more configuration settings to customize how
 *   your test runs.
 *
 * @packageDocumentation
 */

export * as generators from "./generators/index.js";
export * as stateful from "./stateful.js";
export { TestCase } from "./testCase.js";
export {
  Verbosity,
  HealthCheck,
  Phase,
  Backend,
  NondeterminismStrictness,
  Database,
} from "./settings.js";
export type { ResolvedSettings, Settings, TestLocation } from "./settings.js";
export { EngineError } from "./engine.js";
export {
  RunError,
  FailedHealthCheckError,
  FlakyError,
  UnsatisfiableError,
  DidNotReproduceError,
} from "./errors.js";
export { Hegel } from "./runnerCore.js";
export {
  Document,
  PrettyPrinter,
  Speculation,
  prettyPrint,
  type DocumentOptions,
  type PrettyPrintable,
} from "./pretty.js";

import {
  test as nodeTest,
  testAsync as nodeTestAsync,
  resolveSettings as nodeResolveSettings,
  engineVersion as nodeEngineVersion,
  registerProfile as nodeRegisterProfile,
  setDefaultProfile as nodeSetDefaultProfile,
} from "./runner.js";
import type { TestCase } from "./testCase.js";
import type { ResolvedSettings, Settings } from "./settings.js";

// Explicit public types keep the shared declaration graph independent of Node services.
export const test: (testFn: (tc: TestCase) => void, settings?: Settings) => void = nodeTest;
export const testAsync: (
  testFn: (tc: TestCase) => void | Promise<void>,
  settings?: Settings,
) => Promise<void> = nodeTestAsync;
export const resolveSettings: (settings?: Settings) => ResolvedSettings = nodeResolveSettings;
export const engineVersion: () => string = nodeEngineVersion;
export const registerProfile: (name: string, settings?: Settings) => void = nodeRegisterProfile;
export const setDefaultProfile: (name: string | null) => void = nodeSetDefaultProfile;
