import { describe, it, expect, vi } from "vitest";
import koffi, { type LibraryHandle } from "koffi";
import {
  Libhegel,
  bindLibrary,
  LibhegelError,
  Status,
  RunStatus,
  NativeVerbosity,
  fitsInt64,
  bigIntToTwosComplementLE,
  twosComplementLEToBigInt,
  type Bindings,
  type Ptr,
} from "../src/libhegel.js";
import { AssumeError, StopTestError } from "../src/testCase.js";
import { LIBHEGEL_VERSION } from "../src/libhegel-version.js";
import { testLibPath } from "./libPath.js";

// ---------------------------------------------------------------------------
// Unit tests for the integer wire-format helpers.
// ---------------------------------------------------------------------------

describe("integer wire-format helpers", () => {
  it("fitsInt64 accepts the int64 range and rejects anything wider", () => {
    expect(fitsInt64(-(2n ** 63n), 2n ** 63n - 1n)).toBe(true);
    expect(fitsInt64(-(2n ** 63n) - 1n, 0n)).toBe(false);
    expect(fitsInt64(0n, 2n ** 63n)).toBe(false);
  });

  it("encodes minimal two's-complement little-endian bytes", () => {
    expect([...bigIntToTwosComplementLE(0n)]).toEqual([0x00]);
    expect([...bigIntToTwosComplementLE(127n)]).toEqual([0x7f]);
    // A non-negative value whose top bit would read negative gains a 0x00
    // sign byte.
    expect([...bigIntToTwosComplementLE(128n)]).toEqual([0x80, 0x00]);
    expect([...bigIntToTwosComplementLE(-1n)]).toEqual([0xff]);
    expect([...bigIntToTwosComplementLE(-128n)]).toEqual([0x80]);
    // A negative value whose top bit would read non-negative gains a 0xff
    // sign byte.
    expect([...bigIntToTwosComplementLE(-129n)]).toEqual([0x7f, 0xff]);
    expect([...bigIntToTwosComplementLE(-256n)]).toEqual([0x00, 0xff]);
  });

  it("decodes sign-filled buffers of any width", () => {
    expect(twosComplementLEToBigInt(Buffer.from([0x01, 0x00, 0x00]))).toBe(1n);
    expect(twosComplementLEToBigInt(Buffer.from([0xff, 0xff, 0xff]))).toBe(-1n);
  });

  it("round-trips values through encode/decode", () => {
    for (const v of [0n, 1n, -1n, 255n, -255n, 2n ** 127n - 1n, -(2n ** 127n), 10n ** 40n]) {
      expect(twosComplementLEToBigInt(bigIntToTwosComplementLE(v))).toBe(v);
    }
  });
});

// ---------------------------------------------------------------------------
// Fake-bindings unit tests for wrapper logic that the real engine cannot easily
// be driven into (NULL returns, specific result codes, null string getters).
// ---------------------------------------------------------------------------

function fakeBindings(overrides: Partial<Bindings>): Bindings {
  const noop = (): void => undefined;
  const base: Bindings = {
    contextNew: () => ({}) as Ptr,
    contextFree: noop,
    contextLastError: () => "",
    settingsNew: (_ctx, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    settingsFree: noop,
    settingsTestCases: noop,
    settingsVerbosity: noop,
    settingsSeed: noop,
    settingsDerandomize: noop,
    settingsDatabase: noop,
    settingsDatabaseKey: noop,
    settingsSuppressHealthCheck: noop,
    settingsReportMultipleFailures: noop,
    settingsNewForProfile: (_ctx, _name, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    settingsStopOnFirstFailure: noop,
    settingsShowStatistics: noop,
    settingsUnboundedChoices: noop,
    settingsPrintBlob: noop,
    settingsPhases: noop,
    settingsBackend: noop,
    settingsNondeterminismStrictness: noop,
    settingsTestLocation: noop,
    settingsRegisterProfile: noop,
    setDefaultProfile: noop,
    settingsGetVerbosity: () => 0,
    settingsGetPhases: () => 0,
    settingsGetPrintBlob: () => false,
    settingsGetReportMultipleFailures: () => false,
    settingsGetTestCases: () => 0,
    settingsGetSeed: () => null,
    settingsGetDerandomize: () => false,
    settingsGetDatabase: () => null,
    settingsGetSuppressHealthCheck: () => 0,
    settingsGetStopOnFirstFailure: () => false,
    settingsGetShowStatistics: () => false,
    settingsGetUnboundedChoices: () => false,
    settingsGetBackend: () => 0,
    settingsGetNondeterminismStrictness: () => 0,
    runStart: (_ctx, _settings, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    runStartBlob: (_ctx, _settings, _blob, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    nextTestCase: (_ctx, _run, out) => {
      out[0] = null;
      return 0;
    },
    runResult: (_ctx, _run, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    runResultFree: noop,
    runFree: noop,
    testCaseFree: noop,
    testCaseShouldCapture: () => 0,
    testCaseClone: (_ctx, _tc, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    testCaseBlock: (_ctx, _tc, _indent, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    testCaseSetWorker: () => 0,
    note: () => 0,
    target: () => 0,
    event: () => 0,
    eventValue: () => 0,
    labelFromName: () => 0,
    labelCombine: () => 0,
    testCasePrinter: (_ctx, _tc, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    printerOptionsNew: (_ctx, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    printerOptionsSetMaxWidth: () => 0,
    printerOptionsFree: noop,
    printerNew: (_ctx, _options, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    printerFree: noop,
    printerText: () => 0,
    printerReflow: () => 0,
    printerBeginGroup: () => 0,
    printerEndGroup: () => 0,
    printerBreakable: () => 0,
    printerIfBreak: () => 0,
    printerComment: () => 0,
    printerShiftIndent: () => 0,
    printerHardBreak: () => 0,
    printerBeginSpeculative: () => 0,
    printerCommitSpeculative: () => 0,
    printerAbortSpeculative: () => 0,
    printerResolve: () => 0,
    printerValue: () => 0,
    printerValueResultFree: noop,
    generateBoolean: () => 0,
    generateInteger: () => 0,
    generateIntegerBig: () => 0,
    generateFloat: () => 0,
    generateBytes: () => 0,
    generateBytesResultFree: noop,
    stringGeneratorFree: noop,
    stringGeneratorText: () => 0,
    stringGeneratorRegex: () => 0,
    stringGeneratorEmail: () => 0,
    stringGeneratorUrl: () => 0,
    stringGeneratorDomain: () => 0,
    generateString: () => 0,
    ownedStringFree: noop,
    grammarBuilderNew: () => 0,
    grammarBuilderFile: () => -11,
    grammarBuilderProvide: () => 0,
    grammarBuilderFree: noop,
    grammarNew: () => 0,
    grammarFree: noop,
    grammarDrawNew: () => 0,
    grammarDrawNext: () => -11,
    grammarDrawProvide: () => 0,
    grammarDrawResult: () => 0,
    grammarDrawFree: noop,
    generateDate: () => 0,
    generateTime: () => 0,
    generateDatetime: () => 0,
    generateUuid: () => 0,
    generateIpv4: () => 0,
    generateIpv6: () => 0,
    startSpan: () => 0,
    stopSpan: () => 0,
    newCollection: () => 0,
    collectionMore: () => 0,
    collectionReject: () => 0,
    collectionFree: noop,
    markComplete: () => 0,
    newPool: (_ctx, _tc, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    poolAdd: () => 0,
    poolGenerate: () => 0,
    poolRestore: () => 0,
    poolFree: noop,
    newRecursion: (_ctx, _tc, _maxDepth, _maxLeaves, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    recursionBranch: () => 0,
    recursionLeaf: () => 0,
    recursionRetry: () => 0,
    recursionFinish: () => 0,
    recursionFree: noop,
    newStateMachine: (_ctx, _tc, _opts, out) => {
      out[0] = {} as Ptr;
      return 0;
    },
    stateMachineNextGroup: () => 0,
    stateMachineNextRule: () => 0,
    stateMachineRuleRejected: () => 0,
    stateMachineShouldCheckInvariant: () => 0,
    stateMachineFree: noop,
    runResultStatus: () => RunStatus.PASSED,
    runResultError: () => null,
    runResultErrorKind: () => ({ kind: 0, healthCheck: 0 }),
    runResultFailureCount: () => 0,
    runResultFailure: () => ({}),
    failureFree: noop,
    failureOrigin: () => "",
    failureReproductionBlob: () => null,
    failureCaveat: () => null,
    version: () => "0.0.0",
    ...overrides,
  };
  return base;
}

describe("Libhegel wrapper logic (fake bindings)", () => {
  it("throws when run_start returns an error code", () => {
    const lib = new Libhegel(
      fakeBindings({
        runStart: (_ctx, _settings, out) => {
          out[0] = null;
          return -3;
        },
        contextLastError: () => "boom",
      }),
    );
    const h = lib.newContext();
    expect(() => lib.runStart(h, h)).toThrow(/hegel_run_start failed: boom/);
  });

  it("nextTestCase returns null at normal completion (OK code, NULL out)", () => {
    const lib = new Libhegel(
      fakeBindings({
        nextTestCase: (_ctx, _run, out) => {
          out[0] = null;
          return 0;
        },
      }),
    );
    const h = lib.newContext();
    expect(lib.nextTestCase(h, h)).toBeNull();
  });

  it("nextTestCase throws on a non-OK code (caller misuse)", () => {
    const lib = new Libhegel(
      fakeBindings({
        nextTestCase: (_ctx, _run, out) => {
          out[0] = null;
          return -7;
        },
        contextLastError: () => "not complete",
      }),
    );
    const h = lib.newContext();
    expect(() => lib.nextTestCase(h, h)).toThrow(/not complete/);
  });

  it("nextTestCase wraps the handle when non-null", () => {
    const handle = {} as Ptr;
    const testCaseFree = vi.fn();
    const lib = new Libhegel(
      fakeBindings({
        nextTestCase: (_ctx, _run, out) => {
          out[0] = handle;
          return 0;
        },
        testCaseFree,
      }),
    );
    const h = lib.newContext();
    const tc = lib.nextTestCase(h, h);
    expect(tc).not.toBe(handle);
    lib.freeTestCase(tc);
    expect(testCaseFree).toHaveBeenCalledExactlyOnceWith(handle);
  });

  it("throws when run_result returns an error code", () => {
    const lib = new Libhegel(
      fakeBindings({
        runResult: (_ctx, _run, out) => {
          out[0] = null;
          return -7;
        },
        contextLastError: () => "nope",
      }),
    );
    const h = lib.newContext();
    expect(() => lib.runResult(h, h)).toThrow(/hegel_run_result failed: nope/);
  });

  it("throws when run_start_blob returns an error code", () => {
    const lib = new Libhegel(
      fakeBindings({
        runStartBlob: (_ctx, _s, _blob, out) => {
          out[0] = null;
          return -5;
        },
        contextLastError: () => "bad blob",
      }),
    );
    const h = lib.newContext();
    expect(() => lib.runStartBlob(h, h, "x")).toThrow(/hegel_run_start_blob failed: bad blob/);
  });

  it("maps STOP_TEST to StopTestError", () => {
    const lib = new Libhegel(fakeBindings({ startSpan: () => -1 }));
    const h = lib.newContext();
    expect(() => lib.startSpan(h, h, 1n)).toThrow(StopTestError);
  });

  it("maps ASSUME to AssumeError", () => {
    // The engine can reject a draw internally (e.g. an email draw exceeding
    // the RFC length cap), but only on unlucky random draws — drive the code
    // deterministically instead.
    const lib = new Libhegel(fakeBindings({ generateString: () => -2 }));
    const h = lib.newContext();
    expect(() => lib.generateString(h, h, h)).toThrow(AssumeError);
  });

  it("maps RETRY from the recursion protocol to false and checks other codes", () => {
    const lib = new Libhegel(fakeBindings({ recursionLeaf: () => -10, recursionFinish: () => -1 }));
    const h = lib.newContext();
    expect(lib.recursionLeaf(h, h, h)).toBe(false);
    expect(() => lib.recursionFinish(h, h, h)).toThrow(StopTestError);
  });

  it("maps other non-OK codes to LibhegelError with the diagnostic", () => {
    const lib = new Libhegel(
      fakeBindings({ stopSpan: () => -5, contextLastError: () => "bad arg" }),
    );
    const h = lib.newContext();
    try {
      lib.stopSpan(h, h, false);
      expect.unreachable();
    } catch (e) {
      expect(e).toBeInstanceOf(LibhegelError);
      expect((e as LibhegelError).code).toBe(-5);
      expect((e as LibhegelError).message).toMatch(/bad arg/);
    }
  });

  it("passes lastError and failureOrigin through; reproductionBlob and runError pass NULL through", () => {
    const lib = new Libhegel(
      fakeBindings({
        contextLastError: () => "diagnostic",
        failureOrigin: () => "origin",
        failureReproductionBlob: () => null,
        runResultError: () => null,
      }),
    );
    const h = lib.newContext();
    expect(lib.lastError(h)).toBe("diagnostic");
    expect(lib.failureOrigin(h)).toBe("origin");
    expect(lib.reproductionBlob(h)).toBeNull();
    expect(lib.runError(h)).toBeNull();
  });

  it("newCollection uses UINT64_MAX when max is omitted", () => {
    let seenMax: bigint | null = null;
    const handle = {} as Ptr;
    const lib = new Libhegel(
      fakeBindings({
        newCollection: (_ctx, _tc, _min, max, out) => {
          seenMax = max;
          out[0] = handle;
          return 0;
        },
      }),
    );
    const h = lib.newContext();
    lib.newCollection(h, h, 0);
    expect(seenMax).toBe(0xffffffffffffffffn);
  });

  it("newCollection passes an explicit max through", () => {
    let seenMax: bigint | null = null;
    const lib = new Libhegel(
      fakeBindings({
        newCollection: (_ctx, _tc, _min, max, out) => {
          seenMax = max;
          out[0] = {} as Ptr;
          return 0;
        },
      }),
    );
    const h = lib.newContext();
    lib.newCollection(h, h, 0, 5);
    expect(seenMax).toBe(5n);
  });

  it("collectionMore returns the out flag", () => {
    const lib = new Libhegel(
      fakeBindings({
        collectionMore: (_ctx, _tc, _collection, out) => {
          out[0] = true;
          return 0;
        },
      }),
    );
    const h = lib.newContext();
    expect(lib.collectionMore(h, h, h)).toBe(true);
  });

  it("generateBytes passes maxSize through and copies empty results", () => {
    let seenMax: bigint | null = null;
    const lib = new Libhegel(
      fakeBindings({
        generateBytes: (_ctx, _tc, _min, max, out) => {
          seenMax = max;
          out[0] = { data: null, len: 0 };
          return 0;
        },
      }),
    );
    const h = lib.newContext();
    expect(lib.generateBytes(h, h, 0, 0xffffffffffffffffn)).toEqual(Buffer.alloc(0));
    expect(seenMax).toBe(0xffffffffffffffffn);
    expect(lib.generateBytes(h, h, 0, 5n)).toEqual(Buffer.alloc(0));
    expect(seenMax).toBe(5n);
  });

  it("generateString decodes an empty result to the empty string", () => {
    const lib = new Libhegel(
      fakeBindings({
        generateString: (_ctx, _tc, _generator, out) => {
          out[0] = { data: null, len: 0 };
          return 0;
        },
      }),
    );
    const h = lib.newContext();
    expect(lib.generateString(h, h, h)).toBe("");
  });

  it("state-machine index reads map the DONE sentinel to null", () => {
    // koffi returns INT64_MIN as a bigint (it is not a safe integer) and
    // small indices as numbers; both shapes must decode.
    const done = -(2n ** 63n);
    const lib = new Libhegel(
      fakeBindings({
        stateMachineNextGroup: (_ctx, _tc, _machine, out) => {
          out[0] = done;
          return 0;
        },
        stateMachineNextRule: (_ctx, _tc, _machine, _worker, out) => {
          out[0] = 2;
          return 0;
        },
      }),
    );
    const h = lib.newContext();
    expect(lib.stateMachineNextGroup(h, h, h)).toBeNull();
    expect(lib.stateMachineNextRule(h, h, h, 0)).toBe(2);
  });

  it("trivial pass-throughs do not throw", () => {
    const lib = new Libhegel(fakeBindings({}));
    const h = lib.newContext();
    expect(lib.version()).toBe("0.0.0");
    lib.freeContext(lib.newContext());
    lib.freeSettings(lib.newSettings(h));
    lib.setTestCases(h, 10);
    lib.setVerbosity(h, NativeVerbosity.QUIET);
    lib.setSeed(h, 42n);
    lib.setDerandomize(h, true);
    lib.setDatabase(h, h, "");
    lib.setDatabaseKey(h, h, "k");
    lib.setSuppressHealthCheck(h, 1);
    lib.setReportMultipleFailures(h, true);
    lib.collectionReject(h, h, h, "dup");
    lib.freeCollection(lib.newContext());
    lib.markComplete(h, h, Status.VALID, null);
    lib.freePool(lib.newPool(h, h));
    expect(lib.poolAdd(h, h, h)).toBe(0n);
    expect(lib.poolGenerate(h, h, h, true)).toBe(0n);
    lib.freeStateMachine(
      lib.newStateMachine(h, h, {
        ruleNames: ["a"],
        ruleGroups: [0],
        invariantNames: [],
        invariantAlwaysCheck: [],
        minConcurrency: 1,
        maxConcurrency: 1,
        stepCount: 50,
      }),
    );
    lib.stateMachineRuleRejected(h, h, h, 0);
    expect(lib.stateMachineShouldCheckInvariant(h, h, h, 0)).toBe(false);
    lib.freeRunResult(lib.runResult(h, h));
    lib.freeFailure(lib.failure(h, 0));
    lib.freeRun(lib.runStart(h, h));
    lib.freeRun(lib.runStartBlob(h, h, "blob"));
    expect(lib.getVerbosity(h)).toBe(0);
    expect(lib.getPhases(h)).toBe(0);
    expect(lib.getPrintBlob(h)).toBe(false);
    expect(lib.getReportMultipleFailures(h)).toBe(false);
    expect(lib.shouldCapture(h, h)).toBe(false);
    lib.freeTestCase(lib.cloneTestCase(h, h));
    lib.freeTestCase(lib.blockTestCase(h, h, 2));
    lib.setWorker(h, h, 1);
    lib.note(h, h, "note");
    lib.target(h, h, 1, "t");
    lib.event(h, h, "e");
    lib.eventValue(h, h, 1, "v");
    expect(lib.labelFromName(h, "x")).toBe(0n);
    expect(lib.labelCombine(h, [1n, 2n])).toBe(0n);
    const printer = lib.testCasePrinter(h, h);
    lib.printerText(h, printer, "a");
    lib.printerReflow(h, printer, "b");
    lib.printerBeginGroup(h, printer, 4, "[");
    lib.printerBreakable(h, printer, " ");
    lib.printerIfBreak(h, printer, ",");
    lib.printerComment(h, printer, "  // c");
    lib.printerShiftIndent(h, printer, -4);
    lib.printerEndGroup(h, printer, "]");
    lib.printerHardBreak(h, printer);
    lib.printerBeginSpeculative(h, printer);
    lib.printerCommitSpeculative(h, printer);
    lib.printerAbortSpeculative(h, printer);
    lib.printerResolve(h, printer);
    expect(lib.printerValue(h, printer)).toBe("");
    lib.freePrinter(printer);
    expect(lib.reproductionBlob(h)).toBeNull();
    expect(lib.runStatus(h)).toBe(RunStatus.PASSED);
    expect(lib.failureCount(h)).toBe(0);
    expect(lib.failure(h, 0)).not.toBeNull();
    expect(lib.generateBoolean(h, h, 0.5)).toBe(false);
    expect(lib.generateInteger(h, h, 0n, 10n)).toBe(0n);
    expect(lib.generateIntegerBig(h, h, 0n, 10n)).toBe(0n);
    expect(
      lib.generateFloat(h, h, {
        width: 64,
        minValue: -Infinity,
        maxValue: Infinity,
        allowNan: true,
        allowInfinity: true,
        excludeMin: false,
        excludeMax: false,
        smallestNonzeroMagnitude: Number.MIN_VALUE,
      }),
    ).toBe(0);
    expect(
      lib.generateDate(h, h, { year: 1, month: 1, day: 1 }, { year: 2, month: 1, day: 1 }),
    ).toEqual({
      year: 0,
      month: 0,
      day: 0,
    });
  });
});

// ---------------------------------------------------------------------------
// Integration tests against the real libhegel shared library.
// ---------------------------------------------------------------------------

/** Drive a run of `property` over an integer draw; return the result. */
function driveIntegerRun(
  lib: Libhegel,
  property: (n: number) => boolean,
  opts: { testCases?: number } = {},
): { status: number; failureOrigin?: string; reproductionBlob?: string | null } {
  const ctx = lib.newContext();
  const settings = lib.newSettings(ctx);
  let run: Ptr | undefined;
  try {
    lib.setTestCases(settings, opts.testCases ?? 200);
    lib.setVerbosity(settings, NativeVerbosity.QUIET);
    lib.setDerandomize(settings, true);
    lib.setSeed(settings, 12345n);
    lib.setDatabase(ctx, settings, ""); // disable persistence
    lib.setSuppressHealthCheck(settings, 0);
    run = lib.runStart(ctx, settings);

    for (;;) {
      const tc = lib.nextTestCase(ctx, run);
      if (tc === null) break;
      let status: number = Status.VALID;
      let origin: string | null = null;
      try {
        const value = Number(lib.generateInteger(ctx, tc, -1000n, 1000n));
        if (!property(value)) {
          status = Status.INTERESTING;
          origin = "test:integerRun";
        }
      } catch (e) {
        if (e instanceof StopTestError) {
          status = Status.OVERRUN;
        } else {
          throw e;
        }
      }
      lib.markComplete(ctx, tc, status, origin);
      lib.freeTestCase(tc);
    }

    const result = lib.runResult(ctx, run);
    try {
      const status = lib.runStatus(result);
      if (status === RunStatus.FAILED && lib.failureCount(result) > 0) {
        const f = lib.failure(result, 0);
        try {
          return {
            status,
            failureOrigin: lib.failureOrigin(f),
            reproductionBlob: lib.reproductionBlob(f),
          };
        } finally {
          lib.freeFailure(f);
        }
      }
      return { status };
    } finally {
      lib.freeRunResult(result);
    }
  } finally {
    if (run !== undefined) lib.freeRun(run);
    lib.freeSettings(settings);
    lib.freeContext(ctx);
  }
}

describe("Libhegel against the real library", () => {
  const lib = Libhegel.load(testLibPath());

  it("reports the expected version", () => {
    expect(lib.version()).toBe(LIBHEGEL_VERSION);
  });

  it("passes a property that always holds", () => {
    const res = driveIntegerRun(lib, (n) => n >= -1000 && n <= 1000);
    expect(res.status).toBe(RunStatus.PASSED);
  });

  it("fails and surfaces a failure with origin and a reproduce blob for a false property", () => {
    const res = driveIntegerRun(lib, (n) => n < 50);
    expect(res.status).toBe(RunStatus.FAILED);
    expect(res.failureOrigin).toBe("test:integerRun");
    expect(typeof res.reproductionBlob).toBe("string");
  });

  it("throws a LibhegelError when the default settings profile cannot be resolved", () => {
    const original = process.env["HEGEL_DEFAULT_PROFILE"];
    const ctx = lib.newContext();
    try {
      process.env["HEGEL_DEFAULT_PROFILE"] = "no-such-profile";
      expect(() => lib.newSettings(ctx)).toThrow(LibhegelError);
      expect(() => lib.newSettings(ctx)).toThrow(/unknown settings profile "no-such-profile"/);
    } finally {
      if (original === undefined) {
        delete process.env["HEGEL_DEFAULT_PROFILE"];
      } else {
        process.env["HEGEL_DEFAULT_PROFILE"] = original;
      }
      lib.freeContext(ctx);
    }
  });

  it("throws a LibhegelError on invalid draw arguments (inverted integer bounds)", () => {
    const ctx = lib.newContext();
    const settings = lib.newSettings(ctx);
    lib.setVerbosity(settings, NativeVerbosity.QUIET);
    lib.setDatabase(ctx, settings, "");
    const run = lib.runStart(ctx, settings);
    try {
      const tc = lib.nextTestCase(ctx, run);
      expect(tc).not.toBeNull();
      expect(() => lib.generateInteger(ctx, tc, 10n, -10n)).toThrow(LibhegelError);
      lib.markComplete(ctx, tc, Status.INVALID, null);
      lib.freeTestCase(tc);
    } finally {
      lib.freeRun(run);
      lib.freeSettings(settings);
      lib.freeContext(ctx);
    }
  });

  it("throws when next_test_case is called before completing the previous case", () => {
    const ctx = lib.newContext();
    const settings = lib.newSettings(ctx);
    lib.setVerbosity(settings, NativeVerbosity.QUIET);
    lib.setDatabase(ctx, settings, "");
    const run = lib.runStart(ctx, settings);
    try {
      const tc = lib.nextTestCase(ctx, run);
      expect(tc).not.toBeNull();
      // Misuse: pull again without marking the first complete.
      expect(() => lib.nextTestCase(ctx, run)).toThrow(LibhegelError);
      lib.markComplete(ctx, tc, Status.VALID, null);
      lib.freeTestCase(tc);
    } finally {
      lib.freeRun(run);
      lib.freeSettings(settings);
      lib.freeContext(ctx);
    }
  });

  it("drives spans and the collection protocol (lists)", () => {
    const ctx = lib.newContext();
    const settings = lib.newSettings(ctx);
    lib.setTestCases(settings, 20);
    lib.setVerbosity(settings, NativeVerbosity.QUIET);
    lib.setDatabase(ctx, settings, "");
    const run = lib.runStart(ctx, settings);
    try {
      let rejectedOnce = false;
      for (;;) {
        const tc = lib.nextTestCase(ctx, run);
        if (tc === null) break;
        try {
          lib.startSpan(ctx, tc, 1n);
          const collection = lib.newCollection(ctx, tc, 0, 5);
          try {
            while (lib.collectionMore(ctx, tc, collection)) {
              lib.startSpan(ctx, tc, 2n);
              lib.generateInteger(ctx, tc, -100n, 100n);
              lib.stopSpan(ctx, tc, false);
              if (!rejectedOnce) {
                lib.collectionReject(ctx, tc, collection, "exercise reject");
                rejectedOnce = true;
              }
            }
          } finally {
            lib.freeCollection(collection);
          }
          lib.stopSpan(ctx, tc, false);
          lib.markComplete(ctx, tc, Status.VALID, null);
        } catch (e) {
          if (e instanceof StopTestError) {
            lib.markComplete(ctx, tc, Status.OVERRUN, null);
          } else {
            throw e;
          }
        } finally {
          lib.freeTestCase(tc);
        }
      }
      const result = lib.runResult(ctx, run);
      try {
        expect(lib.runStatus(result)).toBe(RunStatus.PASSED);
        expect(lib.runError(result)).toBeNull();
      } finally {
        lib.freeRunResult(result);
      }
    } finally {
      lib.freeRun(run);
      lib.freeSettings(settings);
      lib.freeContext(ctx);
    }
  });

  it("computes labels as unsigned 64-bit hashes of names", () => {
    const ctx = lib.newContext();
    try {
      const a = lib.labelFromName(ctx, "hegel.lists");
      const b = lib.labelFromName(ctx, "hegel.integer");
      expect(a).toBe(lib.labelFromName(ctx, "hegel.lists"));
      expect(a).not.toBe(b);
      for (const label of [a, b, lib.labelCombine(ctx, [a, b])]) {
        expect(label).toBeGreaterThanOrEqual(0n);
        expect(label).toBeLessThan(2n ** 64n);
      }
      expect(lib.labelCombine(ctx, [a, b])).not.toBe(lib.labelCombine(ctx, [b, a]));
    } finally {
      lib.freeContext(ctx);
    }
  });

  it("bindLibrary exposes the version symbol directly", () => {
    // Exercises bindLibrary's returned wrapper independently of Libhegel.
    expect(typeof lib.version()).toBe("string");
  });
});

// Re-export to ensure bindLibrary is referenced (it is used by Libhegel.load).
void bindLibrary;

// Regression coverage for the audited 0.42.4 ABI and result ownership.
describe("Libhegel ABI 0.42.4 regressions", () => {
  it("rejects failed constructors instead of leaking null handles into shared code", () => {
    const lib = new Libhegel(
      fakeBindings({
        contextNew: () => null,
        settingsNew: (_ctx, out) => {
          out[0] = null;
          return 0;
        },
        runResultFailure: () => null,
      }),
    );
    expect(() => lib.newContext()).toThrow("null handle");
    const h = new Libhegel(fakeBindings({})).newContext();
    expect(() => lib.newSettings(h)).toThrow("Foreign, freed or invalid engine handle");
    expect(() => lib.failure(h, 0)).toThrow("Foreign, freed or invalid engine handle");
    const owner = new Libhegel(
      fakeBindings({
        settingsNew: (_ctx, out) => {
          out[0] = null;
          return 0;
        },
        runResultFailure: () => null,
      }),
    );
    const ctx = owner.newContext();
    expect(() => owner.newSettings(ctx)).toThrow("null handle");
    expect(() => owner.failure(ctx, 0)).toThrow("null handle");
  });

  it.each([-3, -4, -5, -6, -7, -8, -9, -10])(
    "does not map result %s to assumption or overrun",
    (code) => {
      const lib = new Libhegel(
        fakeBindings({ generateBoolean: () => code, contextLastError: () => "copied diagnostic" }),
      );
      const h = lib.newContext();
      expect(() => lib.generateBoolean(h, h, 0.5)).toThrow(LibhegelError);
    },
  );

  it("frees native byte/string results when copying fails", () => {
    const bytesFree = vi.fn();
    const stringFree = vi.fn();
    const lib = new Libhegel(
      fakeBindings({
        generateBytes: (_ctx, _tc, _min, _max, out) => {
          out[0] = { data: null, len: -1 };
          return 0;
        },
        generateString: (_ctx, _tc, _generator, out) => {
          out[0] = { data: null, len: -1 };
          return 0;
        },
        generateBytesResultFree: bytesFree,
        ownedStringFree: stringFree,
      }),
    );
    const h = lib.newContext();
    expect(() => lib.generateBytes(h, h, 0, 0n)).toThrow("Invalid native buffer result");
    expect(() => lib.generateString(h, h, h)).toThrow("Invalid native buffer result");
    expect(bytesFree).toHaveBeenCalledExactlyOnceWith({ data: null, len: -1 });
    expect(stringFree).toHaveBeenCalledExactlyOnceWith({ data: null, len: -1 });
  });

  it("checks rejected native setters and passes NUL-containing strings through", () => {
    const fns = bindLibrary(koffi.load(testLibPath()));
    const lib = new Libhegel(fns);
    const ctx = lib.newContext();
    const settings = lib.newSettings(ctx);
    const raw = fns.contextNew();
    try {
      expect(() => fns.settingsTestCases(null, 1)).toThrow(LibhegelError);
      expect(() => fns.settingsVerbosity(null, 1)).toThrow(LibhegelError);
      expect(() => fns.settingsSeed(null, 1n, true)).toThrow(LibhegelError);
      expect(() => fns.settingsDerandomize(null, true)).toThrow(LibhegelError);
      expect(() => fns.settingsSuppressHealthCheck(null, 1)).toThrow(LibhegelError);
      expect(() => fns.settingsReportMultipleFailures(null, true)).toThrow(LibhegelError);
      expect(() => fns.settingsDatabase(raw, null, "")).toThrow(LibhegelError);
      expect(() => fns.settingsDatabaseKey(raw, null, "key")).toThrow(LibhegelError);
      expect(() => lib.setTestCases(null, 1)).toThrow("Foreign, freed or invalid engine handle");
      lib.setDatabaseKey(ctx, settings, "key\0suffix");
    } finally {
      fns.contextFree(raw);
      lib.freeSettings(settings);
      lib.freeContext(ctx);
    }
  });

  it("marshals fixed negative dates and nanosecond temporal bounds against the published engine", () => {
    const lib = Libhegel.load(testLibPath());
    const ctx = lib.newContext();
    const settings = lib.newSettings(ctx);
    lib.setDatabase(ctx, settings, "");
    lib.setVerbosity(settings, NativeVerbosity.QUIET);
    const run = lib.runStart(ctx, settings);
    const tc = lib.nextTestCase(ctx, run);
    try {
      const date = { year: -12345, month: 6, day: 17 };
      expect(lib.generateDate(ctx, tc, date, date)).toEqual(date);
      for (const nanosecond of [1, 123456789, 999999999]) {
        const time = { hour: 12, minute: 34, second: 56, nanosecond };
        expect(lib.generateTime(ctx, tc, time, time)).toEqual(time);
        const datetime = { date, time };
        expect(lib.generateDatetime(ctx, tc, datetime, datetime)).toEqual(datetime);
      }
      const uuid = lib.generateUuid(ctx, tc, 4);
      expect(uuid).toHaveLength(16);
      expect(uuid[6] >> 4).toBe(4);
      expect(uuid[8] >> 6).toBe(2);
    } finally {
      lib.markComplete(ctx, tc, Status.VALID, null);
      lib.freeTestCase(tc);
      lib.freeRun(run);
      lib.freeSettings(settings);
      lib.freeContext(ctx);
    }
  });
});

describe("bindLibrary checked ABI declarations", () => {
  it("checks configuration codes even when no context diagnostic is available", () => {
    const func = vi.fn((prototype: string) => {
      if (prototype === "hegel_context_last_error") return () => ({ data: null, len: 0 });
      if (prototype === "hegel_settings_set_database") return () => -5;
      return () => 0;
    });
    const bindings = bindLibrary({ func } as unknown as LibraryHandle);
    expect(() => bindings.settingsDatabase({}, {}, "path")).toThrow(LibhegelError);
    expect(func).toHaveBeenCalledWith("int hegel_context_free(void* ctx)");
    expect(func).toHaveBeenCalledWith("int hegel_settings_free(void* ctx, void* s)");
    expect(func).toHaveBeenCalledWith(
      "int hegel_string_generator_free(void* ctx, void* generator)",
    );
  });
});

it("Libhegel frees engine results when Koffi copying throws", () => {
  const copyError = new Error("copy failed");
  const decode = vi.spyOn(koffi, "decode").mockImplementation(() => {
    throw copyError;
  });
  const bytesFree = vi.fn();
  const stringFree = vi.fn();
  const lib = new Libhegel(
    fakeBindings({
      generateBytes: (_ctx, _tc, _min, _max, out) => {
        out[0] = { data: {}, len: 1 };
        return 0;
      },
      generateString: (_ctx, _tc, _generator, out) => {
        out[0] = { data: {}, len: 1 };
        return 0;
      },
      generateBytesResultFree: bytesFree,
      ownedStringFree: stringFree,
    }),
  );
  try {
    const h = lib.newContext();
    expect(() => lib.generateBytes(h, h, 0, 0n)).toThrow(copyError);
    expect(() => lib.generateString(h, h, h)).toThrow(copyError);
    expect(bytesFree).toHaveBeenCalledTimes(1);
    expect(stringFree).toHaveBeenCalledTimes(1);
  } finally {
    decode.mockRestore();
  }
});
