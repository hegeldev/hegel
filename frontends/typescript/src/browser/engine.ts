import {
  EngineError,
  UINT64_MAX,
  doneOrIndex,
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
  type GrammarOptions,
  type StateMachineOptions,
  type NativeDate,
  type NativeTime,
  type NativeDatetime,
  type NativeFloatOptions,
  type NativeTestLocation,
  type NativeRunErrorKind,
  type TextGeneratorOptions,
  type UuidVersion,
} from "../engine.js";
import { AssumeError, StopTestError } from "../testCase.js";
import { bigIntToTwosComplementLE, twosComplementLEToBigInt } from "../bytes.js";
import { wtf8ToString } from "../wtf8.js";
import { WasmAbi, u32, u64, type Operation, type Argument } from "./abi.js";
import { WasmArena, WasmMemory } from "./arena.js";

/** The engine-allocated buffer results, each with the destructor that releases it. */
const BUFFER_FREE = {
  generate_bytes: "generate_bytes_result_free",
  generate_string: "owned_string_free",
  printer_value: "printer_value_result_free",
  grammar_draw_result: "owned_string_free",
} as const;

export class WasmEngine implements Engine {
  private readonly handles = new WeakMap<object, number>();
  private readonly memory: WasmMemory;

  constructor(private readonly abi: WasmAbi) {
    this.memory = new WasmMemory(abi.memory);
  }

  private ptr(handle: object): number {
    const ptr = this.handles.get(handle);
    if (ptr === undefined) throw new EngineError("Foreign, freed or invalid engine handle");
    this.memory.bytes(ptr, 1);
    return ptr;
  }

  private owned<H>(ptr: number): H {
    if (ptr === 0) throw new EngineError("Wasm engine returned a null owned handle");
    this.memory.bytes(ptr, 1);
    const handle = Object.freeze({});
    this.handles.set(handle, ptr);
    return handle as H;
  }

  private check(ctx: number, op: Operation, args: Argument[], control = false): void {
    const code = this.abi.call(op, ...args);
    if (code !== 0) this.raise(ctx, op, code, control);
  }

  private raise(ctx: number, op: Operation, code: number, control: boolean): never {
    if (control && code === -1) throw new StopTestError();
    if (control && code === -2) throw new AssumeError();
    // Copy before the arena or an owned handle can be released.
    const message = ctx === 0 ? "" : this.readLastError(ctx);
    throw new EngineError(`hegel_${op} failed (${code})${message ? `: ${message}` : ""}`);
  }

  /**
   * Call an operation that writes an output of `size` bytes, as {@link output}
   * does, but return `null` for `HEGEL_E_NOT_FOUND`.
   */
  private found<T>(
    ctx: number,
    op: Operation,
    args: Argument[],
    size: number,
    read: (arena: WasmArena, ptr: number) => T,
  ): T | null {
    return WasmArena.scoped(this.abi, (arena) => {
      const out = arena.alloc(size, 4);
      const code = this.abi.call(op, ...args, out);
      if (code === -11) return null;
      if (code !== 0) this.raise(ctx, op, code, true);
      return read(arena, out);
    });
  }

  /** `false` for `HEGEL_E_RETRY`, otherwise as {@link check} with control codes. */
  private retryable(ctx: number, op: Operation, args: Argument[]): boolean {
    const code = this.abi.call(op, ...args);
    if (code === -10) return false;
    if (code !== 0) this.raise(ctx, op, code, true);
    return true;
  }

  private readLastError(ctx: number): string {
    return WasmArena.scoped(this.abi, (a) => {
      const out = a.alloc(8, 4);
      this.abi.call("context_last_error", out, ctx);
      return a.string(out);
    });
  }

  private output<T>(
    ctx: number,
    op: Operation,
    args: Argument[],
    size: number,
    align: number,
    read: (arena: WasmArena, ptr: number) => T,
    control = false,
  ): T {
    return WasmArena.scoped(this.abi, (arena) => {
      const out = arena.alloc(size, align);
      this.check(ctx, op, [...args, out], control);
      return read(arena, out);
    });
  }

  private handle<H>(ctx: number, op: Operation, args: Argument[], control = false): H {
    return this.output(ctx, op, args, 4, 4, (a, p) => this.owned<H>(a.pointer(p)), control);
  }

  private free(op: Operation, handle: object): void {
    const ptr = this.ptr(handle);
    this.check(0, op, [0, ptr]);
    this.handles.delete(handle);
  }

  private string(op: Operation, args: Argument[]): string {
    return this.output(0, op, args, 8, 4, (a, p) => a.string(p));
  }

  private optionalString(op: Operation, args: Argument[]): string | null {
    return this.output(0, op, args, 4, 4, (a, p) => {
      const ptr = a.pointer(p);
      return ptr === 0 ? null : a.string(ptr);
    });
  }

  version(): string {
    return this.string("version", [0]);
  }
  newContext(): ContextHandle {
    return this.owned(this.abi.call("context_new") >>> 0);
  }
  setUnicodeVersion(ctx: ContextHandle, version: string): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "set_unicode_version", [c, a.text(version)]);
    });
  }
  freeContext(ctx: ContextHandle): void {
    this.check(0, "context_free", [this.ptr(ctx)]);
    this.handles.delete(ctx);
  }
  lastError(ctx: ContextHandle): string {
    return this.readLastError(this.ptr(ctx));
  }
  newSettings(ctx: ContextHandle): SettingsHandle {
    const ptr = this.ptr(ctx);
    return this.handle(ptr, "settings_new", [ptr]);
  }
  newSettingsForProfile(ctx: ContextHandle, name: string): SettingsHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "settings_new_for_profile", [c, a.text(name)]);
    });
  }
  freeSettings(s: SettingsHandle): void {
    this.free("settings_free", s);
  }
  setTestCases(s: SettingsHandle, n: number): void {
    this.check(0, "settings_set_test_cases", [0, this.ptr(s), u64(n)]);
  }
  setVerbosity(s: SettingsHandle, v: number): void {
    this.check(0, "settings_set_verbosity", [0, this.ptr(s), u32(v)]);
  }
  setSeed(s: SettingsHandle, seed: bigint | null): void {
    this.check(0, "settings_set_seed", [0, this.ptr(s), u64(seed ?? 0n), +(seed !== null)]);
  }
  setDerandomize(s: SettingsHandle, on: boolean): void {
    this.check(0, "settings_set_derandomize", [0, this.ptr(s), +on]);
  }
  setDatabase(ctx: ContextHandle, s: SettingsHandle, db: string | null): void {
    if (db !== null && db !== "")
      throw new EngineError("Filesystem databases are not supported in the browser");
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "settings_set_database", [c, this.ptr(s), a.text("")]);
    });
  }
  setDatabaseKey(ctx: ContextHandle, s: SettingsHandle, key: string): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "settings_set_database_key", [c, this.ptr(s), a.text(key)]);
    });
  }
  setSuppressHealthCheck(s: SettingsHandle, checks: number): void {
    this.check(0, "settings_set_suppress_health_check", [0, this.ptr(s), u32(checks)]);
  }
  setReportMultipleFailures(s: SettingsHandle, yes: boolean): void {
    this.check(0, "settings_set_report_multiple_failures", [0, this.ptr(s), +yes]);
  }
  setStopOnFirstFailure(s: SettingsHandle, yes: boolean): void {
    this.check(0, "settings_set_stop_on_first_failure", [0, this.ptr(s), +yes]);
  }
  setShowStatistics(s: SettingsHandle, yes: boolean): void {
    this.check(0, "settings_set_show_statistics", [0, this.ptr(s), +yes]);
  }
  setUnboundedChoices(s: SettingsHandle, yes: boolean): void {
    this.check(0, "settings_set_unbounded_choices", [0, this.ptr(s), +yes]);
  }
  setPrintBlob(s: SettingsHandle, yes: boolean): void {
    this.check(0, "settings_set_print_blob", [0, this.ptr(s), +yes]);
  }
  setPhases(s: SettingsHandle, phases: number): void {
    this.check(0, "settings_set_phases", [0, this.ptr(s), u32(phases)]);
  }
  setBackend(s: SettingsHandle, backend: number): void {
    this.check(0, "settings_set_backend", [0, this.ptr(s), u32(backend)]);
  }
  setNondeterminismStrictness(s: SettingsHandle, strictness: number): void {
    this.check(0, "settings_set_nondeterminism_strictness", [0, this.ptr(s), u32(strictness)]);
  }
  setTestLocation(ctx: ContextHandle, s: SettingsHandle, location: NativeTestLocation): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "settings_set_test_location", [
        c,
        this.ptr(s),
        a.text(location.file),
        u32(location.beginLine),
        a.text(location.className),
        a.text(location.function),
      ]);
    });
  }
  registerProfile(ctx: ContextHandle, name: string, s: SettingsHandle): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "settings_register_profile", [c, a.text(name), this.ptr(s)]);
    });
  }
  setDefaultProfile(ctx: ContextHandle, name: string): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "set_default_profile", [c, a.text(name)]);
    });
  }
  private settingsValue<T>(
    op:
      | "settings_get_verbosity"
      | "settings_get_phases"
      | "settings_get_print_blob"
      | "settings_get_report_multiple_failures"
      | "settings_get_test_cases"
      | "settings_get_derandomize"
      | "settings_get_suppress_health_check"
      | "settings_get_stop_on_first_failure"
      | "settings_get_show_statistics"
      | "settings_get_unbounded_choices"
      | "settings_get_backend"
      | "settings_get_nondeterminism_strictness",
    s: SettingsHandle,
    size: number,
    read: (arena: WasmArena, ptr: number) => T,
  ): T {
    return this.output(0, op, [0, this.ptr(s)], size, size, read);
  }
  getVerbosity(s: SettingsHandle): number {
    return this.settingsValue("settings_get_verbosity", s, 4, (a, p) => a.pointer(p));
  }
  getPhases(s: SettingsHandle): number {
    return this.settingsValue("settings_get_phases", s, 4, (a, p) => a.pointer(p));
  }
  getPrintBlob(s: SettingsHandle): boolean {
    return this.settingsValue("settings_get_print_blob", s, 1, (a, p) => a.boolean(p));
  }
  getReportMultipleFailures(s: SettingsHandle): boolean {
    return this.settingsValue("settings_get_report_multiple_failures", s, 1, (a, p) =>
      a.boolean(p),
    );
  }
  getTestCases(s: SettingsHandle): number {
    return this.settingsValue("settings_get_test_cases", s, 8, (a, p) =>
      Number(a.view(p, 8).getBigUint64(0, true)),
    );
  }
  getSeed(s: SettingsHandle): bigint | null {
    return WasmArena.scoped(this.abi, (a) => {
      const seed = a.alloc(8, 8);
      const hasSeed = a.alloc(1, 1);
      this.check(0, "settings_get_seed", [0, this.ptr(s), seed, hasSeed]);
      return a.boolean(hasSeed) ? a.view(seed, 8).getBigUint64(0, true) : null;
    });
  }
  getDerandomize(s: SettingsHandle): boolean {
    return this.settingsValue("settings_get_derandomize", s, 1, (a, p) => a.boolean(p));
  }
  getDatabase(s: SettingsHandle): string | null {
    return this.optionalString("settings_get_database", [0, this.ptr(s)]);
  }
  getSuppressHealthCheck(s: SettingsHandle): number {
    return this.settingsValue("settings_get_suppress_health_check", s, 4, (a, p) => a.pointer(p));
  }
  getStopOnFirstFailure(s: SettingsHandle): boolean {
    return this.settingsValue("settings_get_stop_on_first_failure", s, 1, (a, p) => a.boolean(p));
  }
  getShowStatistics(s: SettingsHandle): boolean {
    return this.settingsValue("settings_get_show_statistics", s, 1, (a, p) => a.boolean(p));
  }
  getUnboundedChoices(s: SettingsHandle): boolean {
    return this.settingsValue("settings_get_unbounded_choices", s, 1, (a, p) => a.boolean(p));
  }
  getBackend(s: SettingsHandle): number {
    return this.settingsValue("settings_get_backend", s, 4, (a, p) => a.pointer(p));
  }
  getNondeterminismStrictness(s: SettingsHandle): number {
    return this.settingsValue("settings_get_nondeterminism_strictness", s, 4, (a, p) =>
      a.pointer(p),
    );
  }
  runStart(ctx: ContextHandle, settings: SettingsHandle): RunHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "run_start", [c, this.ptr(settings), 0, 0]);
  }
  runStartBlob(ctx: ContextHandle, settings: SettingsHandle, blob: string): RunHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "run_start_blob", [c, this.ptr(settings), a.text(blob), 0, 0]);
    });
  }
  nextTestCase(ctx: ContextHandle, run: RunHandle): TestCaseHandle | null {
    const c = this.ptr(ctx);
    return this.output(c, "next_test_case", [c, this.ptr(run)], 4, 4, (a, p) => {
      const ptr = a.pointer(p);
      return ptr === 0 ? null : this.owned<TestCaseHandle>(ptr);
    });
  }
  runResult(ctx: ContextHandle, run: RunHandle): RunResultHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "run_result", [c, this.ptr(run)]);
  }
  freeRunResult(r: RunResultHandle): void {
    this.free("run_result_free", r);
  }
  freeRun(run: RunHandle): void {
    this.free("run_free", run);
  }
  freeTestCase(tc: TestCaseHandle): void {
    this.free("test_case_free", tc);
  }
  shouldCapture(ctx: ContextHandle, tc: TestCaseHandle): boolean {
    const c = this.ptr(ctx);
    return this.output(c, "test_case_should_capture", [c, this.ptr(tc)], 1, 1, (a, p) =>
      a.boolean(p),
    );
  }
  cloneTestCase(ctx: ContextHandle, tc: TestCaseHandle): TestCaseHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "test_case_clone", [c, this.ptr(tc)], true);
  }
  blockTestCase(ctx: ContextHandle, tc: TestCaseHandle, indent: number): TestCaseHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "test_case_block", [c, this.ptr(tc), u64(indent)], true);
  }
  setWorker(ctx: ContextHandle, tc: TestCaseHandle, index: number): void {
    const c = this.ptr(ctx);
    this.check(c, "test_case_set_worker", [c, this.ptr(tc), BigInt(index)], true);
  }
  private withText(
    ctx: ContextHandle,
    op:
      | "note"
      | "event"
      | "printer_text"
      | "printer_end_group"
      | "printer_breakable"
      | "printer_if_break"
      | "printer_comment",
    target: object,
    text: string,
  ): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, op, [c, this.ptr(target), a.text(text)], true);
    });
  }
  private withValue(
    ctx: ContextHandle,
    op: "target" | "event_value",
    tc: TestCaseHandle,
    value: number,
    label: string,
  ): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, op, [c, this.ptr(tc), value, a.text(label)], true);
    });
  }
  note(ctx: ContextHandle, tc: TestCaseHandle, text: string): void {
    this.withText(ctx, "note", tc, text);
  }
  target(ctx: ContextHandle, tc: TestCaseHandle, value: number, label: string): void {
    this.withValue(ctx, "target", tc, value, label);
  }
  event(ctx: ContextHandle, tc: TestCaseHandle, label: string): void {
    this.withText(ctx, "event", tc, label);
  }
  eventValue(ctx: ContextHandle, tc: TestCaseHandle, value: number, label: string): void {
    this.withValue(ctx, "event_value", tc, value, label);
  }
  labelFromName(ctx: ContextHandle, name: string): bigint {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      const out = a.alloc(8, 8);
      this.check(c, "label_from_name", [c, a.text(name), out]);
      return a.view(out, 8).getBigUint64(0, true);
    });
  }
  labelCombine(ctx: ContextHandle, labels: readonly bigint[]): bigint {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      const input = a.alloc(Math.max(8, labels.length * 8), 8);
      labels.forEach((label, index) => {
        a.view(input + index * 8, 8).setBigUint64(0, label, true);
      });
      const out = a.alloc(8, 8);
      this.check(c, "label_combine", [c, input, u32(labels.length), out]);
      return a.view(out, 8).getBigUint64(0, true);
    });
  }
  testCasePrinter(ctx: ContextHandle, tc: TestCaseHandle): PrinterHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "test_case_printer", [c, this.ptr(tc), 0]);
  }
  freePrinter(p: PrinterHandle): void {
    this.free("printer_free", p);
  }
  printerText(ctx: ContextHandle, p: PrinterHandle, text: string): void {
    this.withText(ctx, "printer_text", p, text);
  }
  printerBeginGroup(ctx: ContextHandle, p: PrinterHandle, indent: number, open: string): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "printer_begin_group", [c, this.ptr(p), u64(indent), a.text(open)], true);
    });
  }
  printerEndGroup(ctx: ContextHandle, p: PrinterHandle, close: string): void {
    this.withText(ctx, "printer_end_group", p, close);
  }
  printerBreakable(ctx: ContextHandle, p: PrinterHandle, sep: string): void {
    this.withText(ctx, "printer_breakable", p, sep);
  }
  printerIfBreak(ctx: ContextHandle, p: PrinterHandle, text: string): void {
    this.withText(ctx, "printer_if_break", p, text);
  }
  printerComment(ctx: ContextHandle, p: PrinterHandle, text: string): void {
    this.withText(ctx, "printer_comment", p, text);
  }
  printerShiftIndent(ctx: ContextHandle, p: PrinterHandle, delta: number): void {
    const c = this.ptr(ctx);
    this.check(c, "printer_shift_indent", [c, this.ptr(p), BigInt(delta)], true);
  }
  printerReflow(ctx: ContextHandle, p: PrinterHandle, text: string): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "printer_reflow", [c, this.ptr(p), 0, a.text(text)]);
    });
  }
  private printerOp(
    ctx: ContextHandle,
    op:
      | "printer_hard_break"
      | "printer_begin_speculative"
      | "printer_commit_speculative"
      | "printer_abort_speculative"
      | "printer_resolve",
    p: PrinterHandle,
  ): void {
    const c = this.ptr(ctx);
    this.check(c, op, [c, this.ptr(p)]);
  }
  printerHardBreak(ctx: ContextHandle, p: PrinterHandle): void {
    this.printerOp(ctx, "printer_hard_break", p);
  }
  printerBeginSpeculative(ctx: ContextHandle, p: PrinterHandle): void {
    this.printerOp(ctx, "printer_begin_speculative", p);
  }
  printerCommitSpeculative(ctx: ContextHandle, p: PrinterHandle): void {
    this.printerOp(ctx, "printer_commit_speculative", p);
  }
  printerAbortSpeculative(ctx: ContextHandle, p: PrinterHandle): void {
    this.printerOp(ctx, "printer_abort_speculative", p);
  }
  printerResolve(ctx: ContextHandle, p: PrinterHandle): void {
    this.printerOp(ctx, "printer_resolve", p);
  }
  printerValue(ctx: ContextHandle, p: PrinterHandle): string {
    const c = this.ptr(ctx);
    return this.buffer(c, "printer_value", [c, this.ptr(p)], wtf8ToString);
  }

  generateBoolean(ctx: ContextHandle, tc: TestCaseHandle, p: number): boolean {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_boolean",
      [c, this.ptr(tc), p, 0, 0],
      1,
      1,
      (a, p) => a.boolean(p),
      true,
    );
  }
  generateInteger(ctx: ContextHandle, tc: TestCaseHandle, min: bigint, max: bigint): bigint {
    if (min < -0x8000000000000000n || max > 0x7fffffffffffffffn || min > max) {
      throw new EngineError("Invalid signed i64 bounds");
    }
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_integer",
      [c, this.ptr(tc), min, max],
      8,
      8,
      (a, p) => a.view(p, 8).getBigInt64(0, true),
      true,
    );
  }
  generateIntegerBig(ctx: ContextHandle, tc: TestCaseHandle, min: bigint, max: bigint): bigint {
    return WasmArena.scoped(this.abi, (a) => {
      const lo = bigIntToTwosComplementLE(min),
        hi = bigIntToTwosComplementLE(max);
      const capacity = Math.max(lo.length, hi.length);
      const out = a.alloc(capacity),
        length = a.alloc(4, 4);
      const c = this.ptr(ctx);
      this.check(
        c,
        "generate_integer_big",
        [c, this.ptr(tc), a.input(lo), lo.length, a.input(hi), hi.length, out, capacity, length],
        true,
      );
      const written = a.pointer(length);
      if (written === 0 || written > capacity)
        throw new EngineError("Invalid big integer result length");
      return twosComplementLEToBigInt(a.bytes(out, written));
    });
  }
  generateFloat(ctx: ContextHandle, tc: TestCaseHandle, opts: NativeFloatOptions): number {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_float",
      [
        c,
        this.ptr(tc),
        u32(opts.width),
        opts.minValue,
        opts.maxValue,
        +opts.allowNan,
        +opts.allowInfinity,
        +opts.excludeMin,
        +opts.excludeMax,
        opts.smallestNonzeroMagnitude,
      ],
      8,
      8,
      (a, p) => a.view(p, 8).getFloat64(0, true),
      true,
    );
  }

  private buffer<T>(
    ctx: number,
    op: keyof typeof BUFFER_FREE,
    args: Argument[],
    decode: (bytes: Uint8Array) => T,
  ): T {
    return WasmArena.scoped(this.abi, (a) => {
      const out = a.alloc(8, 4);
      this.check(ctx, op, [...args, out], true);
      let result!: T;
      let failure: unknown;
      let failed = false;
      try {
        const data = a.pointer(out),
          length = a.pointer(out + 4);
        if (data === 0) throw new EngineError("Wasm returned a null generated buffer");
        result = decode(a.bytes(data, length).slice());
      } catch (error) {
        failed = true;
        failure = error;
      } finally {
        try {
          this.check(0, BUFFER_FREE[op], [0, out]);
        } catch (cleanup) {
          failure = failed
            ? new EngineError("Wasm buffer copy and release failed", {
                cause: new AggregateError([failure, cleanup]),
              })
            : cleanup;
          failed = true;
        }
      }
      if (failed) throw failure;
      return result;
    });
  }
  generateBytes(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    minSize: number,
    maxSize: bigint,
  ): Uint8Array {
    const c = this.ptr(ctx);
    return this.buffer(
      c,
      "generate_bytes",
      [c, this.ptr(tc), u64(minSize), u64(maxSize)],
      (bytes) => bytes,
    );
  }
  generateString(ctx: ContextHandle, tc: TestCaseHandle, generator: StringGeneratorHandle): string {
    const c = this.ptr(ctx);
    return this.buffer(c, "generate_string", [c, this.ptr(tc), this.ptr(generator)], wtf8ToString);
  }
  stringGeneratorText(ctx: ContextHandle, opts: TextGeneratorOptions): StringGeneratorHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "string_generator_text", [
        c,
        u64(opts.minSize),
        u64(opts.maxSize),
        a.text(opts.codec),
        u32(opts.minCodepoint),
        u32(opts.maxCodepoint),
        a.stringList(opts.categories),
        opts.categories?.length ?? 0,
        a.stringList(opts.excludeCategories),
        opts.excludeCategories?.length ?? 0,
        a.bytesValue(opts.includeCharacters),
        a.bytesValue(opts.excludeCharacters),
      ]);
    });
  }
  stringGeneratorRegex(
    ctx: ContextHandle,
    pattern: string,
    fullmatch: boolean,
    alphabet: StringGeneratorHandle | null,
  ): StringGeneratorHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "string_generator_regex", [
        c,
        a.text(pattern),
        +fullmatch,
        alphabet === null ? 0 : this.ptr(alphabet),
      ]);
    });
  }
  grammarBuilderNew(ctx: ContextHandle, text: string): GrammarBuilderHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "grammar_builder_new", [c, a.text(text)]);
    });
  }
  grammarBuilderFile(
    ctx: ContextHandle,
    builder: GrammarBuilderHandle,
    index: number,
  ): string | null {
    const c = this.ptr(ctx);
    return this.found(c, "grammar_builder_file", [c, this.ptr(builder), u32(index)], 8, (a, p) =>
      a.string(p),
    );
  }
  grammarBuilderProvide(
    ctx: ContextHandle,
    builder: GrammarBuilderHandle,
    index: number,
    text: string | null,
  ): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "grammar_builder_provide", [c, this.ptr(builder), u32(index), a.text(text)]);
    });
  }
  freeGrammarBuilder(builder: GrammarBuilderHandle): void {
    this.free("grammar_builder_free", builder);
  }
  newGrammar(
    ctx: ContextHandle,
    builder: GrammarBuilderHandle,
    opts: GrammarOptions,
  ): GrammarHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "grammar_new", [
        c,
        this.ptr(builder),
        a.text(opts.start),
        u64(opts.maxSize),
        u32(opts.maxDepth),
        opts.alphabet === null ? 0 : this.ptr(opts.alphabet),
        a.stringList(opts.explicitNames),
        opts.explicitNames.length,
      ]);
    });
  }
  freeGrammar(grammar: GrammarHandle): void {
    this.free("grammar_free", grammar);
  }
  grammarDrawNew(ctx: ContextHandle, grammar: GrammarHandle, maxSize: number): GrammarDrawHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "grammar_draw_new", [c, this.ptr(grammar), u64(maxSize)]);
  }
  grammarDrawNext(ctx: ContextHandle, tc: TestCaseHandle, draw: GrammarDrawHandle): number | null {
    const c = this.ptr(ctx);
    return this.found(c, "grammar_draw_next", [c, this.ptr(tc), this.ptr(draw)], 4, (a, p) =>
      a.pointer(p),
    );
  }
  grammarDrawProvide(ctx: ContextHandle, draw: GrammarDrawHandle, value: string): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "grammar_draw_provide", [c, this.ptr(draw), a.text(value)]);
    });
  }
  grammarDrawResult(ctx: ContextHandle, draw: GrammarDrawHandle): string {
    const c = this.ptr(ctx);
    return this.buffer(c, "grammar_draw_result", [c, this.ptr(draw)], wtf8ToString);
  }
  freeGrammarDraw(draw: GrammarDrawHandle): void {
    this.free("grammar_draw_free", draw);
  }
  stringGeneratorEmail(ctx: ContextHandle): StringGeneratorHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "string_generator_email", [c]);
  }
  stringGeneratorUrl(ctx: ContextHandle): StringGeneratorHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "string_generator_url", [c]);
  }
  stringGeneratorDomain(ctx: ContextHandle, maxLength: number): StringGeneratorHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "string_generator_domain", [c, u64(maxLength)]);
  }
  freeStringGenerator(generator: StringGeneratorHandle): void {
    this.free("string_generator_free", generator);
  }

  // These three by-value C inputs lower to pointers in the pinned raw artifact.
  // Reinspect signatures and fixed-value tests before changing its source/toolchain.
  private temporal<T>(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    op: Operation,
    min: T,
    max: T,
    size: number,
    write: (view: DataView, value: T) => void,
    read: (view: DataView) => T,
  ): T {
    return WasmArena.scoped(this.abi, (a) => {
      const lo = a.alloc(size, 4),
        hi = a.alloc(size, 4),
        out = a.alloc(size, 4);
      write(a.view(lo, size), min);
      write(a.view(hi, size), max);
      const c = this.ptr(ctx);
      this.check(c, op, [c, this.ptr(tc), lo, hi, out], true);
      return read(a.view(out, size));
    });
  }
  generateDate(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDate,
    max: NativeDate,
  ): NativeDate {
    return this.temporal(ctx, tc, "generate_date", min, max, 8, writeDate, readDate);
  }
  generateTime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeTime,
    max: NativeTime,
  ): NativeTime {
    return this.temporal(ctx, tc, "generate_time", min, max, 8, writeTime, readTime);
  }
  generateDatetime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDatetime,
    max: NativeDatetime,
  ): NativeDatetime {
    return this.temporal(
      ctx,
      tc,
      "generate_datetime",
      min,
      max,
      16,
      (view, value) => {
        writeDate(view, value.date);
        writeTime(view, value.time, 8);
      },
      (view) => ({ date: readDate(view), time: readTime(view, 8) }),
    );
  }
  generateUuid(ctx: ContextHandle, tc: TestCaseHandle, version?: UuidVersion): Uint8Array {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_uuid",
      [c, this.ptr(tc), version === undefined ? 0 : u32(version), +(version !== undefined)],
      16,
      1,
      (a, p) => a.bytes(p, 16).slice(),
      true,
    );
  }
  generateIpv4(ctx: ContextHandle, tc: TestCaseHandle): Uint8Array {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_ipv4",
      [c, this.ptr(tc)],
      4,
      1,
      (a, p) => a.bytes(p, 4).slice(),
      true,
    );
  }
  generateIpv6(ctx: ContextHandle, tc: TestCaseHandle): Uint8Array {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_ipv6",
      [c, this.ptr(tc)],
      16,
      1,
      (a, p) => a.bytes(p, 16).slice(),
      true,
    );
  }
  startSpan(ctx: ContextHandle, tc: TestCaseHandle, label: bigint): void {
    const c = this.ptr(ctx);
    this.check(c, "start_span", [c, this.ptr(tc), u64(label)], true);
  }
  stopSpan(ctx: ContextHandle, tc: TestCaseHandle, discard: boolean): void {
    const c = this.ptr(ctx);
    this.check(c, "stop_span", [c, this.ptr(tc), +discard], true);
  }
  newCollection(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: number,
    max?: number,
  ): CollectionHandle {
    const c = this.ptr(ctx);
    return this.handle(
      c,
      "new_collection",
      [c, this.ptr(tc), u64(min), max === undefined ? UINT64_MAX : u64(max)],
      true,
    );
  }
  collectionMore(ctx: ContextHandle, tc: TestCaseHandle, collection: CollectionHandle): boolean {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "collection_more",
      [c, this.ptr(tc), this.ptr(collection)],
      1,
      1,
      (a, p) => a.boolean(p),
      true,
    );
  }
  collectionReject(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    collection: CollectionHandle,
    why: string | null,
  ): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(
        c,
        "collection_reject",
        [c, this.ptr(tc), this.ptr(collection), a.text(why)],
        true,
      );
    });
  }
  freeCollection(collection: CollectionHandle): void {
    this.free("collection_free", collection);
  }
  markComplete(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    status: number,
    origin: string | null,
  ): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, "mark_complete", [c, this.ptr(tc), u32(status), a.text(origin)]);
    });
  }
  newPool(ctx: ContextHandle, tc: TestCaseHandle): PoolHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "new_pool", [c, this.ptr(tc)], true);
  }
  private variableId(ctx: number, op: "pool_add" | "pool_generate", args: Argument[]): bigint {
    return this.output(ctx, op, args, 8, 8, (a, p) => a.view(p, 8).getBigInt64(0, true), true);
  }
  poolAdd(ctx: ContextHandle, tc: TestCaseHandle, pool: PoolHandle): bigint {
    const c = this.ptr(ctx);
    return this.variableId(c, "pool_add", [c, this.ptr(tc), this.ptr(pool)]);
  }
  poolGenerate(ctx: ContextHandle, tc: TestCaseHandle, pool: PoolHandle, consume: boolean): bigint {
    const c = this.ptr(ctx);
    return this.variableId(c, "pool_generate", [c, this.ptr(tc), this.ptr(pool), +consume]);
  }
  poolRestore(ctx: ContextHandle, tc: TestCaseHandle, pool: PoolHandle, variableId: bigint): void {
    const c = this.ptr(ctx);
    this.check(c, "pool_restore", [c, this.ptr(tc), this.ptr(pool), variableId], true);
  }
  freePool(pool: PoolHandle): void {
    this.free("pool_free", pool);
  }
  newRecursion(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    maxDepth: number,
    maxLeaves: number,
  ): RecursionHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "new_recursion", [c, this.ptr(tc), u64(maxDepth), u64(maxLeaves)], true);
  }
  recursionBranch(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    recursion: RecursionHandle,
    depth: number,
  ): boolean {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "recursion_branch",
      [c, this.ptr(tc), this.ptr(recursion), u64(depth)],
      1,
      1,
      (a, p) => a.boolean(p),
      true,
    );
  }
  recursionLeaf(ctx: ContextHandle, tc: TestCaseHandle, recursion: RecursionHandle): boolean {
    const c = this.ptr(ctx);
    return this.retryable(c, "recursion_leaf", [c, this.ptr(tc), this.ptr(recursion)]);
  }
  recursionRetry(ctx: ContextHandle, tc: TestCaseHandle, recursion: RecursionHandle): void {
    const c = this.ptr(ctx);
    this.check(c, "recursion_retry", [c, this.ptr(tc), this.ptr(recursion)], true);
  }
  recursionFinish(ctx: ContextHandle, tc: TestCaseHandle, recursion: RecursionHandle): boolean {
    const c = this.ptr(ctx);
    return this.retryable(c, "recursion_finish", [c, this.ptr(tc), this.ptr(recursion)]);
  }
  freeRecursion(recursion: RecursionHandle): void {
    this.free("recursion_free", recursion);
  }
  newStateMachine(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    opts: StateMachineOptions,
  ): StateMachineHandle {
    return WasmArena.scoped(this.abi, (a) => {
      // `int64_t` group ids, written little-endian; a present empty array is
      // still a valid (never dereferenced) pointer, like `stringList`.
      const groups = a.alloc(Math.max(8, opts.ruleGroups.length * 8), 8);
      opts.ruleGroups.forEach((group, index) => {
        a.view(groups + index * 8, 8).setBigInt64(0, BigInt(group), true);
      });
      const c = this.ptr(ctx);
      // The machine handle is not the trailing out-parameter here: the drawn
      // concurrency level follows it (and is discarded, the bounds being 1).
      const machine = a.alloc(4, 4);
      const concurrency = a.alloc(8, 8);
      this.check(
        c,
        "new_state_machine",
        [
          c,
          this.ptr(tc),
          a.stringList(opts.ruleNames),
          groups,
          // `rule_weights`: NULL for all-equal weights, which this client
          // does not expose.
          0,
          u32(opts.ruleNames.length),
          a.stringList(opts.invariantNames),
          a.input(Uint8Array.from(opts.invariantAlwaysCheck, Number)),
          u32(opts.invariantNames.length),
          BigInt(opts.minConcurrency),
          BigInt(opts.maxConcurrency),
          BigInt(opts.stepCount),
          machine,
          concurrency,
        ],
        true,
      );
      return this.owned<StateMachineHandle>(a.pointer(machine));
    });
  }
  private stateMachineIndex(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
    op: "state_machine_next_group" | "state_machine_next_rule",
    extra: Argument[],
  ): number | null {
    const c = this.ptr(ctx);
    return this.output(
      c,
      op,
      [c, this.ptr(tc), this.ptr(machine), ...extra],
      8,
      8,
      (a, p) => doneOrIndex(a.view(p, 8).getBigInt64(0, true)),
      true,
    );
  }
  stateMachineNextGroup(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
  ): number | null {
    return this.stateMachineIndex(ctx, tc, machine, "state_machine_next_group", []);
  }
  stateMachineNextRule(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
    workerIndex: number,
  ): number | null {
    return this.stateMachineIndex(ctx, tc, machine, "state_machine_next_rule", [
      BigInt(workerIndex),
    ]);
  }
  stateMachineRuleRejected(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
    workerIndex: number,
  ): void {
    const c = this.ptr(ctx);
    this.check(
      c,
      "state_machine_rule_rejected",
      [c, this.ptr(tc), this.ptr(machine), BigInt(workerIndex)],
      true,
    );
  }
  stateMachineShouldCheckInvariant(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
    invariantIndex: number,
  ): boolean {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "state_machine_should_check_invariant",
      [c, this.ptr(tc), this.ptr(machine), BigInt(invariantIndex)],
      1,
      1,
      (a, p) => a.boolean(p),
      true,
    );
  }
  freeStateMachine(machine: StateMachineHandle): void {
    this.free("state_machine_free", machine);
  }
  runStatus(r: RunResultHandle): number {
    return this.output(0, "run_result_status", [0, this.ptr(r)], 4, 4, (a, p) =>
      a.view(p, 4).getInt32(0, true),
    );
  }
  runError(r: RunResultHandle): string | null {
    return this.optionalString("run_result_error", [0, this.ptr(r)]);
  }
  runErrorKind(r: RunResultHandle): NativeRunErrorKind {
    return WasmArena.scoped(this.abi, (a) => {
      const kind = a.alloc(4, 4),
        healthCheck = a.alloc(4, 4);
      this.check(0, "run_result_error_kind", [0, this.ptr(r), kind, healthCheck]);
      return { kind: a.pointer(kind), healthCheck: a.pointer(healthCheck) };
    });
  }
  failureCount(r: RunResultHandle): number {
    return this.output(0, "run_result_failure_count", [0, this.ptr(r)], 4, 4, (a, p) =>
      a.pointer(p),
    );
  }
  failure(r: RunResultHandle, index: number): FailureHandle {
    return this.handle(0, "run_result_failure", [0, this.ptr(r), u32(index)]);
  }
  freeFailure(f: FailureHandle): void {
    this.free("failure_free", f);
  }
  failureOrigin(fp: FailureHandle): string {
    return this.string("failure_origin", [0, this.ptr(fp)]);
  }
  reproductionBlob(fp: FailureHandle): string | null {
    return this.optionalString("failure_reproduction_blob", [0, this.ptr(fp)]);
  }
  failureCaveat(fp: FailureHandle): string | null {
    return this.optionalString("failure_caveat", [0, this.ptr(fp)]);
  }
}

function integer(value: number, min: number, max: number): number {
  if (!Number.isInteger(value) || value < min || value > max)
    throw new EngineError("Invalid temporal field");
  return value;
}
function writeDate(view: DataView, date: NativeDate): void {
  view.setInt32(0, integer(date.year, -0x80000000, 0x7fffffff), true);
  view.setUint8(4, integer(date.month, 1, 12));
  view.setUint8(5, integer(date.day, 1, 31));
}
function readDate(view: DataView): NativeDate {
  return {
    year: view.getInt32(0, true),
    month: integer(view.getUint8(4), 1, 12),
    day: integer(view.getUint8(5), 1, 31),
  };
}
function writeTime(view: DataView, time: NativeTime, offset = 0): void {
  view.setUint8(offset, integer(time.hour, 0, 23));
  view.setUint8(offset + 1, integer(time.minute, 0, 59));
  view.setUint8(offset + 2, integer(time.second, 0, 59));
  view.setUint32(offset + 4, integer(time.nanosecond, 0, 999999999), true);
}
function readTime(view: DataView, offset = 0): NativeTime {
  return {
    hour: integer(view.getUint8(offset), 0, 23),
    minute: integer(view.getUint8(offset + 1), 0, 59),
    second: integer(view.getUint8(offset + 2), 0, 59),
    nanosecond: integer(view.getUint32(offset + 4, true), 0, 999999999),
  };
}
