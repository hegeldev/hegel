import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.maps()", () => {
  test("generates Map instances with basic generators", () =>
    hegel.test(
      (tc) => {
        const m = tc.draw(
          gs.maps(
            gs.text({ minSize: 1, maxSize: 5 }),
            gs.integers({ minValue: 0, maxValue: 100 }),
            { maxSize: 3 },
          ),
        );
        expect(m).toBeInstanceOf(Map);
        expect(m.size).toBeLessThanOrEqual(3);
        for (const [k, v] of m.entries()) {
          expect(typeof k).toBe("string");
          expect(typeof v).toBe("number");
        }
      },
      { testCases: 30 },
    ));

  test("generates Map with minSize constraint", () =>
    hegel.test(
      (tc) => {
        const m = tc.draw(
          gs.maps(
            gs.text({ minSize: 1, maxSize: 5 }),
            gs.integers({ minValue: 0, maxValue: 100 }),
            { minSize: 1, maxSize: 5 },
          ),
        );
        expect(m.size).toBeGreaterThanOrEqual(1);
      },
      { testCases: 30 },
    ));

  test("applies key and value transforms", () =>
    hegel.test(
      (tc) => {
        const uppercaseKeys = gs.text({ minSize: 1, maxSize: 5 }).map((s) => s.toUpperCase());
        const negatedInts = gs.integers({ minValue: 1, maxValue: 100 }).map((n) => -n);
        const m = tc.draw(gs.maps(uppercaseKeys, negatedInts, { maxSize: 3 }));
        for (const [k, v] of m.entries()) {
          expect(k).toBe(k.toUpperCase());
          expect(v).toBeLessThan(0);
        }
      },
      { testCases: 30 },
    ));

  test("non-basic path (filtered keys) generates Map via collection protocol", () =>
    hegel.test(
      (tc) => {
        const filteredKeys = gs.text({ minSize: 1, maxSize: 3 }).filter((s) => s.length > 0);
        const m = tc.draw(
          gs.maps(filteredKeys, gs.integers({ minValue: 0, maxValue: 100 }), { maxSize: 3 }),
        );
        expect(m).toBeInstanceOf(Map);
        for (const [k, v] of m.entries()) {
          expect(typeof k).toBe("string");
          expect(k.length).toBeGreaterThan(0);
          expect(typeof v).toBe("number");
        }
      },
      { testCases: 30 },
    ));

  test("with minSize alone exercises the minSize-only branch", () =>
    hegel.test(
      (tc) => {
        const m = tc.draw(
          gs.maps(gs.integers({ minValue: 0, maxValue: 100 }), gs.booleans(), {
            minSize: 1,
            maxSize: 3,
          }),
        );
        expect(m.size).toBeGreaterThanOrEqual(1);
      },
      { testCases: 10 },
    ));
});
