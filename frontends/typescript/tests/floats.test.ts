import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.floats()", () => {
  test("exposes a schema via asBasic", () => {
    expect(gs.floats().asBasic()).not.toBeNull();
  });

  test("generates numbers in range", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats({ minValue: 0, maxValue: 1 }));
        expect(typeof v).toBe("number");
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(1);
      },
      { testCases: 20 },
    ));

  test("generates floats without bounds", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats());
        expect(typeof v).toBe("number");
      },
      { testCases: 10 },
    ));

  test("generates floats with only minValue", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats({ minValue: 0 }));
        expect(typeof v).toBe("number");
      },
      { testCases: 10 },
    ));
});
