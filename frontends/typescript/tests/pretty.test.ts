import { describe, expect, it, vi } from "vitest";
import {
  Document,
  EngineDocument,
  prettyPrint,
  PrettyPrinter,
  Speculation,
  printedLine,
  splitLines,
} from "../src/pretty.js";
import { getLibhegel } from "../src/session.js";
import { installRuntime } from "../src/runnerCore.js";
import { nodeRuntime } from "../src/nodeRuntime.js";
import { browserRuntime } from "../src/browser/runtime.js";
import * as hegel from "@hegeldev/hegel";
import type { Engine } from "../src/engine.js";
import { wasmFixture } from "./wasmFixture.js";

const lib = getLibhegel();
const ctx = lib.newContext();

function repr(value: unknown): string {
  return printedLine(lib, ctx, (printer) => printer.value(value));
}

class Point {
  constructor(
    readonly x: unknown,
    readonly y: unknown,
  ) {}
  get sum(): unknown {
    return (this.x as number) + (this.y as number);
  }
}
class Empty {}

class Point3 extends Point {}

function evaluate(text: string): unknown {
  return new Function("Point", "Point3", "Empty", `return ${text};`)(
    Point,
    Point3,
    Empty,
  ) as unknown;
}

function document(body: (document: EngineDocument) => void): string[] {
  const doc = new EngineDocument(lib, ctx);
  try {
    body(doc);
    return doc.lines();
  } finally {
    doc.dispose();
  }
}

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

  it("prints a DataView as one over a copy of its bytes", () => {
    const view = new DataView(Uint8Array.from([9, 1, 2, 9]).buffer, 1, 2);
    const text = repr(view);
    expect(text).toBe("new DataView(Uint8Array.from([1, 2]).buffer)");
    expect(evaluate(text)).toEqual(new DataView(Uint8Array.from([1, 2]).buffer));
  });

  it("prints a RegExp as a literal", () => {
    expect(repr(/a b\//giu)).toBe("/a b\\//giu");
    expect(repr(new RegExp(""))).toBe("/(?:)/");
  });

  it("writes a __proto__ key as a computed key, so that it stays an own property", () => {
    const value = JSON.parse('{"__proto__": 1}') as object;
    const text = repr(value);
    expect(text).toBe('{ ["__proto__"]: 1 }');
    expect(evaluate(text)).toStrictEqual(value);
  });

  it("prints a class instance as an object created from its class's prototype", () => {
    expect(repr(new Point(1, 2))).toBe(
      "Object.assign(Object.create(Point.prototype), { x: 1, y: 2 })",
    );
    expect(repr(new Empty())).toBe("Object.create(Empty.prototype)");
    expect(repr([new Point(new Empty(), [new Point(0, "")])])).toBe(
      "[Object.assign(Object.create(Point.prototype), { x: Object.create(Empty.prototype), " +
        'y: [Object.assign(Object.create(Point.prototype), { x: 0, y: "" })] })]',
    );
  });

  it.each([
    ["an instance", () => new Point(1, -0)],
    ["an instance with no properties", () => new Empty()],
    [
      "nested instances",
      () => new Point(new Point(1n, "a"), [new Empty(), new Map([[1, new Point(2, 3)]])]),
    ],
    ["a subclass instance", () => new Point3(1, 2)],
  ])("prints %s as an expression that evaluates to an equivalent instance", (_name, make) => {
    const value = make();
    const copy = evaluate(repr(value));
    expect(copy).toStrictEqual(value);
    expect(Object.getPrototypeOf(copy)).toBe(Object.getPrototypeOf(value));
  });

  it("leaves out symbol-keyed, non-enumerable and inherited properties", () => {
    const point = new Point(1, 2);
    Object.defineProperty(point, "hidden", { value: 3, enumerable: false });
    Object.assign(point, { [Symbol.for("s")]: 4 });
    const copy = evaluate(repr(point)) as Point;
    expect(repr(point)).toBe("Object.assign(Object.create(Point.prototype), { x: 1, y: 2 })");
    expect(copy.sum).toBe(3);
  });

  it("prints an instance containing itself with [Circular]", () => {
    const point = new Point(1, null);
    (point as { y: unknown }).y = [point];
    expect(repr(point)).toBe(
      "Object.assign(Object.create(Point.prototype), { x: 1, y: [[Circular]] })",
    );
  });

  it("prints an object whose class it cannot name as an object literal", () => {
    const Anonymous = (() => class {})();
    Object.defineProperty(Anonymous, "name", { value: "" });
    expect(repr(Object.assign(new Anonymous(), { a: 1 }))).toBe("{ a: 1 }");
    class Spaced {}
    Object.defineProperty(Spaced, "name", { value: "not an identifier" });
    expect(repr(new Spaced())).toBe("{}");
    class Numbered {}
    Object.defineProperty(Numbered, "name", { value: 1 });
    expect(repr(new Numbered())).toBe("{}");
    expect(repr(Object.create(new Point(1, 2)))).toBe("{}");
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

describe("EngineDocument", () => {
  it("keeps committed regions and drops aborted ones", () => {
    const lines = document((doc) => {
      const printer = doc.view();
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
    });
    expect(lines).toEqual(["const a = 1", "kept"]);
  });

  it("holds notes made inside a speculative region until it closes", () => {
    const lines = document((doc) => {
      const printer = doc.view();
      printer.beginSpeculative();
      printer.beginSpeculative();
      printer.text("x");
      doc.note(0, "inner\nnote");
      printer.hardBreak();
      printer.commitSpeculative();
      printer.text("y");
      printer.hardBreak();
      printer.commitSpeculative();
      doc.note(0, "after");
    });
    expect(lines).toEqual(["x", "y", "inner", "note", "after"]);
  });

  it("indents an indented view's lines, starting afresh after an aborted region", () => {
    const lines = document((doc) => {
      const printer = doc.view();
      const inner = doc.view(2);
      inner.beginSpeculative();
      inner.text("dropped ");
      inner.abortSpeculative();
      inner.text("a = ");
      inner.reflow("1");
      inner.hardBreak();
      doc.note(4, "deep");
      printer.text("top");
      printer.hardBreak();
      printer.hardBreak();
      printer.text("after a blank line");
    });
    expect(lines).toEqual(["  a = 1", "    deep", "top", "", "after a blank line"]);
  });

  it("lays a group out within the width, and never breaks one on a single line", () => {
    const long = Array.from({ length: 30 }, (_, i) => `item${i}`);
    const lines = document((doc) => {
      const printer = doc.view(2);
      printer.text("const xs = ");
      new PrettyPrinter(printer).value(long);
      printer.text(";");
    });
    expect(lines).toEqual(["  const xs = [", ...long.map((item) => `      "${item}",`), "  ];"]);
    expect(repr(long)).toBe(`[${long.map((item) => `"${item}"`).join(", ")}]`);
  });
});

describe.each([
  ["native", () => getLibhegel() as Engine],
  ["wasm", () => wasmFixture().engine as Engine],
])("printedLine on the %s engine", (_name, engine) => {
  it("prints a value on one line however long it is", () => {
    const wide = engine();
    const wideCtx = wide.newContext();
    const long = Array.from({ length: 40 }, (_, i) => i);
    expect(printedLine(wide, wideCtx, (printer) => printer.value(long))).toBe(
      `[${long.join(", ")}]`,
    );
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

describe("printing through a class's own prettyPrint method", () => {
  class Pair {
    constructor(
      readonly a: unknown,
      readonly b: unknown,
    ) {}
    [prettyPrint](p: PrettyPrinter): void {
      p.group(5, "Pair(", ")", () => {
        p.value(this.a);
        p.text(",");
        p.breakable();
        p.value(this.b);
        p.ifBreak(",");
        p.shiftIndent(-5);
        p.breakable("");
        p.shiftIndent(5);
      });
      p.reflow(" /* ok */");
      p.comment("pair");
    }
  }

  it("prints through the engine's layout, nested values included", () => {
    expect(repr(new Pair(1, new Pair("x", [2n])))).toBe(
      ["Pair(1,", '     Pair("x", [2n]) /* ok */,  // pair', ") /* ok */  // pair"].join("\n"),
    );
  });

  it("is the symbol exported from the package", () => {
    expect(hegel.prettyPrint).toBe(prettyPrint);
    expect(hegel.PrettyPrinter).toBe(PrettyPrinter);
    expect(prettyPrint).toBe(Symbol.for("hegel.prettyPrint"));
  });

  it("prints a value that contains itself through the hook as circular", () => {
    const pair = new Pair(1, null);
    (pair as { b: unknown }).b = pair;
    expect(repr(pair)).toBe("Pair(1, [Circular]) /* ok */  // pair");
  });

  it("prints hard breaks through the hook", () => {
    class Lines {
      [prettyPrint](p: PrettyPrinter): void {
        p.text("a");
        p.hardBreak();
        p.text("b\n\nc");
      }
    }
    expect(repr(new Lines())).toBe("a\nb\n\nc");
  });

  it.each([
    [
      "a separator",
      (p: PrettyPrinter) => p.breakable("\n"),
      "a break point's separator must not contain a newline",
    ],
    ["if-break text", (p: PrettyPrinter) => p.ifBreak("\n"), "text must not contain a newline"],
    ["a comment", (p: PrettyPrinter) => p.comment("\n"), "a comment must not contain a newline"],
    [
      "an opening",
      (p: PrettyPrinter) => p.beginGroup(1, "\n"),
      "a group's opening text must not contain a newline",
    ],
    [
      "a closing",
      (p: PrettyPrinter) => p.endGroup("\n"),
      "a group's closing text must not contain a newline",
    ],
  ])("rejects a newline in %s", (_what, write, message) => {
    const doc = new EngineDocument(lib, ctx);
    try {
      expect(() => write(new PrettyPrinter(doc.view()))).toThrow(message);
    } finally {
      doc.dispose();
    }
  });
});

describe("PrettyPrinter", () => {
  function render(body: (p: PrettyPrinter) => void, maxWidth?: number): string {
    const doc = new Document(maxWidth === undefined ? {} : { maxWidth });
    body(doc.printer);
    return doc.finish();
  }

  it.each([79, 20, 8, 1])("lays out seq as an array value prints at width %i", (width) => {
    const items = [1000, "two", [3, 4], null];
    expect(render((p) => p.seq("[", "]", items, (q, item) => q.value(item)), width)).toBe(
      render((p) => p.value(items), width),
    );
    expect(render((p) => p.seq("[", "]", [], (q, item) => q.value(item)), width)).toBe("[]");
  });

  it("separates group items with a comma and a break point", () => {
    const pair = (p: PrettyPrinter) =>
      p.group(1, "(", ")", () => {
        p.text("first");
        p.separator();
        p.text("second");
      });
    expect(render(pair)).toBe("(first, second)");
    expect(render(pair, 8)).toBe("(first,\n second)");
  });

  it("returns the group body's result and passes it the printer", () => {
    let seen: PrettyPrinter | null = null;
    expect(
      render((p) => {
        expect(
          p.group(1, "<", ">", (q) => {
            seen = q;
            return 7;
          }),
        ).toBe(7);
        expect(seen).toBe(p);
      }),
    ).toBe("<>");
  });

  it("keeps a committed speculation and drops an aborted one", () => {
    expect(
      render((p) => {
        const dropped = p.speculate();
        dropped.printer.text("dropped");
        dropped.abort();
        dropped.commit();
        const kept = p.speculate();
        kept.printer.text("kept");
        kept.commit();
        kept.abort();
      }),
    ).toBe("kept");
  });

  it("prefixes a comment and keeps text's newlines", () => {
    expect(render((p) => (p.text("a\nb"), p.comment("why")))).toBe("a\nb  // why");
  });

  it("splices deferred regions, nested ones included, in where they were opened", () => {
    expect(
      render((p) => {
        p.text("f(");
        const outer = p.deferred();
        p.text(")");
        outer.value(1);
        const inner = outer.deferred();
        outer.text("!");
        inner.text(" and 2");
        expect(inner.shouldPrint()).toBe(true);
      }),
    ).toBe("f(1 and 2!)");
  });

  it("discards everything printed to the no-op printer", () => {
    const p = PrettyPrinter.noop();
    expect(p.shouldPrint()).toBe(false);
    p.text("a\nb");
    p.breakable();
    p.ifBreak(",");
    p.comment("c");
    p.hardBreak();
    p.shiftIndent(2);
    p.reflow("r");
    p.value([1]);
    p.seq("[", "]", [1], (q, n) => q.value(n));
    expect(p.group(1, "(", ")", () => 3)).toBe(3);
    const speculation = p.speculate();
    speculation.commit();
    p.speculate().abort();
    expect(p.deferred().shouldPrint()).toBe(false);
  });

  it("stops printing once its document is read", () => {
    const doc = new EngineDocument(lib, ctx);
    try {
      const view = doc.view();
      const p = new PrettyPrinter(view);
      p.text("kept");
      const slot = p.deferred();
      expect(doc.lines()).toEqual(["kept"]);
      expect(p.shouldPrint()).toBe(false);
      expect(slot.shouldPrint()).toBe(false);
      p.text("late");
      p.hardBreak();
      p.shiftIndent(1);
      slot.text("late");
      expect(p.deferred().shouldPrint()).toBe(false);
      expect(view.deferred()).toBeNull();
      expect(doc.lines()).toEqual(["kept"]);
    } finally {
      doc.dispose();
    }
  });
});

describe("Document", () => {
  it("is laid out to its width, 79 by default", () => {
    const long = Array.from({ length: 20 }, (_, i) => i);
    const doc = new Document();
    doc.printer.value(long);
    expect(doc.finish()).toBe(`[${long.join(", ")}]`);
    const narrow = new Document({ maxWidth: 8 });
    narrow.printer.seq("[", "]", [1, 2, 3], (p, n) => p.value(n));
    expect(narrow.finish()).toBe("[\n    1,\n    2,\n    3,\n]");
  });

  it("wraps a class instance's properties like an object literal's", () => {
    const point = new Point("a".repeat(30), [1, 2, 3]);
    const doc = new Document({ maxWidth: 40 });
    doc.printer.value([point]);
    const text = doc.finish();
    expect(text).toBe(
      [
        "[",
        "    Object.assign(Object.create(Point.prototype), {",
        `        x: "${"a".repeat(30)}",`,
        "        y: [1, 2, 3],",
        "    }),",
        "]",
      ].join("\n"),
    );
    expect(evaluate(text)).toStrictEqual([point]);
  });

  it("can only be finished once", () => {
    const doc = new Document();
    expect(doc.finish()).toBe("");
    expect(() => doc.finish()).toThrow("Document.finish was already called");
    expect(doc.printer.shouldPrint()).toBe(false);
  });

  it.each([0, -1, 1.5, NaN])("rejects a width of %s", (maxWidth) => {
    expect(() => new Document({ maxWidth })).toThrow(
      `maxWidth must be a positive integer, got ${String(maxWidth)}`,
    );
  });

  it.each([
    ["native", () => nodeRuntime],
    ["Wasm", () => browserRuntime(wasmFixture().engine)],
  ])("is laid out by the installed %s engine, deferred regions included", (_, runtime) => {
    const installed = runtime();
    const engine = installed.getEngine();
    const newContext = vi.spyOn(engine, "newContext");
    installRuntime(installed);
    try {
      const doc = new Document({ maxWidth: 12 });
      doc.printer.beginGroup(9, "recorder(");
      const slot = doc.printer.deferred();
      doc.printer.endGroup(")");
      [1, 2].forEach((n, i) => {
        if (i > 0) slot.separator();
        slot.value(n);
      });
      expect(doc.finish()).toBe("recorder(1,\n         2)");
      expect(newContext).toHaveBeenCalledOnce();
      const narrow = new Document({ maxWidth: 8 });
      narrow.printer.text("f(");
      const hole = narrow.printer.deferred();
      narrow.printer.text(")");
      hole.seq("[", "]", [1, 2, 3], (p, n) => p.value(n));
      expect(narrow.finish()).toBe("f([\n    1,\n    2,\n    3,\n])");
    } finally {
      newContext.mockRestore();
      installRuntime(nodeRuntime);
    }
  });

  it("is exported from the package", () => {
    expect(hegel.Document).toBe(Document);
    expect(hegel.Speculation).toBe(Speculation);
  });
});
