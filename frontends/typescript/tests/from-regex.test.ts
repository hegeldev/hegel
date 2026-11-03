import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.fromRegex()", () => {
  test("exposes a schema via asBasic", () => {
    expect(gs.fromRegex("[0-9]+").asBasic()).not.toBeNull();
  });

  test("generates full matches by default", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.fromRegex("[0-9]{3}"));
        expect(v).toMatch(/^[0-9]{3}$/);
      },
      { testCases: 50 },
    ));

  test("fullmatch=true generates full matches", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.fromRegex("[0-9]{3}", { fullmatch: true }));
        expect(v).toMatch(/^[0-9]{3}$/);
      },
      { testCases: 50 },
    ));

  test("fullmatch=false allows partial matches", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.fromRegex("[a-z]+", { fullmatch: false }));
        expect(typeof v).toBe("string");
        expect(v).toMatch(/[a-z]+/);
      },
      { testCases: 20 },
    ));

  test("restricts the characters to an alphabet given as a string or characters()", () =>
    hegel.test(
      (tc) => {
        expect(tc.draw(gs.fromRegex("\\w+", { alphabet: "ab" }))).toMatch(/^[ab]+$/);
        expect(
          tc.draw(
            gs.fromRegex("x", { fullmatch: false, alphabet: gs.characters({ alphabet: "xy" }) }),
          ),
        ).toMatch(/^[xy]*x[xy]*$/);
      },
      { testCases: 50 },
    ));

  test("rejects an alphabet that is not a string or characters()", () => {
    expect(() => gs.fromRegex("a", { alphabet: gs.text() })).toThrow(
      "alphabet must be a string or a generator made by characters()",
    );
  });
});
