import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.booleans()", () => {
  test("exposes a schema via asBasic", () => {
    expect(gs.booleans().asBasic()).not.toBeNull();
  });

  test("generates booleans", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.booleans());
        expect(typeof v).toBe("boolean");
      },
      { testCases: 20 },
    ));
});

describe("gs.weightedBooleans()", () => {
  test("exposes a schema via asBasic and keeps the boolean label", () => {
    expect(gs.weightedBooleans(0.25).asBasic()).not.toBeNull();
    expect(gs.weightedBooleans(0.25).label).toEqual(gs.booleans().label);
  });

  test("generates booleans", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.weightedBooleans(0.25));
        expect(typeof v).toBe("boolean");
      },
      { testCases: 20 },
    ));

  test("p = 0 is always false and p = 1 is always true", () =>
    hegel.test(
      (tc) => {
        expect(tc.draw(gs.weightedBooleans(0))).toBe(false);
        expect(tc.draw(gs.weightedBooleans(1))).toBe(true);
      },
      { testCases: 20 },
    ));

  test("a high p produces mostly true", async () => {
    let trues = 0;
    await hegel.test(
      (tc) => {
        if (tc.draw(gs.weightedBooleans(0.99))) trues++;
      },
      { testCases: 100, database: hegel.Database.disabled },
    );
    expect(trues).toBeGreaterThan(50);
  });

  test.each([-0.1, 1.1, Number.NaN, "0.5"])("rejects p = %s", (p) => {
    expect(() => gs.weightedBooleans(p as number)).toThrow(
      "weightedBooleans(p) requires p between 0 and 1",
    );
  });
});
