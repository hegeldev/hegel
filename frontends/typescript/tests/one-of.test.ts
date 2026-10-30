import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.oneOf()", () => {
  test("throws if 0 generators provided", () => {
    expect(() => gs.oneOf()).toThrow("oneOf requires at least one generator");
  });

  test("accepts 1 generator", () => {
    expect(() => gs.oneOf(gs.integers())).not.toThrow();
  });

  test("accepts 2 generators", () => {
    expect(() => gs.oneOf(gs.integers(), gs.booleans())).not.toThrow();
  });

  test("generates values from one of the branches", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(
          gs.oneOf(
            gs.integers({ minValue: 0, maxValue: 10 }),
            gs.integers({ minValue: 100, maxValue: 200 }),
          ),
        );
        expect((v >= 0 && v <= 10) || (v >= 100 && v <= 200)).toBe(true);
      },
      { testCases: 50 },
    ));

  test("generates values from both branches across many runs", async () => {
    const low: number[] = [];
    const high: number[] = [];
    await hegel.test(
      (tc) => {
        const v = tc.draw(
          gs.oneOf(
            gs.integers({ minValue: 0, maxValue: 10 }),
            gs.integers({ minValue: 100, maxValue: 200 }),
          ),
        );
        if (v <= 10) low.push(v);
        else high.push(v);
      },
      { testCases: 100 },
    );
    expect(low.length).toBeGreaterThan(0);
    expect(high.length).toBeGreaterThan(0);
  });

  test("with transforms: dispatches per-branch transform by index", () =>
    hegel.test(
      (tc) => {
        const gen1 = gs.integers({ minValue: 0, maxValue: 5 }).map((x) => x * 2);
        const gen2 = gs.integers({ minValue: 100, maxValue: 105 }).map((x) => x + 1);
        const v = tc.draw(gs.oneOf(gen1, gen2));
        // gen1 produces 0,2,4,6,8,10; gen2 produces 101,102,103,104,105,106
        const isFromGen1 = v >= 0 && v <= 10 && v % 2 === 0;
        const isFromGen2 = v >= 101 && v <= 106;
        expect(isFromGen1 || isFromGen2).toBe(true);
      },
      { testCases: 50 },
    ));

  test("composite path: non-basic generators", () =>
    hegel.test(
      (tc) => {
        const filtered = gs.integers({ minValue: 0, maxValue: 10 }).filter(() => true);
        const v = tc.draw(gs.oneOf(filtered, gs.text({ minSize: 0, maxSize: 5 })));
        expect(typeof v === "number" || typeof v === "string").toBe(true);
      },
      { testCases: 50 },
    ));

  test("composite path generates values from either branch", () =>
    hegel.test(
      (tc) => {
        const filtered = gs.integers({ minValue: 0, maxValue: 100 }).filter(() => true);
        const v = tc.draw(gs.oneOf(filtered, gs.text({ minSize: 0, maxSize: 5 })));
        // Must be a number or string -- validates generator produces valid output
        expect(typeof v === "number" || typeof v === "string").toBe(true);
      },
      { testCases: 50 },
    ));
});
