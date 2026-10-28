import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import { StructuralKeys } from "../src/generators/structuralKey.js";

class Point {
  constructor(readonly x: number) {}
}

function same(a: unknown, b: unknown): boolean {
  const keys = new StructuralKeys();
  return keys.key(a) === keys.key(b);
}

describe("structural keys", () => {
  test.each([
    ["equal strings", "a", "a"],
    ["0 and -0", 0, -0],
    ["NaN and NaN", NaN, NaN],
    ["equal bigints", 10n ** 30n, 10n ** 30n],
    ["true and true", true, true],
    ["undefined and undefined", undefined, undefined],
    ["null and null", null, null],
    ["equal arrays", [1, [2, "x"]], [1, [2, "x"]]],
    ["objects with keys in another order", { a: 1, b: [2] }, { b: [2], a: 1 }],
    ["null-prototype objects", Object.assign(Object.create(null), { a: 1 }), { a: 1 }],
    [
      "maps with entries in another order",
      new Map([
        [1, "a"],
        [2, "b"],
      ]),
      new Map([
        [2, "b"],
        [1, "a"],
      ]),
    ],
    ["sets with members in another order", new Set([1, 2]), new Set([2, 1])],
    ["equal dates", new Date(5), new Date(5)],
    ["equal typed arrays", new Uint8Array([1, 2]), new Uint8Array([1, 2])],
    ["equal bigint arrays", new BigInt64Array([1n]), new BigInt64Array([1n])],
  ])("treats %s as equal", (_name, a, b) => {
    expect(same(a, b)).toBe(true);
  });

  const shared = new Point(1);
  test.each([
    ["distinct strings", "a", "b"],
    ["a number and its string", 1, "1"],
    ["a number and a bigint", 1, 1n],
    ["true and false", true, false],
    ["undefined and null", undefined, null],
    ["a string and a boolean spelt alike", "true", true],
    ["arrays of different lengths", [1], [1, 1]],
    ["objects with different values", { a: 1 }, { a: 2 }],
    ["an object and an array", {}, []],
    ["an empty map and an empty set", new Map(), new Set()],
    ["maps with different values", new Map([[1, "a"]]), new Map([[1, "b"]])],
    ["different dates", new Date(5), new Date(6)],
    ["typed arrays of different types", new Uint8Array([1]), new Int8Array([1])],
    ["equal class instances", new Point(1), new Point(1)],
    ["distinct functions", () => 1, () => 1],
    ["distinct symbols", Symbol("a"), Symbol("a")],
    ["data views", new DataView(new ArrayBuffer(1)), new DataView(new ArrayBuffer(1))],
  ])("treats %s as distinct", (_name, a, b) => {
    expect(same(a, b)).toBe(false);
  });

  test("keys one object the same each time it is seen", () => {
    expect(same(shared, shared)).toBe(true);
    expect(same([shared], [shared])).toBe(true);
  });

  test("keys a cyclic value by identity where it recurs", () => {
    const a: unknown[] = [1];
    a.push(a);
    const keys = new StructuralKeys();
    expect(keys.key(a)).toBe(keys.key(a));
    expect(keys.key(a)).not.toBe(keys.key([1, [1]]));
  });
});

describe("unique arrays compare elements structurally", () => {
  test("bigint elements", () =>
    hegel.test(
      (tc) => {
        const xs = tc.draw(
          gs.arrays(
            gs.bigIntegers({ minValue: 0n, maxValue: 3n }).map((x) => x),
            { unique: true, maxSize: 4 },
          ),
        );
        expect(new Set(xs).size).toBe(xs.length);
      },
      { testCases: 30 },
    ));

  test("distinct maps", () =>
    hegel.test(
      (tc) => {
        const maps = tc.draw(
          gs.arrays(
            gs.integers({ minValue: 0, maxValue: 3 }).map((x) => new Map([[x, x]])),
            { unique: true, minSize: 2, maxSize: 4 },
          ),
        );
        expect(new Set(maps.map((m) => [...m.keys()][0])).size).toBe(maps.length);
      },
      { testCases: 30 },
    ));

  test("uniqueBy keys that are arrays", () =>
    hegel.test(
      (tc) => {
        const xs = tc.draw(
          gs.arrays(gs.integers({ minValue: 0, maxValue: 5 }), {
            uniqueBy: (x) => [x % 3],
            maxSize: 5,
          }),
        );
        expect(new Set(xs.map((x) => x % 3)).size).toBe(xs.length);
      },
      { testCases: 30 },
    ));
});
