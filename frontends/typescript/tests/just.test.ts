import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.just()", () => {
  test("returns constant value", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.just(42));
        expect(v).toBe(42);
      },
      { testCases: 10 },
    ));

  test("returns constant object (same reference)", () =>
    hegel.test(
      (tc) => {
        const obj = { x: 1, y: 2 };
        const v = tc.draw(gs.just(obj));
        expect(v).toBe(obj);
      },
      { testCases: 5 },
    ));
});
