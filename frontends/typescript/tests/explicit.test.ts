import { expect, it } from "vitest";
import { ExplicitDataSource } from "../src/explicit.js";

it("ExplicitDataSource refuses every engine operation and records only notes", () => {
  const ds = new ExplicitDataSource();
  const refused: [string, () => unknown][] = [
    ["Drawing from a generator's engine primitives", () => ds.generate()],
    ["Labelling a span", () => ds.label()],
    ["Opening a span", () => ds.startSpan()],
    ["Closing a span", () => ds.stopSpan()],
    ["A collection", () => ds.newCollection()],
    ["A collection", () => ds.collectionMore()],
    ["A collection", () => ds.collectionReject()],
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
  ];
  for (const [operation, call] of refused) {
    expect(call).toThrow(`${operation} is not supported in explicit examples`);
  }
  ds.markComplete();
  ds.target();
  ds.event();
  ds.eventValue();
  ds.setWorker();
  expect(ds.clone()).toBe(ds);
  expect(ds.block()).toBe(ds);
  ds.note("a note");
  ds.printer().text("text");
  ds.printer().hardBreak();
  expect(ds.lines()).toEqual(["a note", "text"]);
});
