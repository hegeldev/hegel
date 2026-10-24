import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.binary()", () => {
  test("exposes a schema via asBasic", () => {
    expect(gs.binary().asBasic()).not.toBeNull();
  });

  test("generates Uint8Array within size bounds", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.binary({ minSize: 0, maxSize: 10 }));
        expect(v).toBeInstanceOf(Uint8Array);
        expect(v.length).toBeLessThanOrEqual(10);
      },
      { testCases: 20 },
    ));

  test("generates Uint8Array with minSize", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.binary({ minSize: 2, maxSize: 8 }));
        expect(v).toBeInstanceOf(Uint8Array);
        expect(v.length).toBeGreaterThanOrEqual(2);
        expect(v.length).toBeLessThanOrEqual(8);
      },
      { testCases: 20 },
    ));

  test("with minSize alone exercises the minSize-only branch", () =>
    hegel.test(
      (tc) => {
        const b = tc.draw(gs.binary({ minSize: 5, maxSize: 10 }));
        expect(b.length).toBeGreaterThanOrEqual(5);
      },
      { testCases: 10 },
    ));

  test("without maxSize the engine's default maximum applies", () =>
    hegel.test(
      (tc) => {
        expect(tc.draw(gs.binary()).length).toBeLessThanOrEqual(100);
        const long = tc.draw(gs.binary({ minSize: 120 }));
        expect(long.length).toBeGreaterThanOrEqual(120);
        expect(long.length).toBeLessThanOrEqual(220);
      },
      { testCases: 50 },
    ));

  test("a maxSize at or above the engine's reserved default means no bound", () =>
    hegel.test(
      (tc) => {
        expect(tc.draw(gs.binary({ maxSize: 2 ** 64 }))).toBeInstanceOf(Uint8Array);
      },
      { testCases: 10 },
    ));
});
