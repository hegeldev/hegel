import { describe, expect, it } from "vitest";
import { repr, splitLines, TextPrinter } from "../src/pretty.js";

describe("repr", () => {
  it.each([
    ["a string", 'with "quotes"\n', '"with \\"quotes\\"\\n"'],
    ["an integer", 3, "3"],
    ["a float", 1.5, "1.5"],
    ["negative zero", -0, "-0"],
    ["NaN", NaN, "NaN"],
    ["infinity", -Infinity, "-Infinity"],
    ["a bigint", -12n, "-12n"],
    ["a boolean", true, "true"],
    ["undefined", undefined, "undefined"],
    ["null", null, "null"],
    ["a symbol", Symbol("s"), "Symbol(s)"],
    ["a named function", function named() {}, "[Function named]"],
    ["an anonymous function", (() => () => {})(), "[Function (anonymous)]"],
    ["an array", [1, "a", [null]], '[1, "a", [null]]'],
    ["an empty array", [], "[]"],
    [
      "a Map",
      new Map<unknown, unknown>([
        [1, "a"],
        ["b", [2]],
      ]),
      'new Map([[1, "a"], ["b", [2]]])',
    ],
    ["a Set", new Set([1, 2]), "new Set([1, 2])"],
    ["a Uint8Array", Uint8Array.from([0, 255]), "Uint8Array.from([0, 255])"],
    ["a BigInt64Array", BigInt64Array.from([-1n]), "BigInt64Array.from([-1n])"],
    ["a Date", new Date(Date.UTC(2024, 1, 29)), 'new Date("2024-02-29T00:00:00.000Z")'],
    ["an invalid Date", new Date(NaN), "new Date(NaN)"],
    ["an Error", new TypeError("bad"), 'new TypeError("bad")'],
    [
      "a plain object",
      { a: 1, "not an identifier": 2, $b_2: 3 },
      '{ a: 1, "not an identifier": 2, $b_2: 3 }',
    ],
    ["an empty object", {}, "{}"],
    ["a null-prototype object", Object.assign(Object.create(null), { a: 1 }), "{ a: 1 }"],
  ])("prints %s", (_name, value, expected) => {
    expect(repr(value)).toBe(expected);
  });

  it("prints a DataView as an object rather than a typed array", () => {
    expect(repr(new DataView(new ArrayBuffer(2)))).toBe("DataView {}");
  });

  it("prints class instances with their class name", () => {
    class Point {
      constructor(
        readonly x: number,
        readonly y: number,
      ) {}
    }
    expect(repr(new Point(1, 2))).toBe("Point { x: 1, y: 2 }");
    const Anonymous = (() => class {})();
    Object.defineProperty(Anonymous, "name", { value: "" });
    expect(repr(new Anonymous())).toBe("{}");
    expect(repr(Object.create(Object.create(null)))).toBe("{}");
  });

  it("prints a repeated reference inside itself as [Circular], and a shared one in full", () => {
    const cycle: unknown[] = [1];
    cycle.push(cycle);
    expect(repr(cycle)).toBe("[1, [Circular]]");
    const shared = { a: 1 };
    expect(repr([shared, shared])).toBe("[{ a: 1 }, { a: 1 }]");
  });
});

describe("TextPrinter", () => {
  it("keeps committed regions and drops aborted ones", () => {
    const printer = new TextPrinter();
    printer.text("const a = ");
    printer.reflow("1");
    printer.hardBreak();
    printer.beginSpeculative();
    printer.text("dropped");
    printer.abortSpeculative();
    printer.beginSpeculative();
    printer.text("kept");
    printer.hardBreak();
    printer.commitSpeculative();
    expect(printer.lines()).toEqual(["const a = 1", "kept"]);
  });

  it("holds notes made inside a speculative region until it closes", () => {
    const printer = new TextPrinter();
    printer.beginSpeculative();
    printer.beginSpeculative();
    printer.text("x");
    printer.note("inner\nnote");
    printer.hardBreak();
    printer.commitSpeculative();
    printer.text("y");
    printer.hardBreak();
    printer.commitSpeculative();
    printer.note("after");
    expect(printer.lines()).toEqual(["x", "y", "inner", "note", "after"]);
  });

  it("indents an indented view's lines, starting afresh after an aborted region", () => {
    const printer = new TextPrinter();
    const inner = printer.indented(2);
    const innermost = inner.indented(2);
    inner.beginSpeculative();
    inner.text("dropped ");
    inner.abortSpeculative();
    inner.text("a = ");
    inner.reflow("1");
    inner.hardBreak();
    innermost.note("deep");
    printer.text("top");
    printer.hardBreak();
    expect(printer.lines()).toEqual(["  a = 1", "    deep", "top"]);
  });
});

describe("splitLines", () => {
  it.each([
    ["", []],
    ["a", ["a"]],
    ["a\n", ["a"]],
    ["a\n\nb\n", ["a", "", "b"]],
  ])("splits %j", (document, lines) => {
    expect(splitLines(document)).toEqual(lines);
  });
});
