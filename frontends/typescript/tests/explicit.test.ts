import { describe, expect, it } from "vitest";
import { ExplicitDataSource } from "../src/explicit.js";
import { EngineDocument } from "../src/pretty.js";
import { ExplicitExampleError } from "../src/testCase.js";
import { getLibhegel } from "../src/session.js";
import type { Engine } from "../src/engine.js";
import { wasmFixture } from "./wasmFixture.js";

describe.each([
  ["native", () => getLibhegel() as Engine],
  ["wasm", () => wasmFixture().engine as Engine],
])("ExplicitDataSource on the %s engine", (_name, engine) => {
  it("refuses every engine operation and lays out its draws and notes", () => {
    const lib = engine();
    const ctx = lib.newContext();
    const document = new EngineDocument(lib, ctx);
    const ds = new ExplicitDataSource(document);
    const refused: [string, () => unknown][] = [
      ["Drawing from a generator's engine primitives", () => ds.generate()],
      ["Labelling a span", () => ds.label()],
      ["Opening a span", () => ds.startSpan()],
      ["Closing a span", () => ds.stopSpan()],
      ["A collection", () => ds.newCollection()],
      ["A collection", () => ds.collectionMore()],
      ["A collection", () => ds.collectionReject()],
      ["A repeat loop", () => ds.newRepeat()],
      ["A repeat loop", () => ds.repeatMore()],
      ["A repeat loop", () => ds.repeatFailed()],
      ["A stateful.Pool", () => ds.newPool()],
      ["A stateful.Pool", () => ds.poolAdd()],
      ["A stateful.Pool", () => ds.poolGenerate()],
      ["A stateful.Pool", () => ds.poolRestore()],
      ["A recursive generator", () => ds.newRecursion()],
      ["A recursive generator", () => ds.recursionBranch()],
      ["A recursive generator", () => ds.recursionLeaf()],
      ["A recursive generator", () => ds.recursionRetry()],
      ["A recursive generator", () => ds.recursionFinish()],
      ["A state machine", () => ds.newStateMachine()],
      ["A state machine", () => ds.stateMachineNextRound()],
      ["A state machine", () => ds.stateMachineNextRule()],
      ["A state machine", () => ds.stateMachineRuleRejected()],
      ["A state machine", () => ds.stateMachineShouldCheckInvariant()],
      ["A grammar", () => ds.drawGrammar()],
    ];
    for (const [operation, call] of refused) {
      expect(call).toThrow(
        new ExplicitExampleError(`${operation} is not supported in explicit test cases`),
      );
    }
    ds.markComplete();
    ds.target();
    ds.event();
    ds.eventValue();
    ds.setWorker();
    expect(ds.clone()).toBe(ds);
    const block = ds.block(2);
    expect(block.family).toBe(ds.family);
    block.note("first");
    ds.note("a note");
    ds.printer().text("text");
    ds.printer().hardBreak();
    block.note("indented\nnote");
    block.printer().text("indented ");
    block.printer().reflow("text");
    block.printer().hardBreak();
    const printer = block.printer();
    printer.beginSpeculative();
    printer.text("const held = ");
    block.note("held");
    printer.beginGroup(4, "[");
    printer.breakable("");
    printer.text("x".repeat(80));
    printer.ifBreak(",");
    printer.shiftIndent(-4);
    printer.breakable("");
    printer.shiftIndent(4);
    printer.endGroup("]");
    printer.comment("  // c");
    printer.hardBreak();
    printer.commitSpeculative();
    printer.beginSpeculative();
    printer.text("dropped");
    printer.hardBreak();
    ds.note("dropped too");
    printer.abortSpeculative();
    ds.printer().text("last");
    expect(document.lines()).toEqual([
      "  first",
      "a note",
      "text",
      "  indented",
      "  note",
      "  indented text",
      "  const held = [",
      `      ${"x".repeat(80)},`,
      "  ]  // c",
      "  held",
      "dropped too",
      "last",
    ]);
    document.dispose();
    lib.freeContext(ctx);
  });
});
