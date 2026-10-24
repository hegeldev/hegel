/**
 * Tests for the collection protocol path (non-basic element generators).
 *
 * When a generator doesn't have asBasic() (e.g. after .filter()), the
 * collection generators (arrays, sets, maps) must use the server's
 * collection protocol (new_collection / collection_more / collection_reject)
 * instead of sending a schema.
 *
 * Ported from hegel-rust tests/test_collections.rs and the old
 * tests/generators/collections.test.ts.
 */

import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

// Collection protocol tests use fewer test cases because each test case
// involves many server round-trips (one per collection element).
const FEW = { testCases: 20 };

// .filter(() => true) is a no-op filter that strips asBasic(),
// forcing the collection protocol path.
function nonBasic(gen: ReturnType<typeof gs.integers>) {
  return gen.filter(() => true);
}

describe("arrays with non-basic elements (collection protocol)", () => {
  test("generates arrays of filtered integers", () =>
    hegel.test((tc) => {
      const arr = tc.draw(
        gs.arrays(gs.integers({ minValue: 0, maxValue: 100 }).filter((x) => x > 5)),
      );
      for (const x of arr) {
        expect(x).toBeGreaterThan(5);
      }
    }, FEW));

  test("respects maxSize", () =>
    hegel.test((tc) => {
      const arr = tc.draw(
        gs.arrays(nonBasic(gs.integers({ minValue: 0, maxValue: 100 })), { maxSize: 5 }),
      );
      expect(arr.length).toBeLessThanOrEqual(5);
    }, FEW));

  test("respects minSize", () =>
    hegel.test((tc) => {
      const arr = tc.draw(
        gs.arrays(nonBasic(gs.integers({ minValue: 0, maxValue: 100 })), {
          minSize: 1,
          maxSize: 10,
        }),
      );
      expect(arr.length).toBeGreaterThanOrEqual(1);
    }, FEW));

  test("with no max: generates a list", () =>
    hegel.test((tc) => {
      const arr = tc.draw(gs.arrays(nonBasic(gs.integers({ minValue: 0, maxValue: 10 }))));
      expect(Array.isArray(arr)).toBe(true);
      for (const x of arr) {
        expect(x).toBeGreaterThanOrEqual(0);
        expect(x).toBeLessThanOrEqual(10);
      }
    }, FEW));

  test("unique arrays with non-basic elements reject duplicates", () =>
    hegel.test((tc) => {
      const arr = tc.draw(
        gs.arrays(nonBasic(gs.integers({ minValue: 0, maxValue: 20 })), {
          maxSize: 5,
          unique: true,
        }),
      );
      const uniqueCount = new Set(arr).size;
      expect(uniqueCount).toBe(arr.length);
    }, FEW));
});

describe("sets with non-basic elements (collection protocol)", () => {
  test("generates sets of filtered integers", () =>
    hegel.test((tc) => {
      const s = tc.draw(
        gs.sets(nonBasic(gs.integers({ minValue: 0, maxValue: 100 })), { maxSize: 5 }),
      );
      expect(s).toBeInstanceOf(Set);
      expect(s.size).toBeLessThanOrEqual(5);
    }, FEW));

  test("respects minSize", () =>
    hegel.test((tc) => {
      const s = tc.draw(
        gs.sets(nonBasic(gs.integers({ minValue: 0, maxValue: 100 })), { minSize: 1, maxSize: 10 }),
      );
      expect(s.size).toBeGreaterThanOrEqual(1);
    }, FEW));

  test("no maxSize: generates a Set", () =>
    hegel.test((tc) => {
      const s = tc.draw(gs.sets(nonBasic(gs.integers({ minValue: 0, maxValue: 100 }))));
      expect(s).toBeInstanceOf(Set);
    }, FEW));
});

describe("maps with non-basic elements (collection protocol)", () => {
  test("generates maps with filtered keys", () =>
    hegel.test((tc) => {
      const m = tc.draw(
        gs.maps(
          gs.integers({ minValue: 0, maxValue: 100 }).filter((x) => x > 5),
          gs.booleans(),
          {
            maxSize: 3,
          },
        ),
      );
      expect(m).toBeInstanceOf(Map);
      expect(m.size).toBeLessThanOrEqual(3);
      for (const key of m.keys()) {
        expect(key).toBeGreaterThan(5);
      }
    }, FEW));

  test("generates maps with filtered values", () =>
    hegel.test((tc) => {
      const m = tc.draw(
        gs.maps(
          gs.text({ minSize: 1, maxSize: 3 }),
          nonBasic(gs.integers({ minValue: 0, maxValue: 100 })),
          {
            maxSize: 3,
          },
        ),
      );
      expect(m).toBeInstanceOf(Map);
    }, FEW));

  test("respects minSize", () =>
    hegel.test((tc) => {
      const m = tc.draw(
        gs.maps(nonBasic(gs.integers({ minValue: 0, maxValue: 100 })), gs.booleans(), {
          minSize: 1,
          maxSize: 5,
        }),
      );
      expect(m.size).toBeGreaterThanOrEqual(1);
    }, FEW));

  test("no maxSize: generates a Map", () =>
    hegel.test((tc) => {
      const m = tc.draw(
        gs.maps(nonBasic(gs.integers({ minValue: 0, maxValue: 100 })), gs.booleans()),
      );
      expect(m).toBeInstanceOf(Map);
    }, FEW));
});

describe("arrays with uniqueBy", () => {
  test("keys from a function are distinct", () =>
    hegel.test((tc) => {
      const pairs = tc.draw(
        gs.arrays(gs.tuples(gs.integers({ minValue: 0, maxValue: 5 }), gs.booleans()), {
          uniqueBy: (pair) => pair[0],
          maxSize: 10,
        }),
      );
      expect(new Set(pairs.map((pair) => pair[0])).size).toBe(pairs.length);
    }, FEW));

  test("every key function's keys are distinct", () =>
    hegel.test((tc) => {
      const xs = tc.draw(
        gs.arrays(gs.integers({ minValue: 0, maxValue: 20 }), {
          uniqueBy: [(x) => x % 3, (x) => x % 5],
          minSize: 2,
        }),
      );
      expect(new Set(xs.map((x) => x % 3)).size).toBe(xs.length);
      expect(new Set(xs.map((x) => x % 5)).size).toBe(xs.length);
    }, FEW));

  test("invalid combinations throw", () => {
    expect(() => gs.arrays(gs.integers(), { unique: true, uniqueBy: (x) => x })).toThrow(
      "Cannot pass both unique and uniqueBy",
    );
    expect(() => gs.arrays(gs.integers(), { uniqueBy: [] })).toThrow(
      "uniqueBy must be a function or a non-empty array of functions",
    );
    expect(gs.arrays(gs.integers(), { uniqueBy: (x) => x }).asBasic()).toBeNull();
  });
});

describe("gs.permutations()", () => {
  test("generates reorderings of every element", () =>
    hegel.test((tc) => {
      const perm = tc.draw(gs.permutations(["a", "b", "c", "d"]));
      expect([...perm].sort()).toEqual(["a", "b", "c", "d"]);
      expect(tc.draw(gs.permutations([]))).toEqual([]);
    }, FEW));

  test("reorders, and shrinks toward the original order", () => {
    let reordered = false;
    let last: number[] = [];
    expect(() =>
      hegel.test(
        (tc) => {
          const perm = tc.draw(gs.permutations([1, 2, 3, 4, 5]));
          if (perm.join() !== "1,2,3,4,5") reordered = true;
          if (perm[4] !== 5) {
            last = perm;
            throw new Error("moved");
          }
        },
        { testCases: 200, database: hegel.Database.disabled },
      ),
    ).toThrow("moved");
    expect(reordered).toBe(true);
    expect(last).toEqual([1, 2, 3, 5, 4]);
  });
});
