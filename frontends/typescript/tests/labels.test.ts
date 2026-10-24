import { describe, expect, it } from "vitest";
import * as gs from "../src/generators/index.js";
import { Generator } from "../src/generators/core.js";
import { labelSpec, LabelNames, resolveLabel, type LabelSpec } from "../src/labels.js";
import { fakeEngine, context } from "./fakeEngine.js";

function names(spec: LabelSpec): unknown {
  return spec.parts.length === 0 ? spec.name : [spec.name, ...spec.parts.map(names)];
}

describe("generator labels", () => {
  it("name a generator's kind and its components", () => {
    expect(names(gs.arrays(gs.integers()).label)).toEqual(["hegel.lists", "hegel.integer"]);
    expect(names(gs.maps(gs.text(), gs.booleans()).label)).toEqual([
      "hegel.dicts",
      "hegel.string",
      "hegel.boolean",
    ]);
    expect(names(gs.tuples(gs.integers(), gs.booleans()).label)).toEqual([
      "hegel.tuples",
      "hegel.integer",
      "hegel.boolean",
    ]);
    expect(names(gs.composite((tc) => tc.draw(gs.integers())).label)).toBe(LabelNames.COMPOSITE);
  });

  it("label a generator of no known kind as a plain generator", () => {
    class Custom extends Generator<number> {
      doDraw(): number {
        return 1;
      }
    }
    const custom = new Custom();
    expect(custom.label).toEqual(labelSpec(LabelNames.GENERATOR));
    expect(custom.label).toBe(custom.label);
  });
});

describe("resolveLabel", () => {
  it("asks the engine once per spec, combining a spec's own label with its parts'", () => {
    const engine = fakeEngine();
    const leaf = labelSpec("leaf");
    const spec = labelSpec("node", leaf, leaf);
    const label = resolveLabel(engine, context, spec);
    expect(resolveLabel(engine, context, spec)).toBe(label);
    expect(engine.labelFromName).toHaveBeenCalledTimes(2);
    expect(engine.labelCombine).toHaveBeenCalledExactlyOnceWith(context, [4n, 4n, 4n]);
    expect(resolveLabel(fakeEngine(), context, spec)).toBe(label);
  });
});
