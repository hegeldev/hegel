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
  type StateMachineHandle,
  type DateOptionsHandle,
  type TimeZoneSpecHandle,
  type ZonedSpecHandle,
  type SpanSpecHandle,
} from "../src/engine.js";
import { Database, Verbosity } from "../src/runnerCore.js";
import type { RuntimeServices } from "../src/runtime.js";

export const context = {} as ContextHandle;
export const settings = {} as SettingsHandle;
export const run = {} as RunHandle;
export const result = {} as RunResultHandle;
export const testCase = {} as TestCaseHandle;
export const replayCase = {} as TestCaseHandle;
export const failure = {} as FailureHandle;
export const collection = {} as CollectionHandle;
export const stringGenerator = {} as StringGeneratorHandle;
export const pool = {} as PoolHandle;
export const stateMachine = {} as StateMachineHandle;
export const dateOptions = {} as DateOptionsHandle;
export const timeZoneSpec = {} as TimeZoneSpecHandle;
export const zonedSpec = {} as ZonedSpecHandle;
export const spanSpec = {} as SpanSpecHandle;

const epoch2000 = { second: 946_684_800n, nanosecond: 0 };

/** Deterministic Engine double; no ABI pointers or generated engine logic. */
export function fakeEngine() {
  return {
    version: vi.fn<Engine["version"]>(() => "0.42.4"),
    newContext: vi.fn<Engine["newContext"]>(() => context),
    freeContext: vi.fn<Engine["freeContext"]>(),
    lastError: vi.fn<Engine["lastError"]>(() => "diagnostic"),
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
    runStart: vi.fn<Engine["runStart"]>(() => run),
    nextTestCase: vi
      .fn<Engine["nextTestCase"]>()
      .mockReturnValueOnce(testCase)
      .mockReturnValue(null),
    runResult: vi.fn<Engine["runResult"]>(() => result),
    freeRunResult: vi.fn<Engine["freeRunResult"]>(),
    freeRun: vi.fn<Engine["freeRun"]>(),
    testCaseFromBlob: vi.fn<Engine["testCaseFromBlob"]>(() => replayCase),
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
    dateOptionsNew: vi.fn<Engine["dateOptionsNew"]>(() => dateOptions),
    dateOptionsSetWeekdays: vi.fn<Engine["dateOptionsSetWeekdays"]>(),
    dateOptionsSetMonths: vi.fn<Engine["dateOptionsSetMonths"]>(),
    freeDateOptions: vi.fn<Engine["freeDateOptions"]>(),
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
    generateTimestamp: vi.fn<Engine["generateTimestamp"]>(() => epoch2000),
    generateOffset: vi.fn<Engine["generateOffset"]>(() => ({ seconds: 0, known: true })),
    timezoneInfo: vi.fn<Engine["timezoneInfo"]>(() => ({
      known: true,
      primaryName: "UTC",
      isLink: false,
      rank: 0,
      tags: 0,
    })),
    timezoneSpecNew: vi.fn<Engine["timezoneSpecNew"]>(() => timeZoneSpec),
    timezoneSpecSetNames: vi.fn<Engine["timezoneSpecSetNames"]>(),
    freeTimezoneSpec: vi.fn<Engine["freeTimezoneSpec"]>(),
    generateTimezone: vi.fn<Engine["generateTimezone"]>(() => "UTC"),
    zonedSpecNew: vi.fn<Engine["zonedSpecNew"]>(() => zonedSpec),
    zonedSpecSetCivilBounds: vi.fn<Engine["zonedSpecSetCivilBounds"]>(),
    zonedSpecSetInstantBounds: vi.fn<Engine["zonedSpecSetInstantBounds"]>(),
    zonedSpecSetPrecision: vi.fn<Engine["zonedSpecSetPrecision"]>(),
    zonedSpecSetImaginary: vi.fn<Engine["zonedSpecSetImaginary"]>(),
    zonedSpecSetAmbiguous: vi.fn<Engine["zonedSpecSetAmbiguous"]>(),
    freeZonedSpec: vi.fn<Engine["freeZonedSpec"]>(),
    generateZonedDatetime: vi.fn<Engine["generateZonedDatetime"]>(() => ({
      value: {
        civil: {
          date: { year: 2000, month: 1, day: 1 },
          time: { hour: 0, minute: 0, second: 0, nanosecond: 0 },
        },
        offsetSeconds: 0,
        offsetKnown: true,
        fold: false,
        kind: 0,
        offsetBefore: 0,
        offsetAfter: 0,
        timestamp: epoch2000,
      },
      zone: "UTC",
    })),
    spanSpecNew: vi.fn<Engine["spanSpecNew"]>(() => spanSpec),
    spanSpecSetUnitBounds: vi.fn<Engine["spanSpecSetUnitBounds"]>(),
    spanSpecSetSign: vi.fn<Engine["spanSpecSetSign"]>(),
    freeSpanSpec: vi.fn<Engine["freeSpanSpec"]>(),
    generateSpan: vi.fn<Engine["generateSpan"]>(() => ({
      years: 0n,
      months: 0n,
      weeks: 0n,
      days: 0n,
      hours: 0n,
      minutes: 0n,
      seconds: 0n,
      milliseconds: 0n,
      microseconds: 0n,
      nanoseconds: 0n,
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
    freePool: vi.fn<Engine["freePool"]>(),
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
    failureCount: vi.fn<Engine["failureCount"]>(() => 1),
    failure: vi.fn<Engine["failure"]>(() => failure),
    freeFailure: vi.fn<Engine["freeFailure"]>(),
    failureOrigin: vi.fn<Engine["failureOrigin"]>(() => "property.ts:1"),
    reproductionBlob: vi.fn<Engine["reproductionBlob"]>(() => "blob"),
    freeStringGenerator: vi.fn<Engine["freeStringGenerator"]>(),
  } satisfies Engine;
}

export function fakeRuntime(engine: Engine): RuntimeServices {
  return {
    getEngine: () => engine,
    defaultSettings: () => ({
      testCases: 100,
      seed: null,
      verbosity: Verbosity.Normal,
      derandomize: true,
      database: Database.disabled,
      suppressHealthCheck: [],
      reportMultipleFailures: false,
    }),
    validateSettings: vi.fn(),
    reportFinalValue: vi.fn(),
    reportFinalError: vi.fn(),
    note: vi.fn(),
    emitAntithesisAssertion: vi.fn(),
  };
}
