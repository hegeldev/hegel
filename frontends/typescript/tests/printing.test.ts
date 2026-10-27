import { describe, expect, it } from "vitest";
import * as gs from "../src/generators/index.js";
import { createRunner, type Settings } from "../src/runnerCore.js";
import { nodeRuntime } from "../src/nodeRuntime.js";
import { browserRuntime } from "../src/browser/runtime.js";
import type { RuntimeServices } from "../src/runtime.js";
import { ExplicitExampleError, type TestCase } from "../src/testCase.js";
import { PrettyPrinter, prettyPrint } from "../src/pretty.js";
import { wasmFixture } from "./wasmFixture.js";

const settings: Settings = {
  database: { kind: "disabled" },
  derandomize: true,
  printBlob: false,
};

const runtimes = [nodeRuntime, browserRuntime(wasmFixture().engine)];

function reportOn(runtime: RuntimeServices, body: (tc: TestCase) => void): string[] {
  const lines: string[] = [];
  const runner = createRunner({ ...runtime, output: (line) => lines.push(line) });
  expect(() => runner.test(body, settings)).toThrow();
  return lines;
}

/** The report of `body` failing, the same on the native and the Wasm engine. */
function report(body: (tc: TestCase) => void): string[] {
  const reports = runtimes.map((runtime) => reportOn(runtime, body));
  expect(reports[0]).toEqual(reports[1]);
  return reports[0];
}

function capturing() {
  const lines: string[] = [];
  const runner = createRunner({ ...nodeRuntime, output: (l) => lines.push(l) });
  return { lines, runner };
}

/** The report line of `generator`'s minimal value, drawn as `x` in a test that always fails. */
function line(generator: gs.PrintableGenerator<unknown>): string {
  const lines = report((tc) => {
    tc.draw(generator, "x");
    throw new Error("fail");
  });
  expect(lines).toHaveLength(1);
  return lines[0];
}

const digit = gs.integers({ minValue: 0, maxValue: 9 }).printWith((d, p) => p.text(`Digit(${d})`));
const unschema = <T>(g: gs.PrintableGenerator<T>) => g.filter(() => true);

describe("a custom-printed element prints inside its container", () => {
  it.each<[string, gs.PrintableGenerator<unknown>, string]>([
    ["an array", gs.arrays(digit, { minSize: 2 }), "[Digit(0), Digit(0)]"],
    [
      "a unique array",
      gs.arrays(digit, { minSize: 2, uniqueBy: (d) => d }),
      "[Digit(0), Digit(1)]",
    ],
    [
      "an array drawn element-wise",
      gs.arrays(unschema(digit), { minSize: 2 }),
      "[Digit(0), Digit(0)]",
    ],
    ["a tuple", gs.tuples(digit, gs.booleans()), "[Digit(0), false]"],
    ["a tuple drawn element-wise", gs.tuples(unschema(digit), gs.booleans()), "[Digit(0), false]"],
    ["a record", gs.record({ d: digit, "a b": digit }), '{ d: Digit(0), "a b": Digit(0) }'],
    ["a record drawn field by field", gs.record({ d: unschema(digit) }), "{ d: Digit(0) }"],
    ["a set", gs.sets(digit, { minSize: 2 }), "new Set([Digit(0), Digit(1)])"],
    [
      "a set drawn element-wise",
      gs.sets(unschema(digit), { minSize: 2 }),
      "new Set([Digit(0), Digit(1)])",
    ],
    ["a map", gs.maps(digit, digit, { minSize: 1 }), "new Map([[Digit(0), Digit(0)]])"],
    [
      "a map drawn entry by entry",
      gs.maps(unschema(digit), unschema(digit), { minSize: 1 }),
      "new Map([[Digit(0), Digit(0)]])",
    ],
    ["a present optional", gs.optional(digit).filter((d) => d !== null), "Digit(0)"],
    [
      "a present optional drawn element-wise",
      gs.optional(unschema(digit)).filter((d) => d !== null),
      "Digit(0)",
    ],
    ["one of several", gs.oneOf(gs.just(10), digit).filter((d) => d < 10), "Digit(0)"],
    [
      "one of several drawn element-wise",
      gs.oneOf(gs.just(10), unschema(digit)).filter((d) => d < 10),
      "Digit(0)",
    ],
    ["a flatMap's target", gs.booleans().flatMap(() => digit), "Digit(0)"],
    ["a filter's source", unschema(digit), "Digit(0)"],
    ["a forward reference", gs.deferred(() => digit), "Digit(0)"],
    ["a filter over a container", unschema(gs.arrays(digit, { minSize: 1 })), "[Digit(0)]"],
    [
      "a recursive leaf",
      gs.recursive<unknown>(digit, (children) => gs.arrays(children, { maxSize: 2 })),
      "Digit(0)",
    ],
    [
      "a recursive branch",
      gs
        .recursive<unknown>(digit, (children) => gs.arrays(children, { minSize: 1, maxSize: 1 }))
        .filter(Array.isArray),
      "[Digit(0)]",
    ],
  ])("in %s", (_name, generator, expected) => {
    expect(line(generator)).toBe(`const x = ${expected};`);
  });

  it("prints an absent optional as null", () => {
    expect(line(gs.optional(digit))).toBe("const x = null;");
    expect(line(gs.optional(unschema(digit)))).toBe("const x = null;");
  });

  it("prints a map over a container by the mapped value", () => {
    expect(line(gs.arrays(digit, { minSize: 1 }).map((ds) => ds.length))).toBe("const x = 1;");
  });
});

describe("built-in generators print their values as the values print", () => {
  const long = { minSize: 6, maxSize: 6 };
  it.each<[string, gs.PrintableGenerator<unknown>]>([
    ["integers", gs.integers()],
    ["big integers", gs.bigIntegers({ minValue: 12n })],
    ["floats", gs.floats({ minValue: -1, maxValue: -0.5 })],
    ["booleans", gs.booleans()],
    ["text", gs.text({ minSize: 2 })],
    ["binary", gs.binary({ minSize: 2 })],
    ["a sample", gs.sampledFrom(["a", 1])],
    ["a constant", gs.just(new Map([[1, { a: [2n] }]]))],
    ["permutations", gs.permutations([3, 1, 2])],
    ["arrays", gs.arrays(gs.integers(), { minSize: 2 })],
    ["unique arrays", gs.arrays(gs.integers(), { minSize: 2, uniqueBy: (n) => n })],
    ["sets", gs.sets(gs.integers(), { minSize: 2 })],
    ["maps", gs.maps(gs.text(), gs.booleans(), { minSize: 2 })],
    ["tuples", gs.tuples(gs.integers(), gs.text(), gs.booleans())],
    ["records", gs.record({ a: gs.integers(), "b c": gs.optional(gs.text()) })],
    ["records drawn field by field", gs.record({ a: unschema(gs.integers()) })],
    ["optional values", gs.optional(gs.integers())],
    ["one of several", gs.oneOf(gs.integers(), gs.text())],
    [
      "recursive values",
      gs
        .recursive<unknown>(gs.integers(), (children) => gs.arrays(children, { maxSize: 3 }))
        .filter(Array.isArray),
    ],
    [
      "values too long for a line",
      gs.arrays(gs.record({ name: gs.text({ minSize: 30, maxSize: 30 }), n: gs.integers() }), long),
    ],
    [
      "long values drawn element-wise",
      gs.arrays(unschema(gs.tuples(gs.text({ minSize: 30, maxSize: 30 }), gs.integers())), long),
    ],
  ])("prints %s", (_name, generator) => {
    for (const runtime of runtimes) {
      let drawn: unknown;
      const lines = reportOn(runtime, (tc) => {
        drawn = tc.draw(generator, "x");
        throw new Error("fail");
      });
      const value = drawn;
      expect(
        reportOn(runtime, (tc) => {
          tc.draw(gs.just(value), "x");
          throw new Error("fail");
        }),
      ).toEqual(lines);
    }
  });
});

describe("value-producing generators print the finished value", () => {
  class Point {
    constructor(
      readonly x: number,
      readonly y: number,
    ) {}
  }
  const zero = gs.integers({ minValue: 0, maxValue: 0 });

  it("prints a map's result, not its input", () => {
    expect(line(zero.map((n) => new Point(n, n)))).toBe("const x = Point { x: 0, y: 0 };");
    expect(line(digit.map((d) => d))).toBe("const x = 0;");
  });

  it("prints a map's input as a call with printAsCall", () => {
    const make = (n: number) => new Point(n, n);
    expect(line(zero.map(make).printAsCall("makePoint"))).toBe("const x = makePoint(0);");
    expect(line(unschema(zero).map(make).printAsCall("makePoint"))).toBe("const x = makePoint(0);");
    expect(line(gs.arrays(digit.map(make).printAsCall("pt"), { minSize: 1 }))).toBe(
      "const x = [pt(Digit(0))];",
    );
    expect(line(gs.arrays(unschema(digit).map(make).printAsCall("pt"), { minSize: 1 }))).toBe(
      "const x = [pt(Digit(0))];",
    );
    expect(() => zero.map(make).printAsCall("")).toThrow(
      "printAsCall requires a non-empty function name",
    );
  });

  it("prints a composite's return value and none of its draws", () => {
    const lines = report((tc) => {
      tc.draw(
        gs.composite((inner) => {
          inner.draw(gs.integers(), "hidden");
          return "done";
        }),
        "x",
      );
      throw new Error("fail");
    });
    expect(lines).toEqual(['const x = "done";']);
  });
});

describe("printing matches Rust's report structure", () => {
  class Point {
    constructor(
      readonly x: unknown,
      readonly y: unknown,
    ) {}
  }
  const points = <T>(coordinates: gs.PrintableGenerator<T>) =>
    gs
      .tuples(coordinates, coordinates)
      .map(([x, y]) => new Point(x, y))
      .printWith((pt, p) =>
        p.group(6, "Point(", ")", () => {
          p.value(pt.x);
          p.separator();
          p.value(pt.y);
        }),
      );

  it("prints one binding per draw, in draw order, with notes between them", () => {
    const lines = report((tc) => {
      tc.draw(gs.arrays(points(gs.integers()), { minSize: 2 }), "ps");
      tc.note("between");
      tc.draw(gs.integers(), "n");
      throw new Error("fail");
    });
    expect(lines).toEqual(["const ps = [Point(0, 0), Point(0, 0)];", "between", "const n = 0;"]);
  });

  it("breaks a custom-printed value with the same primitives", () => {
    const long = gs.text({ minSize: 40, maxSize: 40, alphabet: "a" });
    const zeros = '"' + "a".repeat(40) + '"';
    expect(
      report((tc) => {
        tc.draw(points(long), "p");
        throw new Error("fail");
      }),
    ).toEqual([`const p = Point(${zeros},`, `      ${zeros});`]);
  });
});

describe("speculative printing leaves nothing behind", () => {
  it("prints only the accepted attempt of a filter", () => {
    expect(line(gs.integers({ minValue: 0, maxValue: 9 }).filter((n) => n >= 5))).toBe(
      "const x = 5;",
    );
  });

  it("prints only the kept elements of a unique collection", () => {
    const bit = unschema(gs.integers({ minValue: 0, maxValue: 1 }));
    expect(line(gs.arrays(bit, { minSize: 2, unique: true }))).toBe("const x = [0, 1];");
    expect(line(gs.sets(bit, { minSize: 2 }))).toBe("const x = new Set([0, 1]);");
    expect(line(gs.maps(bit, bit, { minSize: 2 }))).toBe("const x = new Map([[0, 0], [1, 0]]);");
  });

  it("keeps the first entry of a map whose mapped keys collide", () => {
    const key = gs.integers({ minValue: 0, maxValue: 9 }).map(() => "k");
    expect(line(gs.maps(key, gs.integers({ minValue: 0, maxValue: 9 }), { minSize: 2 }))).toBe(
      'const x = new Map([["k", 0]]);',
    );
    expect(line(gs.sets(key, { minSize: 2 }))).toBe('const x = new Set(["k"]);');
  });

  it("prints only the attempt a recursive generator keeps", () => {
    const tree = gs.recursive<unknown>(
      gs.integers({ minValue: 0, maxValue: 0 }),
      (children) => gs.arrays(children, { minSize: 2, maxSize: 2 }),
      { maxLeaves: 3 },
    );
    const lines = report((tc) => {
      const value = tc.draw(tree, "x");
      if (Array.isArray(value)) throw new Error("fail");
    });
    expect(lines).toEqual(["const x = [0, 0];"]);
  });

  it("drops the binding a throwing printer started", () => {
    const thrower = gs.integers().printWith(() => {
      throw new Error("printer failed");
    });
    const { lines, runner } = capturing();
    expect(() =>
      runner.test((tc) => {
        tc.draw(thrower, "x");
        throw new Error("fail");
      }, settings),
    ).toThrow();
    expect(lines.filter((l) => l.startsWith("const x"))).toEqual([]);
  });
});

describe("deferred printing", () => {
  type Recorder = (x: number) => number;

  /** A stub function that prints as the calls made to it. */
  class Recorders extends gs.PrintableGenerator<Recorder> {
    readonly slots: PrettyPrinter[] = [];
    readonly made: Recorder[] = [];

    doDrawAndPrint(_tc: TestCase, p: PrettyPrinter): Recorder {
      p.beginGroup(9, "recorder(");
      const slot = p.deferred();
      p.endGroup(")");
      this.slots.push(slot);
      let first = true;
      const recorder = (x: number) => {
        if (!first) slot.separator();
        first = false;
        slot.value(x);
        return x;
      };
      this.made.push(recorder);
      return recorder;
    }
  }

  it("prints a value described after it was drawn", () => {
    const recorders = new Recorders();
    const lines = report((tc) => {
      const f = tc.draw(recorders, "f");
      f(1);
      f(2);
      f(3);
      throw new Error("fail");
    });
    expect(lines).toEqual(["const f = recorder(1, 2, 3);"]);
  });

  it.each<[string, (g: Recorders) => gs.PrintableGenerator<Recorder[]>]>([
    ["an array", (g) => gs.arrays(g, { minSize: 2, maxSize: 2 })],
    ["a unique array", (g) => gs.arrays(g, { minSize: 2, maxSize: 2, uniqueBy: (f) => f })],
    ["a tuple", (g) => gs.tuples(g, g)],
  ])("prints a value described after it was drawn inside %s", (_, container) => {
    const lines = report((tc) => {
      const [f, g] = tc.draw(container(new Recorders()), "fs");
      f(1);
      g(2);
      g(3);
      throw new Error("fail");
    });
    expect(lines).toEqual(["const fs = [recorder(1), recorder(2, 3)];"]);
  });

  it("lays a deferred value out as if it were printed inline", () => {
    const calls = Array.from({ length: 12 }, (_, i) => 100000 + i);
    const lines = report((tc) => {
      const f = tc.draw(new Recorders(), "f");
      for (const call of calls) f(call);
      tc.draw(
        gs.just(null).printWith((_, p) =>
          p.group(9, "recorder(", ")", () =>
            calls.forEach((call, i) => {
              if (i > 0) p.separator();
              p.value(call);
            }),
          ),
        ),
        "f",
      );
      throw new Error("fail");
    });
    expect(lines.length).toBeGreaterThan(2);
    const half = lines.length / 2;
    expect(lines.slice(0, half).join("\n").replace("const f =", "const f_2 =")).toBe(
      lines.slice(half).join("\n"),
    );
  });

  it("drops what is written to a region a rejected attempt opened", () => {
    const recorders = new Recorders();
    let attempts = 0;
    const lines = report((tc) => {
      attempts = 0;
      recorders.made.length = 0;
      recorders.slots.length = 0;
      const f = tc.draw(
        recorders.filter(() => ++attempts > 1),
        "f",
      );
      expect(recorders.made).toHaveLength(2);
      recorders.made[0](9);
      f(1);
      throw new Error("fail");
    });
    expect(lines).toEqual(["const f = recorder(1);"]);
  });

  it("needs no deferred region to survive a rejected attempt", () => {
    let attempts = 0;
    const once = gs.just(0).printWith((_, p) => {
      if (attempts === 0) p.deferred().text("dropped");
      else p.text("kept");
    });
    const lines = report((tc) => {
      attempts = 0;
      tc.draw(
        once.filter(() => ++attempts > 1),
        "x",
      );
      throw new Error("fail");
    });
    expect(lines).toEqual(["const x = kept;"]);
  });

  it("ignores writes once the test case is over", () => {
    const recorders = new Recorders();
    report((tc) => {
      tc.draw(recorders, "f");
      throw new Error("fail");
    });
    const slot = recorders.slots[recorders.slots.length - 1];
    expect(slot.shouldPrint()).toBe(false);
    expect(recorders.made[recorders.made.length - 1](7)).toBe(7);
    slot.text("late");
    expect(slot.deferred().shouldPrint()).toBe(false);
  });

  it("nests deferred regions", () => {
    const nested = gs.just(null).printWith((_, p) => {
      p.text("o(");
      const outer = p.deferred();
      p.text(")");
      outer.text("[");
      const inner = outer.deferred();
      outer.text("]");
      inner.text("x");
    });
    expect(line(nested)).toBe("const x = o([x]);");
  });

  it("resolves a deferred region an explicit example's value opened", () => {
    class Lazy {
      slot: PrettyPrinter | null = null;
      [prettyPrint](p: PrettyPrinter): void {
        this.slot = p.deferred();
      }
    }
    const lazy = new Lazy();
    const { lines, runner } = capturing();
    expect(() =>
      new runner.Hegel((tc) => {
        tc.draw(gs.just(new Lazy()), "lazy");
        lazy.slot!.text("described later");
        throw new Error("fail");
      })
        .settings(settings)
        .example(lazy)
        .runSync(),
    ).toThrow("fail");
    expect(lines[0]).toBe("const lazy = described later;");
  });
});

describe("generators that cannot print", () => {
  class Evens extends gs.Generator<number> {
    doDraw(tc: TestCase): number {
      return tc.drawSilent(gs.integers({ minValue: 0, maxValue: 100 })) & ~1;
    }
  }
  const message =
    "Evens cannot print the values it draws. Make it printable with .printWith((value, printer) => …) " +
    "or .printAsValue(); declare helpers as returning PrintableGenerator<T> rather than " +
    "Generator<T>; or draw it with tc.drawSilent(…).";
  const asPrintable = (g: gs.Generator<unknown>) => g as gs.PrintableGenerator<unknown>;

  it("cannot be drawn with tc.draw", () => {
    expect(() =>
      new (capturing().runner.Hegel)((tc) => {
        tc.draw(asPrintable(new Evens()));
      })
        .settings(settings)
        .runSync(),
    ).toThrow(message);
  });

  it.each([
    ["an array", (g: gs.Generator<unknown>) => gs.arrays(g, { minSize: 1 })],
    ["a tuple", (g: gs.Generator<unknown>) => gs.tuples(g)],
    ["a record", (g: gs.Generator<unknown>) => gs.record({ a: g })],
    ["a flatMap", (g: gs.Generator<unknown>) => gs.booleans().flatMap(() => g)],
    ["a forward reference", (g: gs.Generator<unknown>) => gs.deferred(() => g)],
  ])("names the generator that cannot print inside %s", (_name, wrap) => {
    const wrapped = asPrintable(wrap(new Evens()));
    const { runner } = capturing();
    expect(() =>
      runner.test((tc) => {
        tc.draw(wrapped);
        throw new Error("fail");
      }, settings),
    ).toThrow(message);
  });

  it("draws silently", () => {
    new (capturing().runner.Hegel)((tc) => {
      expect(tc.drawSilent(new Evens()) % 2).toBe(0);
      expect(tc.drawSilent(gs.arrays(new Evens())).every((n) => n % 2 === 0)).toBe(true);
    })
      .settings({ ...settings, testCases: 5 })
      .runSync();
  });

  it("prints through printAsValue or printWith", () => {
    expect(line(new Evens().printAsValue())).toBe("const x = 0;");
    expect(line(gs.arrays(new Evens().printAsValue(), { minSize: 1 }))).toBe("const x = [0];");
    expect(line(new Evens().filter((n) => n > 0).printAsValue())).toBe("const x = 2;");
    expect(line(new Evens().printWith((n, p) => p.text(`even(${n})`)))).toBe("const x = even(0);");
  });
});

describe("TestCase.drawAndPrint", () => {
  class Pairs extends gs.PrintableGenerator<[number, number]> {
    doDrawAndPrint(tc: TestCase, p: PrettyPrinter): [number, number] {
      return p.group(5, "Pair(", ")", () => {
        const a = tc.drawAndPrint(gs.integers(), p);
        p.separator();
        const b = tc.drawAndPrint(digit, p);
        return [a, b];
      });
    }
  }

  it("prints a hand-written printable generator's components in place", () => {
    expect(line(new Pairs())).toBe("const x = Pair(0, Digit(0));");
    expect(line(gs.arrays(new Pairs(), { minSize: 1 }))).toBe("const x = [Pair(0, Digit(0))];");
  });

  it("draws the same values silently", () => {
    new (capturing().runner.Hegel)((tc) => {
      const [a, b] = tc.drawSilent(new Pairs());
      expect(Number.isInteger(a) && b >= 0 && b <= 9).toBe(true);
    })
      .settings({ ...settings, testCases: 5 })
      .runSync();
  });

  it("is not available at the top level of an explicit example", () => {
    let error: unknown;
    new (capturing().runner.Hegel)((tc) => {
      try {
        tc.drawAndPrint(gs.integers(), PrettyPrinter.noop());
      } catch (e) {
        error ??= e;
      }
    })
      .settings({ ...settings, testCases: 1 })
      .example()
      .runSync();
    expect(error).toBeInstanceOf(ExplicitExampleError);
  });
});
