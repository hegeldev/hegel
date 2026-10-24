import { vi } from "vitest";
import {
  RunStatus,
  type Engine,
  type ContextHandle,
  type SettingsHandle,
  type RunHandle,
  type RunResultHandle,
  type TestCaseHandle,
  type FailureHandle,
  type CollectionHandle,
  type StringGeneratorHandle,
  type PoolHandle,
  type RecursionHandle,
  type StateMachineHandle,
  type PrinterHandle,
  type GrammarBuilderHandle,
  type GrammarHandle,
  type GrammarDrawHandle,
} from "../src/engine.js";
import type { RuntimeServices } from "../src/runtime.js";

export const context = {} as ContextHandle;
export const settings = {} as SettingsHandle;
export const run = {} as RunHandle;
export const result = {} as RunResultHandle;
export const testCase = {} as TestCaseHandle;
export const failure = {} as FailureHandle;
export const collection = {} as CollectionHandle;
export const stringGenerator = {} as StringGeneratorHandle;
export const pool = {} as PoolHandle;
export const recursion = {} as RecursionHandle;
export const stateMachine = {} as StateMachineHandle;
export const derivedCase = {} as TestCaseHandle;
export const grammarBuilder = {} as GrammarBuilderHandle;
export const grammar = {} as GrammarHandle;
export const grammarDraw = {} as GrammarDrawHandle;

/** A test case's document double: its text, with speculative regions kept or dropped. */
export class FakePrinter {
  regions: string[] = [""];
  get handle(): PrinterHandle {
    return this as unknown as PrinterHandle;
  }
  push(text: string): void {
    this.regions[this.regions.length - 1] += text;
  }
}

function printerOf(p: PrinterHandle): FakePrinter {
  return p as unknown as FakePrinter;
}

/** Deterministic Engine double; no ABI pointers or generated engine logic. */
export function fakeEngine() {
  const printers = new WeakMap<TestCaseHandle, FakePrinter>();
  return {
    version: vi.fn<Engine["version"]>(() => "0.42.4"),
    newContext: vi.fn<Engine["newContext"]>(() => context),
    freeContext: vi.fn<Engine["freeContext"]>(),
    lastError: vi.fn<Engine["lastError"]>(() => "diagnostic"),
    setUnicodeVersion: vi.fn<Engine["setUnicodeVersion"]>(),
    newSettings: vi.fn<Engine["newSettings"]>(() => settings),
    freeSettings: vi.fn<Engine["freeSettings"]>(),
    setTestCases: vi.fn<Engine["setTestCases"]>(),
    setVerbosity: vi.fn<Engine["setVerbosity"]>(),
    setSeed: vi.fn<Engine["setSeed"]>(),
    setDerandomize: vi.fn<Engine["setDerandomize"]>(),
    setDatabase: vi.fn<Engine["setDatabase"]>(),
    setDatabaseKey: vi.fn<Engine["setDatabaseKey"]>(),
    setSuppressHealthCheck: vi.fn<Engine["setSuppressHealthCheck"]>(),
    setReportMultipleFailures: vi.fn<Engine["setReportMultipleFailures"]>(),
    newSettingsForProfile: vi.fn<Engine["newSettingsForProfile"]>(() => settings),
    setStopOnFirstFailure: vi.fn<Engine["setStopOnFirstFailure"]>(),
    setShowStatistics: vi.fn<Engine["setShowStatistics"]>(),
    setUnboundedChoices: vi.fn<Engine["setUnboundedChoices"]>(),
    setPrintBlob: vi.fn<Engine["setPrintBlob"]>(),
    setPhases: vi.fn<Engine["setPhases"]>(),
    setBackend: vi.fn<Engine["setBackend"]>(),
    setNondeterminismStrictness: vi.fn<Engine["setNondeterminismStrictness"]>(),
    setTestLocation: vi.fn<Engine["setTestLocation"]>(),
    registerProfile: vi.fn<Engine["registerProfile"]>(),
    setDefaultProfile: vi.fn<Engine["setDefaultProfile"]>(),
    runStart: vi.fn<Engine["runStart"]>(() => run),
    nextTestCase: vi
      .fn<Engine["nextTestCase"]>()
      .mockReturnValueOnce(testCase)
      .mockReturnValue(null),
    runResult: vi.fn<Engine["runResult"]>(() => result),
    freeRunResult: vi.fn<Engine["freeRunResult"]>(),
    freeRun: vi.fn<Engine["freeRun"]>(),
    runStartBlob: vi.fn<Engine["runStartBlob"]>(() => run),
    getVerbosity: vi.fn<Engine["getVerbosity"]>(() => 0),
    getPhases: vi.fn<Engine["getPhases"]>(() => 0b11111),
    getPrintBlob: vi.fn<Engine["getPrintBlob"]>(() => false),
    getReportMultipleFailures: vi.fn<Engine["getReportMultipleFailures"]>(() => true),
    shouldCapture: vi.fn<Engine["shouldCapture"]>(() => false),
    cloneTestCase: vi.fn<Engine["cloneTestCase"]>(() => derivedCase),
    blockTestCase: vi.fn<Engine["blockTestCase"]>(() => derivedCase),
    setWorker: vi.fn<Engine["setWorker"]>(),
    note: vi.fn<Engine["note"]>(),
    target: vi.fn<Engine["target"]>(),
    event: vi.fn<Engine["event"]>(),
    eventValue: vi.fn<Engine["eventValue"]>(),
    labelFromName: vi.fn<Engine["labelFromName"]>((_ctx, name) => BigInt(name.length)),
    labelCombine: vi.fn<Engine["labelCombine"]>((_ctx, labels) =>
      labels.reduce((a, b) => a * 31n + b, 0n),
    ),
    testCasePrinter: vi.fn<Engine["testCasePrinter"]>((_ctx, tc) => {
      if (!printers.has(tc)) printers.set(tc, new FakePrinter());
      return printers.get(tc)!.handle;
    }),
    freePrinter: vi.fn<Engine["freePrinter"]>(),
    printerText: vi.fn<Engine["printerText"]>((_ctx, p, text) => printerOf(p).push(text)),
    printerHardBreak: vi.fn<Engine["printerHardBreak"]>((_ctx, p) => printerOf(p).push("\n")),
    printerReflow: vi.fn<Engine["printerReflow"]>((_ctx, p, text) => printerOf(p).push(text)),
    printerBeginGroup: vi.fn<Engine["printerBeginGroup"]>((_ctx, p, _indent, open) =>
      printerOf(p).push(open),
    ),
    printerEndGroup: vi.fn<Engine["printerEndGroup"]>((_ctx, p, close) => printerOf(p).push(close)),
    printerBreakable: vi.fn<Engine["printerBreakable"]>((_ctx, p, sep) => printerOf(p).push(sep)),
    printerIfBreak: vi.fn<Engine["printerIfBreak"]>(),
    printerComment: vi.fn<Engine["printerComment"]>((_ctx, p, text) => printerOf(p).push(text)),
    printerShiftIndent: vi.fn<Engine["printerShiftIndent"]>(),
    printerBeginSpeculative: vi.fn<Engine["printerBeginSpeculative"]>((_ctx, p) => {
      printerOf(p).regions.push("");
    }),
    printerCommitSpeculative: vi.fn<Engine["printerCommitSpeculative"]>((_ctx, p) => {
      printerOf(p).push(printerOf(p).regions.pop()!);
    }),
    printerAbortSpeculative: vi.fn<Engine["printerAbortSpeculative"]>((_ctx, p) => {
      printerOf(p).regions.pop();
    }),
    printerResolve: vi.fn<Engine["printerResolve"]>(),
    printerValue: vi.fn<Engine["printerValue"]>((_ctx, p) => printerOf(p).regions[0]),
    freeTestCase: vi.fn<Engine["freeTestCase"]>(),
    generateBoolean: vi.fn<Engine["generateBoolean"]>(() => true),
    generateInteger: vi.fn<Engine["generateInteger"]>(() => 0n),
    generateIntegerBig: vi.fn<Engine["generateIntegerBig"]>(() => 0n),
    generateFloat: vi.fn<Engine["generateFloat"]>(() => 1),
    generateBytes: vi.fn<Engine["generateBytes"]>(() => new Uint8Array([0, 255])),
    stringGeneratorText: vi.fn<Engine["stringGeneratorText"]>(() => stringGenerator),
    stringGeneratorRegex: vi.fn<Engine["stringGeneratorRegex"]>(() => stringGenerator),
    stringGeneratorEmail: vi.fn<Engine["stringGeneratorEmail"]>(() => stringGenerator),
    stringGeneratorUrl: vi.fn<Engine["stringGeneratorUrl"]>(() => stringGenerator),
    stringGeneratorDomain: vi.fn<Engine["stringGeneratorDomain"]>(() => stringGenerator),
    generateString: vi.fn<Engine["generateString"]>(() => "value"),
    generateDate: vi.fn<Engine["generateDate"]>(() => ({ year: 2024, month: 2, day: 29 })),
    generateTime: vi.fn<Engine["generateTime"]>(() => ({
      hour: 1,
      minute: 2,
      second: 3,
      nanosecond: 1,
    })),
    generateDatetime: vi.fn<Engine["generateDatetime"]>(() => ({
      date: { year: 2024, month: 2, day: 29 },
      time: { hour: 1, minute: 2, second: 3, nanosecond: 1 },
    })),
    generateUuid: vi.fn<Engine["generateUuid"]>(() =>
      Uint8Array.from([
        0xa7, 0x0f, 0x44, 0x6c, 0x05, 0xe3, 0x42, 0xa9, 0xa3, 0x1b, 0xf0, 0xd0, 0x54, 0x5d, 0x63,
        0x16,
      ]),
    ),
    generateIpv4: vi.fn<Engine["generateIpv4"]>(() => new Uint8Array([127, 0, 0, 1])),
    generateIpv6: vi.fn<Engine["generateIpv6"]>(() => new Uint8Array(16)),
    startSpan: vi.fn<Engine["startSpan"]>(),
    stopSpan: vi.fn<Engine["stopSpan"]>(),
    newCollection: vi.fn<Engine["newCollection"]>(() => collection),
    collectionMore: vi
      .fn<Engine["collectionMore"]>()
      .mockReturnValueOnce(true)
      .mockReturnValue(false),
    collectionReject: vi.fn<Engine["collectionReject"]>(),
    freeCollection: vi.fn<Engine["freeCollection"]>(),
    markComplete: vi.fn<Engine["markComplete"]>(),
    newPool: vi.fn<Engine["newPool"]>(() => pool),
    poolAdd: vi.fn<Engine["poolAdd"]>(() => 0n),
    poolGenerate: vi.fn<Engine["poolGenerate"]>(() => 0n),
    poolRestore: vi.fn<Engine["poolRestore"]>(),
    freePool: vi.fn<Engine["freePool"]>(),
    newRecursion: vi.fn<Engine["newRecursion"]>(() => recursion),
    recursionBranch: vi.fn<Engine["recursionBranch"]>(() => false),
    recursionLeaf: vi.fn<Engine["recursionLeaf"]>(() => true),
    recursionRetry: vi.fn<Engine["recursionRetry"]>(),
    recursionFinish: vi.fn<Engine["recursionFinish"]>(() => true),
    freeRecursion: vi.fn<Engine["freeRecursion"]>(),
    newStateMachine: vi.fn<Engine["newStateMachine"]>(() => stateMachine),
    stateMachineNextGroup: vi
      .fn<Engine["stateMachineNextGroup"]>()
      .mockReturnValueOnce(0)
      .mockReturnValue(null),
    stateMachineNextRule: vi
      .fn<Engine["stateMachineNextRule"]>()
      .mockReturnValueOnce(0)
      .mockReturnValue(null),
    stateMachineRuleRejected: vi.fn<Engine["stateMachineRuleRejected"]>(),
    stateMachineShouldCheckInvariant: vi.fn<Engine["stateMachineShouldCheckInvariant"]>(() => true),
    freeStateMachine: vi.fn<Engine["freeStateMachine"]>(),
    runStatus: vi.fn<Engine["runStatus"]>(() => RunStatus.PASSED),
    runError: vi.fn<Engine["runError"]>(() => "backend error"),
    runErrorKind: vi.fn<Engine["runErrorKind"]>(() => ({ kind: 6, healthCheck: 0 })),
    failureCount: vi.fn<Engine["failureCount"]>(() => 1),
    failure: vi.fn<Engine["failure"]>(() => failure),
    freeFailure: vi.fn<Engine["freeFailure"]>(),
    failureOrigin: vi.fn<Engine["failureOrigin"]>(() => "property.ts:1"),
    reproductionBlob: vi.fn<Engine["reproductionBlob"]>(() => "blob"),
    failureCaveat: vi.fn<Engine["failureCaveat"]>(() => null),
    freeStringGenerator: vi.fn<Engine["freeStringGenerator"]>(),
    grammarBuilderNew: vi.fn<Engine["grammarBuilderNew"]>(() => grammarBuilder),
    grammarBuilderFile: vi.fn<Engine["grammarBuilderFile"]>(() => null),
    grammarBuilderProvide: vi.fn<Engine["grammarBuilderProvide"]>(),
    freeGrammarBuilder: vi.fn<Engine["freeGrammarBuilder"]>(),
    newGrammar: vi.fn<Engine["newGrammar"]>(() => grammar),
    freeGrammar: vi.fn<Engine["freeGrammar"]>(),
    grammarDrawNew: vi.fn<Engine["grammarDrawNew"]>(() => grammarDraw),
    grammarDrawNext: vi.fn<Engine["grammarDrawNext"]>(() => null),
    grammarDrawProvide: vi.fn<Engine["grammarDrawProvide"]>(),
    grammarDrawResult: vi.fn<Engine["grammarDrawResult"]>(() => ""),
    freeGrammarDraw: vi.fn<Engine["freeGrammarDraw"]>(),
  } satisfies Engine;
}

export function fakeRuntime(engine: Engine): RuntimeServices {
  return {
    getEngine: () => engine,
    validateSettings: vi.fn(),
    persistence: true,
    projectRoot: "",
    output: vi.fn(),
  };
}
