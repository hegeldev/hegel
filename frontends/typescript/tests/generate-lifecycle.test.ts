import { describe, it, expect, vi } from "vitest";
import { generateValue, StringGeneratorCache, type GrammarSpec } from "../src/generate.js";
import { EngineError, type Engine } from "../src/engine.js";
import { getLibhegel } from "../src/session.js";
import { wasmFixture } from "./wasmFixture.js";
import { AssumeError, StopTestError } from "../src/testCase.js";
import { fakeEngine, context, testCase } from "./fakeEngine.js";

const boolean = { type: "boolean" };
const compoundSchemas = [
  { type: "tuple", elements: [boolean] },
  { type: "one_of", generators: [boolean] },
  { type: "list", elements: boolean, min_size: 0, max_size: 1, unique: false },
  { type: "dict", keys: boolean, values: boolean, min_size: 0, max_size: 1 },
];

describe("generateValue compound cleanup", () => {
  for (const schema of compoundSchemas) {
    it(`closes ${schema.type} spans and collections after a nested draw fails`, () => {
      const engine = fakeEngine();
      const error = new EngineError("nested draw");
      engine.generateBoolean.mockImplementation(() => {
        throw error;
      });
      expect(() => generateValue(engine, context, testCase, schema)).toThrow(error);
      expect(engine.stopSpan).toHaveBeenCalledTimes(engine.startSpan.mock.calls.length);
      expect(engine.freeCollection).toHaveBeenCalledTimes(engine.newCollection.mock.calls.length);
    });
  }

  it.each([compoundSchemas[2], compoundSchemas[3]])(
    "closes $type span when collection allocation fails",
    (schema) => {
      const engine = fakeEngine();
      engine.newCollection.mockImplementation(() => {
        throw new EngineError("collection allocation");
      });
      expect(() => generateValue(engine, context, testCase, schema)).toThrow(
        "collection allocation",
      );
      expect(engine.stopSpan).toHaveBeenCalledTimes(1);
      expect(engine.freeCollection).not.toHaveBeenCalled();
    },
  );

  it("never lets cleanup control flow mask an engine fault", () => {
    const engine = fakeEngine();
    const fault = new EngineError("engine fault");
    engine.generateBoolean.mockImplementation(() => {
      throw fault;
    });
    engine.stopSpan.mockImplementation(() => {
      throw new StopTestError();
    });
    expect(() => generateValue(engine, context, testCase, compoundSchemas[0])).toThrow(fault);
  });

  it("preserves ordinary control flow when cleanup also stops", () => {
    const engine = fakeEngine();
    engine.generateBoolean.mockImplementation(() => {
      throw new AssumeError();
    });
    engine.stopSpan.mockImplementation(() => {
      throw new StopTestError();
    });
    expect(() => generateValue(engine, context, testCase, compoundSchemas[0])).toThrow(AssumeError);
  });

  it("reports both failures when nested generation and cleanup fail", () => {
    const engine = fakeEngine();
    engine.generateBoolean.mockImplementation(() => {
      throw new EngineError("draw");
    });
    engine.stopSpan.mockImplementation(() => {
      throw new EngineError("cleanup");
    });
    expect(() => generateValue(engine, context, testCase, compoundSchemas[0])).toThrow(
      "Generation cleanup failed",
    );
  });

  it("reports cleanup failure after a successful draw", () => {
    const engine = fakeEngine();
    engine.stopSpan.mockImplementation(() => {
      throw new EngineError("cleanup");
    });
    expect(() => generateValue(engine, context, testCase, compoundSchemas[0])).toThrow("cleanup");
  });
});

describe("StringGeneratorCache ownership", () => {
  it("reuses schema identity only within its engine and disposes exactly once", () => {
    const first = fakeEngine();
    const second = fakeEngine();
    const firstCache = new StringGeneratorCache(first);
    const secondCache = new StringGeneratorCache(second);
    const schema = { type: "email" };
    generateValue(first, context, testCase, schema, firstCache);
    generateValue(first, context, testCase, schema, firstCache);
    generateValue(second, context, testCase, schema, secondCache);
    expect(first.stringGeneratorEmail).toHaveBeenCalledTimes(1);
    expect(second.stringGeneratorEmail).toHaveBeenCalledTimes(1);
    expect(() => firstCache.get(second, context, schema)).toThrow("another engine");
    firstCache.dispose();
    firstCache.dispose();
    secondCache.dispose();
    expect(first.freeStringGenerator).toHaveBeenCalledTimes(1);
    expect(second.freeStringGenerator).toHaveBeenCalledTimes(1);
  });

  it("bounds live cached generators and frees evictions before constructing replacements", () => {
    const engine = fakeEngine();
    const cache = new StringGeneratorCache(engine);
    const oldest = { type: "email" };
    cache.get(engine, context, oldest);
    for (let i = 0; i < 256; i++) cache.get(engine, context, { type: "email" });
    expect(engine.freeStringGenerator).toHaveBeenCalledTimes(1);
    cache.get(engine, context, oldest);
    expect(engine.stringGeneratorEmail).toHaveBeenCalledTimes(258);
    cache.dispose();
    expect(engine.freeStringGenerator).toHaveBeenCalledTimes(258);
  });

  it("disposes a temporary draw cache even when generation throws", () => {
    const engine = fakeEngine();
    engine.generateString.mockImplementation(() => {
      throw new EngineError("string draw");
    });
    expect(() => generateValue(engine, context, testCase, { type: "email" })).toThrow(
      "string draw",
    );
    expect(engine.freeStringGenerator).toHaveBeenCalledTimes(1);
  });

  it("tries every destructor after an eviction/free error", () => {
    const engine = fakeEngine();
    const cache = new StringGeneratorCache(engine);
    cache.get(engine, context, { type: "email" });
    cache.get(engine, context, { type: "url" });
    engine.freeStringGenerator.mockImplementationOnce(() => {
      throw new EngineError("free failed");
    });
    expect(() => cache.dispose()).toThrow("free failed");
    expect(engine.freeStringGenerator).toHaveBeenCalledTimes(2);
    cache.dispose();
    expect(engine.freeStringGenerator).toHaveBeenCalledTimes(2);
  });

  it("keeps null categories distinct from a present empty list and uses portable UTF-8", () => {
    const engine = fakeEngine();
    generateValue(engine, context, testCase, {
      type: "string",
      min_size: 0,
      categories: [],
      include_characters: "a\0😀",
      exclude_characters: "",
    });
    expect(engine.stringGeneratorText.mock.calls[0][1]).toMatchObject({
      categories: [],
      excludeCategories: null,
      maxSize: 18446744073709551614n,
      includeCharacters: new Uint8Array([97, 0, 240, 159, 152, 128]),
      excludeCharacters: new Uint8Array(0),
    });
  });
});

it("StringGeneratorCache reports multiple failed destructors without retrying handles", () => {
  const engine = fakeEngine();
  const cache = new StringGeneratorCache(engine);
  cache.get(engine, context, { type: "email" });
  cache.get(engine, context, { type: "url" });
  engine.freeStringGenerator.mockImplementation(() => {
    throw new EngineError("free failed");
  });
  expect(() => cache.dispose()).toThrow(AggregateError);
  expect(engine.freeStringGenerator).toHaveBeenCalledTimes(2);
  cache.dispose();
  expect(engine.freeStringGenerator).toHaveBeenCalledTimes(2);
});

it("StringGeneratorCache compiles a grammar once and frees it on dispose, even after a failed free", () => {
  const engine = fakeEngine();
  const cache = new StringGeneratorCache(engine);
  const spec = {
    text: "start: B",
    start: null,
    maxSize: 0,
    maxDepth: 0,
    alphabet: null,
    explicitNames: [],
    importPath: null,
  };
  expect(cache.borrowGrammar(context, spec)).toBe(cache.borrowGrammar(context, spec));
  cache.releaseGrammar(spec);
  cache.releaseGrammar(spec);
  expect(engine.newGrammar).toHaveBeenCalledTimes(1);
  expect(engine.freeGrammarBuilder).toHaveBeenCalledTimes(1);
  cache.get(engine, context, { type: "email" });
  engine.freeGrammar.mockImplementation(() => {
    throw new EngineError("grammar free failed");
  });
  expect(() => cache.dispose()).toThrow("grammar free failed");
  expect(engine.freeStringGenerator).toHaveBeenCalledTimes(1);
  cache.dispose();
  expect(engine.freeGrammar).toHaveBeenCalledTimes(1);
});

describe.each([
  ["native", () => getLibhegel() as Engine],
  ["wasm", () => wasmFixture().engine as Engine],
])("StringGeneratorCache grammars on the %s engine", (_name, engine) => {
  const spec = (n: number): GrammarSpec => ({
    text: `start: "${n}"`,
    start: null,
    maxSize: 0,
    maxDepth: 0,
    alphabet: null,
    explicitNames: [],
    importPath: null,
  });

  it("bounds live grammars, never evicting a borrowed one", () => {
    const lib = engine();
    const freeGrammar = vi.spyOn(lib, "freeGrammar");
    const ctx = lib.newContext();
    const cache = new StringGeneratorCache(lib);
    const outer = spec(0);
    const borrowed = cache.borrowGrammar(ctx, outer);
    const specs = Array.from({ length: 256 }, (_, i) => spec(i + 1));
    for (const inner of specs) {
      cache.borrowGrammar(ctx, inner);
      cache.releaseGrammar(inner);
    }
    expect(freeGrammar.mock.calls).toEqual([[expect.anything()]]);
    expect(freeGrammar.mock.calls[0][0]).not.toBe(borrowed);
    expect(cache.borrowGrammar(ctx, outer)).toBe(borrowed);
    cache.releaseGrammar(outer);
    cache.releaseGrammar(outer);
    cache.borrowGrammar(ctx, specs[0]);
    expect(freeGrammar.mock.calls[1][0]).toBe(borrowed);
    cache.dispose();
    expect(freeGrammar).toHaveBeenCalledTimes(258);
    lib.freeContext(ctx);
  });
});
