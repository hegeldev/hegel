import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.arrays()", () => {
  test("all elements in range", () =>
    hegel.test(
      (tc) => {
        const xs = tc.draw(gs.arrays(gs.integers({ minValue: 0, maxValue: 100 })));
        expect(Array.isArray(xs)).toBe(true);
        for (const x of xs) {
          expect(x).toBeGreaterThanOrEqual(0);
          expect(x).toBeLessThanOrEqual(100);
        }
      },
      { testCases: 50 },
    ));

  test("respects minSize and maxSize", () =>
    hegel.test(
      (tc) => {
        const xs = tc.draw(gs.arrays(gs.booleans(), { minSize: 3, maxSize: 5 }));
        expect(xs.length).toBeGreaterThanOrEqual(3);
        expect(xs.length).toBeLessThanOrEqual(5);
        for (const x of xs) {
          expect(typeof x).toBe("boolean");
        }
      },
      { testCases: 50 },
    ));

  test("basic element with transform: transform applied per item", () =>
    hegel.test(
      (tc) => {
        const xs = tc.draw(
          gs.arrays(
            gs.integers({ minValue: 0, maxValue: 5 }).map((x) => x * 2),
            { maxSize: 5 },
          ),
        );
        for (const x of xs) {
          expect(x % 2).toBe(0);
          expect(x).toBeGreaterThanOrEqual(0);
          expect(x).toBeLessThanOrEqual(10);
        }
      },
      { testCases: 50 },
    ));

  test("non-basic elements: filtered values", () =>
    hegel.test(
      (tc) => {
        const xs = tc.draw(
          gs.arrays(
            gs.integers({ minValue: 0, maxValue: 10 }).filter((x) => x > 5),
            { minSize: 1, maxSize: 5 },
          ),
        );
        expect(xs.length).toBeGreaterThanOrEqual(1);
        expect(xs.length).toBeLessThanOrEqual(5);
        for (const x of xs) {
          expect(x).toBeGreaterThan(5);
        }
      },
      { testCases: 50 },
    ));

  test("nested arrays", () =>
    hegel.test(
      (tc) => {
        const xss = tc.draw(gs.arrays(gs.arrays(gs.booleans(), { maxSize: 3 }), { maxSize: 3 }));
        expect(Array.isArray(xss)).toBe(true);
        for (const xs of xss) {
          expect(Array.isArray(xs)).toBe(true);
          for (const x of xs) {
            expect(typeof x).toBe("boolean");
          }
        }
      },
      { testCases: 50 },
    ));

  test("unique option", () =>
    hegel.test(
      (tc) => {
        const xs = tc.draw(
          gs.arrays(gs.integers({ minValue: 0, maxValue: 100 }), {
            minSize: 1,
            maxSize: 10,
            unique: true,
          }),
        );
        const set = new Set(xs);
        expect(set.size).toBe(xs.length);
      },
      { testCases: 50 },
    ));

  test("throws when minSize > maxSize", () => {
    expect(() => gs.arrays(gs.integers(), { minSize: 5, maxSize: 3 })).toThrow();
  });

  test("accepts equal bounds", () => {
    expect(() => gs.arrays(gs.integers(), { minSize: 3, maxSize: 3 })).not.toThrow();
  });
});
