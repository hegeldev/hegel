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
