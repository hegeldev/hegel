import { describe, test, it, expect, afterAll } from "vitest";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import type { Generator } from "../src/generators/core.js";
import { wasmFixture } from "./wasmFixture.js";
import { createRunner, Database } from "../src/runnerCore.js";
import { browserRuntime } from "../src/browser/runtime.js";
import { Libhegel } from "../src/libhegel.js";
import { testLibPath } from "./libPath.js";
import { AssumeError, StopTestError } from "../src/testCase.js";
import type { Engine } from "../src/engine.js";

const deterministic = { derandomize: true, database: hegel.Database.disabled };

const ARITHMETIC = `
  ?start: sum
  ?sum: product | sum "+" product
  ?product: atom | product "*" atom
  ?atom: NUMBER | "(" sum ")"
  %import common.NUMBER
`;

const NAMES = `
  start: NAME ("," NAME)*
  %declare NAME
`;

function minimal<T>(generator: Generator<T>, failing: (value: T) => boolean): T {
  let last: T | undefined;
  expect(() =>
    hegel.test(
      (tc) => {
        const value = tc.draw(generator);
        if (failing(value)) {
          last = value;
          throw new Error("found");
        }
      },
      { testCases: 1000, ...deterministic },
    ),
  ).toThrow("found");
  return last as T;
}

const dirs: string[] = [];
function grammarDir(files: Record<string, string>): string {
  const dir = mkdtempSync(join(tmpdir(), "hegel-lark-"));
  dirs.push(dir);
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(dir, path, ".."), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  return dir;
}
afterAll(() => {
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
});

describe("gs.fromLark()", () => {
  test("generates strings of the grammar within maxSize", () =>
    hegel.test(
      (tc) => {
        const s = tc.draw(gs.fromLark(ARITHMETIC, { maxSize: 50 }));
        expect(s).toMatch(/^[-0-9+*().eE]+$/);
        expect(s.length).toBeLessThanOrEqual(50);
      },
      { testCases: 50 },
    ));

  test("shrinks toward the grammar's simplest strings", () => {
    expect(minimal(gs.fromLark(ARITHMETIC), (s) => s.includes("+"))).toBe("0+0");
  });

  test("generates strings of the start rule given", () =>
    hegel.test(
      (tc) => {
        expect(tc.draw(gs.fromLark(`start: "a"\nother: "b"+`, { start: "other" }))).toMatch(/^b+$/);
      },
      { testCases: 20 },
    ));

  test("maxDepth bounds how deeply rules nest", () =>
    hegel.test(
      (tc) => {
        const s = tc.draw(gs.fromLark(`start: "(" start ")" | "x"`, { maxDepth: 3 }));
        expect(s.length).toBeLessThanOrEqual(5);
      },
      { testCases: 30 },
    ));

  test("draws explicit terminals from their generators", () =>
    hegel.test(
      (tc) => {
        const s = tc.draw(
          gs.fromLark(NAMES, {
            explicit: { NAME: gs.text({ alphabet: "xyz", minSize: 1, maxSize: 3 }) },
            maxSize: 30,
          }),
        );
        expect(s).toMatch(/^[xyz]{1,3}(,[xyz]{1,3})*$/);
        expect(s.length).toBeLessThanOrEqual(30);
      },
      { testCases: 50 },
    ));

  test("shrinks explicit terminals' values", () => {
    const names = gs.fromLark(NAMES, {
      explicit: { NAME: gs.text({ alphabet: "xyz", minSize: 1, maxSize: 3 }) },
      maxSize: 40,
    });
    expect(minimal(names, (s) => s.includes(","))).toBe("x,x");
  });

  test("an explicit terminal may itself be a grammar", () =>
    hegel.test(
      (tc) => {
        const s = tc.draw(
          gs.fromLark(NAMES, {
            explicit: { NAME: gs.fromLark(`start: "n" DIGIT\n%import common.DIGIT`) },
            maxSize: 60,
          }),
        );
        expect(s).toMatch(/^n[0-9](,n[0-9])*$/);
      },
      { testCases: 30 },
    ));

  test("values that cannot fit maxSize reject the test case", () => {
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(
            gs.fromLark(NAMES, {
              explicit: { NAME: gs.text({ alphabet: "x", minSize: 20, maxSize: 20 }) },
              maxSize: 10,
            }),
          );
        },
        { testCases: 10, ...deterministic },
      ),
    ).toThrow(hegel.RunError);
  });

  test("an explicit terminal must generate strings", () => {
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(
            gs.fromLark(NAMES, {
              explicit: { NAME: gs.integers() as unknown as Generator<string> },
            }),
          );
        },
        { testCases: 5, ...deterministic },
      ),
    ).toThrow("explicit terminal NAME must generate strings, got number");
  });

  test("restricts terminals to an alphabet given as a string or characters()", () =>
    hegel.test(
      (tc) => {
        const word = `start: WORD\nWORD: /[a-z]+/`;
        expect(tc.draw(gs.fromLark(word, { alphabet: "ab" }))).toMatch(/^[ab]+$/);
        expect(
          tc.draw(gs.fromLark(word, { alphabet: gs.characters({ minCodepoint: 120 }) })),
        ).toMatch(/^[x-z]+$/);
      },
      { testCases: 30 },
    ));

  test("rejects invalid options when the generator is made", () => {
    expect(() => gs.fromLark(ARITHMETIC, { maxSize: 0 })).toThrow(
      "maxSize must be an integer of at least 1, got 0",
    );
    expect(() => gs.fromLark(ARITHMETIC, { maxDepth: 1.5 })).toThrow(
      "maxDepth must be an integer of at least 1, got 1.5",
    );
    expect(() => gs.fromLark(ARITHMETIC, { alphabet: gs.text() })).toThrow(
      "alphabet must be a string or a generator made by characters()",
    );
  });

  test("reports a bad grammar when the test runs", () => {
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(gs.fromLark(`start: UNDEFINED`));
        },
        { testCases: 5, ...deterministic },
      ),
    ).toThrow(/UNDEFINED/);
  });

  test("labels generators by their grammar and terminals", () => {
    const name = gs.text();
    expect(gs.fromLark(NAMES, { explicit: { NAME: name } }).label).toEqual(
      gs.fromLark(NAMES, { explicit: { NAME: name } }).label,
    );
    expect(gs.fromLark(ARITHMETIC).label).not.toEqual(gs.fromLark(NAMES).label);
  });

  test("reads imported grammars from importPath, with Lark's bundled ones as fallback", () => {
    const importPath = grammarDir({
      "words.lark": `WORD: "w" DIGIT\n%import common.DIGIT`,
      "nested/inner.lark": `%import .lib.ITEM\nPAIR: ITEM ITEM`,
      "nested/lib.lark": `ITEM: "i"`,
    });
    hegel.test(
      (tc) => {
        expect(tc.draw(gs.fromLark(`start: WORD\n%import words.WORD`, { importPath }))).toMatch(
          /^w[0-9]$/,
        );
        expect(tc.draw(gs.fromLark(`start: PAIR\n%import nested.inner.PAIR`, { importPath }))).toBe(
          "ii",
        );
      },
      { testCases: 10 },
    );
  });

  test("reports a missing relative import and an unreadable grammar file", () => {
    const importPath = grammarDir({ "unreadable.lark/x": "" });
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(gs.fromLark(`start: X\n%import .missing.X`, { importPath }));
        },
        { testCases: 5, ...deterministic },
      ),
    ).toThrow(/missing/);
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(gs.fromLark(`start: X\n%import unreadable.X`, { importPath }));
        },
        { testCases: 5, ...deterministic },
      ),
    ).toThrow(/EISDIR/);
  });

  test("an explicit example supplies the string itself", () => {
    const seen: string[] = [];
    new hegel.Hegel((tc) => {
      seen.push(tc.draw(gs.fromLark(ARITHMETIC)));
    })
      .example("1+2")
      .settings({ testCases: 1, ...deterministic })
      .runSync();
    expect(seen[0]).toBe("1+2");
  });
});

describe("fromLark in the browser", () => {
  it("draws grammar strings with explicit terminals from the Wasm engine", () => {
    const runner = createRunner(browserRuntime(wasmFixture().engine));
    runner.test(
      (tc) => {
        expect(tc.draw(gs.fromLark(ARITHMETIC, { maxSize: 30 }))).toMatch(/^[-0-9+*().eE]+$/);
        expect(
          tc.draw(
            gs.fromLark(NAMES, { explicit: { NAME: gs.just("n") }, alphabet: "n,", maxSize: 30 }),
          ),
        ).toMatch(/^n(,n)*$/);
      },
      { testCases: 30, database: Database.disabled },
    );
  });

  it("cannot read imported grammar files", () => {
    const runner = createRunner(browserRuntime(wasmFixture().engine));
    expect(() =>
      runner.test(
        (tc) => {
          tc.draw(gs.fromLark(`start: X\n%import lib.X`, { importPath: "/grammars" }));
        },
        { testCases: 5, database: Database.disabled },
      ),
    ).toThrow("Reading grammar files (importPath) is not supported on this host");
  });
});

function withTestCase<T>(
  engine: Engine,
  body: (
    ctx: ReturnType<Engine["newContext"]>,
    tc: NonNullable<ReturnType<Engine["nextTestCase"]>>,
  ) => T,
): T {
  const ctx = engine.newContext();
  const settings = engine.newSettings(ctx);
  engine.setSeed(settings, 7n);
  engine.setDatabase(ctx, settings, "");
  const run = engine.runStart(ctx, settings);
  const tc = engine.nextTestCase(ctx, run)!;
  try {
    return body(ctx, tc);
  } finally {
    engine.freeTestCase(tc);
    engine.freeRun(run);
    engine.freeSettings(settings);
    engine.freeContext(ctx);
  }
}

describe.each([
  ["native", () => Libhegel.load(testLibPath()) as Engine],
  ["wasm", () => wasmFixture().engine as Engine],
])("the %s engine's grammar operations", (_name, load) => {
  it("gathers imported files, then draws with explicit terminals", () => {
    const engine = load();
    withTestCase(engine, (ctx, tc) => {
      const builder = engine.grammarBuilderNew(ctx, `start: A B\n%import lib.A\n%declare B`);
      try {
        expect(engine.grammarBuilderFile(ctx, builder, 0)).toBe("lib.lark");
        expect(engine.grammarBuilderFile(ctx, builder, 1)).toBeNull();
        expect(() => engine.grammarBuilderProvide(ctx, builder, 0, null)).toThrow(/lib/);
        engine.grammarBuilderProvide(ctx, builder, 0, `A: "a"`);
        expect(engine.grammarBuilderFile(ctx, builder, 1)).toBeNull();
        const grammar = engine.newGrammar(ctx, builder, {
          start: null,
          maxSize: 0,
          maxDepth: 0,
          alphabet: null,
          explicitNames: ["B"],
        });
        try {
          const draw = engine.grammarDrawNew(ctx, grammar, 0);
          try {
            expect(() => engine.grammarDrawResult(ctx, draw)).toThrow();
            expect(engine.grammarDrawNext(ctx, tc, draw)).toBe(0);
            expect(() => engine.grammarDrawNext(ctx, tc, draw)).toThrow();
            engine.grammarDrawProvide(ctx, draw, "b");
            expect(() => engine.grammarDrawProvide(ctx, draw, "c")).toThrow();
            expect(engine.grammarDrawNext(ctx, tc, draw)).toBeNull();
            expect(engine.grammarDrawResult(ctx, draw)).toBe("ab");
          } finally {
            engine.freeGrammarDraw(draw);
          }
        } finally {
          engine.freeGrammar(grammar);
        }
      } finally {
        engine.freeGrammarBuilder(builder);
      }
    });
  });

  it("reports a grammar that does not parse or compile", () => {
    const engine = load();
    const ctx = engine.newContext();
    try {
      expect(() => engine.grammarBuilderNew(ctx, "start: (")).toThrow();
      const builder = engine.grammarBuilderNew(ctx, "start: B");
      try {
        expect(() =>
          engine.newGrammar(ctx, builder, {
            start: null,
            maxSize: 0,
            maxDepth: 0,
            alphabet: null,
            explicitNames: [],
          }),
        ).toThrow(/'B' used but not defined/);
      } finally {
        engine.freeGrammarBuilder(builder);
      }
    } finally {
      engine.freeContext(ctx);
    }
  });

  it("maps a rejected and an overrun grammar draw to control flow", () => {
    const engine = load();
    withTestCase(engine, (ctx, tc) => {
      const builder = engine.grammarBuilderNew(ctx, `start: B\n%declare B`);
      const grammar = engine.newGrammar(ctx, builder, {
        start: null,
        maxSize: 1,
        maxDepth: 0,
        alphabet: null,
        explicitNames: ["B"],
      });
      engine.freeGrammarBuilder(builder);
      try {
        const draw = engine.grammarDrawNew(ctx, grammar, 0);
        try {
          let thrown: unknown;
          try {
            for (;;) {
              expect(engine.grammarDrawNext(ctx, tc, draw)).toBe(0);
              engine.grammarDrawProvide(ctx, draw, "too long");
            }
          } catch (error) {
            thrown = error;
          }
          expect(thrown instanceof AssumeError || thrown instanceof StopTestError).toBe(true);
        } finally {
          engine.freeGrammarDraw(draw);
        }
      } finally {
        engine.freeGrammar(grammar);
      }
    });
  });
});
