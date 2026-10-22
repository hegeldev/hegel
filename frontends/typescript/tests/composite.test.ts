import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.composite()", () => {
  test("imperative generator works", () =>
    hegel.test(
      (tc) => {
        const pairGen = gs.composite((inner) => {
          const x = inner.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          const y = inner.draw(gs.integers({ minValue: x, maxValue: 100 }));
          return [x, y] as [number, number];
        });

        const [x, y] = tc.draw(pairGen);
        expect(x).toBeLessThanOrEqual(y);
      },
      { testCases: 20 },
    ));

  test("return type annotated on callback", () =>
    hegel.test(
      (tc) => {
        const pointGen = gs.composite((inner): { x: number; y: number } => {
          const x = inner.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          const y = inner.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          return { x, y };
        });

        const point = tc.draw(pointGen);
        expect(typeof point.x).toBe("number");
        expect(typeof point.y).toBe("number");
      },
      { testCases: 20 },
    ));

  test("return type passed as explicit type argument", () =>
    hegel.test(
      (tc) => {
        const pointGen = gs.composite<{ x: number; y: number }>((inner) => {
          const x = inner.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          const y = inner.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          return { x, y };
        });

        const point = tc.draw(pointGen);
        expect(typeof point.x).toBe("number");
        expect(typeof point.y).toBe("number");
      },
      { testCases: 20 },
    ));
});
