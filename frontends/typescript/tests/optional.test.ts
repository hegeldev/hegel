import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.optional()", () => {
  test("generates null or a value", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.optional(gs.integers({ minValue: 0, maxValue: 100 })));
        if (v !== null) {
          expect(typeof v).toBe("number");
          expect(v).toBeGreaterThanOrEqual(0);
          expect(v).toBeLessThanOrEqual(100);
        }
      },
      { testCases: 50 },
    ));

  test("both null and non-null values appear", async () => {
    let seenNull = false;
    let seenValue = false;
    await hegel.test(
      (tc) => {
        const v = tc.draw(gs.optional(gs.integers({ minValue: 0, maxValue: 10 })));
        if (v === null) seenNull = true;
        else seenValue = true;
      },
      { testCases: 100 },
    );
    expect(seenNull).toBe(true);
    expect(seenValue).toBe(true);
  });

  test("optional with non-basic inner: generates null or value", () =>
    hegel.test(
      (tc) => {
        const filtered = gs.integers({ minValue: 0, maxValue: 10 }).filter(() => true);
        const v = tc.draw(gs.optional(filtered));
        if (v !== null) {
          expect(typeof v).toBe("number");
          expect(v).toBeGreaterThanOrEqual(0);
          expect(v).toBeLessThanOrEqual(10);
        }
      },
      { testCases: 50 },
    ));
});
