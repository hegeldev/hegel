> [!IMPORTANT]
> We're excited you're checking out Hegel! Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel/issues/new).
>
> As part of our beta, we may make breaking changes if it makes Hegel a better property-based testing library. If that instability bothers you, please check back in a few months for a stable release!
>
> See https://hegel.dev/compatibility for more details.

# Hegel for TypeScript

- [Documentation](https://hegel.dev/typescript)
- [Website](https://hegel.dev)

Hegel is a property-based testing library for TypeScript, built on libhegel, the [Hegel](https://hegel.dev/) engine. It runs in-process: as a native library on Node, Bun and Deno, and as the same engine compiled to WebAssembly in browsers.

## Installation

To install: `npm install --save-dev @hegeldev/hegel`.

Hegel requires Node 20.11+, Bun 1.2.5+, or Deno 2+. (Note that Deno requires `--allow-ffi --allow-read --allow-env`).

Linux amd64/arm64, macOS arm64, and Windows amd64/arm64 are supported.

Browser support uses the same package imports and initializes the Wasm engine during ESM module loading. It requires top-level await and secure-context Web Crypto. Tests and shrinking run on the main thread; browser persistence is unsupported. See [the browser guide](./BROWSER.md) for bundler configuration and how the Wasm module is packaged.

## Quickstart

Here's a quick example of how to write a Hegel test:

```typescript
import { test } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

function mySort(ls: number[]): number[] {
  const result = [...ls].sort((a, b) => a - b);
  return [...new Set(result)];
}

test("my_sort matches builtin", () =>
  hegel.test((tc) => {
    const vec1 = tc.draw(gs.arrays(gs.integers()));
    const vec2 = mySort(vec1);
    const sorted = [...vec1].sort((a, b) => a - b);
    if (JSON.stringify(sorted) !== JSON.stringify(vec2)) {
      throw new Error(`sort mismatch: ${JSON.stringify(sorted)} != ${JSON.stringify(vec2)}`);
    }
  }));
```

This test will fail when run with `vitest`! Hegel will produce a minimal failing test case for us, printing each value the test drew and how to replay the failure, and then throw the test's own error:

```
const draw_1 = [0, 0];

To reproduce this failure, replay it with the Hegel builder:
    .reproduceFailure("AXicY2VgYGBkZOBiZEBhMAAAAd8AIQ==")
Error: sort mismatch: [0,0] != [0]
```

Hegel reports the minimal example showing that our sort is incorrectly dropping duplicates. If we remove the `new Set(...)` deduplication from `mySort()`, this test will then pass (because it's just comparing the standard sort against itself).

## Async tests

For async tests, use `hegel.testAsync`:

```typescript
import { test } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

test("fetch returns a value matching its input", () =>
  hegel.testAsync(async (tc) => {
    const id = tc.draw(gs.integers({ minValue: 1, maxValue: 1000 }));
    const result = await fetchUser(id);
    if (result.id !== id) {
      throw new Error(`Expected id=${id}, got ${result.id}`);
    }
  }));
```

## Stateful tests

Some bugs only appear after a particular sequence of operations. For those, describe the operations as the rules of a state machine and let Hegel search over sequences of them with `hegel.stateful.run`:

```typescript
import { test } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

const stackMachine: hegel.stateful.StateMachine<{ items: number[] }> = {
  rules: {
    push: (tc, stack) => {
      stack.items.push(tc.draw(gs.integers()));
    },
    pop: (tc, stack) => {
      tc.assume(stack.items.length > 0);
      stack.items.pop();
    },
  },
  invariants: {
    nonNegativeLength: (_tc, stack) => {
      if (stack.items.length < 0) throw new Error("negative length");
    },
  },
};

test("stack", () =>
  hegel.test((tc) => {
    hegel.stateful.run(tc, stackMachine, { items: [] });
  }));
```

Hegel picks the rules to run at each step, shrinks a failing sequence down to a minimal one, and reports it step by step (`Step 1: push {`, with the step's draws indented beneath it, then `}`). See the `stateful` module in the documentation for invariants, step counts, asynchronous rules and `Pool`, which lets rules act on values that earlier rules produced.

## How failing examples print

Printing is the generator's job: each generator prints the values it draws, as it draws them, laid out by the engine to fit the line. Every built-in generator prints:

- Leaves (`integers`, `text`, `binary`, …) print their value as JavaScript: `-0`, `12n`, `"a"`, `Uint8Array.from([0])`.
- Structural generators (`arrays`, `sets`, `maps`, `tuples`, `record`, `optional`, `oneOf`, `filter`, `flatMap`, `recursive`, `deferred`) print element by element through the generators they are built from, so an element's own printing shows up inside its container.
- Value-producing generators (`map`, `just`, `sampledFrom`, `composite`) print the value they produce, as JavaScript. A `composite`'s inner draws are not printed.

Arrays, objects, `Map`s and `Set`s break one item per line when they are too long. To print a generator's values some other way, give it a printer with `printWith`:

```typescript
const digits = gs.integers({ minValue: 0, maxValue: 9 }).printWith((d, p) => p.text(`Digit(${d})`));
tc.draw(gs.arrays(digits), "ds"); // const ds = [Digit(3), Digit(0)];
```

`printWith` is also how to print values with no useful JavaScript representation, such as functions, or to hide a secret. On a `map`, `printAsCall(name)` prints the input draw as an argument instead of the output: `gs.integers().map(makeKey).printAsCall("makeKey")` prints `makeKey(3)`.

`tc.draw` takes a `PrintableGenerator`. A generator you write by extending `gs.Generator` and implementing only `doDraw` draws silently, with `tc.drawSilent`; make it printable with `.printAsValue()` (print each value as JavaScript) or `.printWith(...)`, or extend `gs.PrintableGenerator` and print as you draw:

```typescript
class Pairs extends gs.PrintableGenerator<[number, number]> {
  doDrawAndPrint(tc: hegel.TestCase, p: hegel.PrettyPrinter): [number, number] {
    return p.group(5, "Pair(", ")", () => {
      const a = tc.drawAndPrint(gs.integers(), p);
      p.separator();
      const b = tc.drawAndPrint(gs.integers(), p);
      return [a, b];
    });
  }
}
```

`tc.drawAndPrint` draws an inner generator printing in place. Leave `doDraw` to its default, which runs `doDrawAndPrint` with a printer that discards everything, so that both make the same draws. Declare helper functions that return generators as returning `PrintableGenerator<T>`, not `Generator<T>`, so that their results can be drawn with `tc.draw`.

A group prints on one line when it fits, each `breakable` as its separator, and otherwise breaks at every `breakable`, indenting the next line by the group's indentation. The printer also has `seq` (a delimited list laid out like the built-in containers), `ifBreak`, `hardBreak`, `comment`, `shiftIndent`, `reflow`, `value` (a value as JavaScript), `speculate` (output a rejected draw can retract) and `deferred`, a hole at the current position that can be written later: what a value prints when the test only learns how to describe it after drawing it, such as a stub function recording its calls. `hegel.Document` is a standalone document for trying printing code out outside a test.

When a value is printed as JavaScript (by `value`, `map`, `just`, `composite` or `printAsValue`), instances of your own classes print as their class name and properties. To print one differently, give the class a `hegel.prettyPrint` method:

```typescript
class Interval {
  constructor(
    readonly lo: number,
    readonly hi: number,
  ) {}

  [hegel.prettyPrint](p: hegel.PrettyPrinter): void {
    p.group(9, "Interval(", ")", () => {
      p.value(this.lo);
      p.separator();
      p.value(this.hi);
    });
  }
}
```
