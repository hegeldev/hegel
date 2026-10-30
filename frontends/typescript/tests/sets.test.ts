import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.sets()", () => {
  test("generates Set instances", () =>
    hegel.test(
      (tc) => {
        const s = tc.draw(gs.sets(gs.integers({ minValue: 0, maxValue: 100 }), { maxSize: 5 }));
        expect(s).toBeInstanceOf(Set);
        expect(s.size).toBeLessThanOrEqual(5);
        for (const x of s) {
          expect(x).toBeGreaterThanOrEqual(0);
          expect(x).toBeLessThanOrEqual(100);
        }
      },
      { testCases: 30 },
    ));

  test("respects minSize", () =>
    hegel.test(
      (tc) => {
        const s = tc.draw(
          gs.sets(gs.integers({ minValue: 0, maxValue: 1000 }), { minSize: 2, maxSize: 5 }),
        );
        expect(s.size).toBeGreaterThanOrEqual(2);
        expect(s.size).toBeLessThanOrEqual(5);
      },
      { testCases: 30 },
    ));

  test("with minSize alone exercises the minSize-only branch", () =>
    hegel.test(
      (tc) => {
        const s = tc.draw(
          gs.sets(gs.integers({ minValue: 0, maxValue: 100 }), { minSize: 1, maxSize: 5 }),
        );
        expect(s.size).toBeGreaterThanOrEqual(1);
      },
      { testCases: 10 },
    ));
});
