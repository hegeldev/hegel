import {
  EngineError,
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
  type StateMachineHandle,
  type StateMachineOptions,
  type NativeDate,
  type NativeTime,
  type NativeDatetime,
  type NativeTimestamp,
  type NativeOffset,
  type NativeSpan,
  type NativeZonedDatetime,
  type NativeTimeZoneInfo,
  type DateOptionsHandle,
  type TimeZoneSpecHandle,
  type ZonedSpecHandle,
  type SpanSpecHandle,
  type NativeFloatOptions,
  type TextGeneratorOptions,
  type UuidVersion,
} from "../engine.js";
import { AssumeError, StopTestError } from "../testCase.js";
import { bigIntToTwosComplementLE, twosComplementLEToBigInt } from "../bytes.js";
import { wtf8ToString } from "../wtf8.js";
import { WasmAbi, u32, u64, type Operation, type Argument } from "./abi.js";
import { WasmArena, WasmMemory } from "./arena.js";

const UINT64_MAX = 0xffffffffffffffffn;

export class WasmEngine implements Engine {
  private readonly handles = new WeakMap<object, number>();
  private readonly memory: WasmMemory;

  constructor(private readonly abi: WasmAbi) {
    this.memory = new WasmMemory(abi.memory);
  }

  private ptr(handle: object): number {
    const ptr = this.handles.get(handle);
    if (ptr === undefined) throw new EngineError("Foreign, freed or invalid Wasm handle");
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
    if (code === 0) return;
    if (control && code === -1) throw new StopTestError();
    if (control && code === -2) throw new AssumeError();
    // Copy before the arena or an owned handle can be released.
    const message = ctx === 0 ? "" : this.readLastError(ctx);
    throw new EngineError(`hegel_${op} failed (${code})${message ? `: ${message}` : ""}`);
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
  freeSettings(s: SettingsHandle): void {
    this.free("settings_free", s);
  }
  setTestCases(s: SettingsHandle, n: number): void {
    this.check(0, "settings_set_test_cases", [0, this.ptr(s), u64(n)]);
  }
  setVerbosity(s: SettingsHandle, v: number): void {
    this.check(0, "settings_set_verbosity", [0, this.ptr(s), u32(v)]);
  }
  setSeed(s: SettingsHandle, seed: bigint): void {
    this.check(0, "settings_set_seed", [0, this.ptr(s), u64(seed), 1]);
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
  runStart(ctx: ContextHandle, settings: SettingsHandle): RunHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "run_start", [c, this.ptr(settings), 0, 0]);
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
  testCaseFromBlob(ctx: ContextHandle, settings: SettingsHandle, blob: string): TestCaseHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "test_case_from_blob", [c, this.ptr(settings), a.text(blob), 0, 0]);
    });
  }
  freeTestCase(tc: TestCaseHandle): void {
    this.free("test_case_free", tc);
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

  /** Run `read` over the engine-owned buffer at `out`, then release it with `free`. */
  private released<T>(
    free: "generate_bytes_result_free" | "owned_string_free",
    out: number,
    read: () => T,
  ): T {
    let result!: T;
    let failure: unknown;
    let failed = false;
    try {
      result = read();
    } catch (error) {
      failed = true;
      failure = error;
    } finally {
      try {
        this.check(0, free, [0, out]);
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
  }

  private copyBuffer<T>(a: WasmArena, out: number, decode: (bytes: Uint8Array) => T): T {
    const data = a.pointer(out),
      length = a.pointer(out + 4);
    if (data === 0) throw new EngineError("Wasm returned a null generated buffer");
    return decode(a.bytes(data, length).slice());
  }

  private buffer<T>(
    ctx: number,
    op: "generate_bytes" | "generate_string" | "generate_timezone",
    args: Argument[],
    decode: (bytes: Uint8Array) => T,
  ): T {
    return WasmArena.scoped(this.abi, (a) => {
      const out = a.alloc(8, 4);
      this.check(ctx, op, [...args, out], true);
      return this.released(
        op === "generate_bytes" ? "generate_bytes_result_free" : "owned_string_free",
        out,
        () => this.copyBuffer(a, out, decode),
      );
    });
  }
  generateBytes(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    minSize: number,
    maxSize?: number,
  ): Uint8Array {
    const c = this.ptr(ctx);
    return this.buffer(
      c,
      "generate_bytes",
      [c, this.ptr(tc), u64(minSize), maxSize === undefined ? UINT64_MAX : u64(maxSize)],
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
  ): StringGeneratorHandle {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      return this.handle(c, "string_generator_regex", [c, a.text(pattern), +fullmatch, 0]);
    });
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

  // The by-value C struct inputs lower to pointers in the pinned raw artifact.
  // Reinspect signatures and fixed-value tests before changing its source/toolchain.
  private temporal<T>(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    op: Operation,
    min: T,
    max: T,
    extra: Argument[],
    layout: Layout<T>,
  ): T {
    return WasmArena.scoped(this.abi, (a) => {
      const lo = this.struct(a, layout, min),
        hi = this.struct(a, layout, max),
        out = a.alloc(layout.size, layout.align);
      const c = this.ptr(ctx);
      this.check(c, op, [c, this.ptr(tc), lo, hi, ...extra, out], true);
      return layout.read(a.view(out, layout.size));
    });
  }
  /** Configure a spec through `op`, whose arguments follow the context and spec pointers. */
  private configure(
    ctx: ContextHandle,
    op: Operation,
    spec: object,
    args: (a: WasmArena) => Argument[],
  ): void {
    WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      this.check(c, op, [c, this.ptr(spec), ...args(a)]);
    });
  }
  private struct<T>(a: WasmArena, layout: Layout<T>, value: T): number {
    const ptr = a.alloc(layout.size, layout.align);
    layout.write(a.view(ptr, layout.size), value);
    return ptr;
  }
  dateOptionsNew(ctx: ContextHandle): DateOptionsHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "date_options_new", [c]);
  }
  dateOptionsSetWeekdays(ctx: ContextHandle, options: DateOptionsHandle, mask: number): void {
    this.configure(ctx, "date_options_set_weekdays", options, () => [u32(mask)]);
  }
  dateOptionsSetMonths(ctx: ContextHandle, options: DateOptionsHandle, mask: number): void {
    this.configure(ctx, "date_options_set_months", options, () => [u32(mask)]);
  }
  freeDateOptions(options: DateOptionsHandle): void {
    this.free("date_options_free", options);
  }
  generateDate(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDate,
    max: NativeDate,
    options: DateOptionsHandle | null,
  ): NativeDate {
    const extra = [options === null ? 0 : this.ptr(options)];
    return this.temporal(ctx, tc, "generate_date", min, max, extra, dateLayout);
  }
  generateTime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeTime,
    max: NativeTime,
    precision: number,
  ): NativeTime {
    return this.temporal(ctx, tc, "generate_time", min, max, [u32(precision)], timeLayout);
  }
  generateDatetime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDatetime,
    max: NativeDatetime,
    precision: number,
  ): NativeDatetime {
    const extra = [u32(precision)];
    return this.temporal(ctx, tc, "generate_datetime", min, max, extra, datetimeLayout);
  }
  generateTimestamp(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeTimestamp,
    max: NativeTimestamp,
    precision: number,
  ): NativeTimestamp {
    const extra = [u32(precision)];
    return this.temporal(ctx, tc, "generate_timestamp", min, max, extra, timestampLayout);
  }
  generateOffset(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: number,
    max: number,
    granularity: number,
    allowUnknown: boolean,
  ): NativeOffset {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_offset",
      [c, this.ptr(tc), i32(min), i32(max), u32(granularity), allowUnknown ? 1 : 0],
      8,
      4,
      (a, p) => ({ seconds: a.view(p, 4).getInt32(0, true), known: a.boolean(p + 4) }),
      true,
    );
  }
  timezoneInfo(ctx: ContextHandle, name: string): NativeTimeZoneInfo {
    return WasmArena.scoped(this.abi, (a) => {
      const c = this.ptr(ctx);
      const out = a.alloc(20, 4);
      this.check(c, "timezone_info", [c, a.text(name), out]);
      const view = a.view(out, 20);
      const known = a.boolean(out);
      return {
        known,
        primaryName: known ? a.string(out + 4) : "",
        isLink: a.boolean(out + 12),
        rank: view.getUint16(14, true),
        tags: view.getUint32(16, true),
      };
    });
  }
  timezoneSpecNew(ctx: ContextHandle): TimeZoneSpecHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "timezone_spec_new", [c]);
  }
  timezoneSpecSetNames(
    ctx: ContextHandle,
    spec: TimeZoneSpecHandle,
    names: readonly string[],
  ): void {
    this.configure(ctx, "timezone_spec_set_names", spec, (a) => [
      a.stringList(names),
      u32(names.length),
    ]);
  }
  freeTimezoneSpec(spec: TimeZoneSpecHandle): void {
    this.free("timezone_spec_free", spec);
  }
  generateTimezone(ctx: ContextHandle, tc: TestCaseHandle, spec: TimeZoneSpecHandle): string {
    const c = this.ptr(ctx);
    return this.buffer(c, "generate_timezone", [c, this.ptr(tc), this.ptr(spec)], wtf8ToString);
  }
  zonedSpecNew(ctx: ContextHandle): ZonedSpecHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "zoned_spec_new", [c]);
  }
  zonedSpecSetCivilBounds(
    ctx: ContextHandle,
    spec: ZonedSpecHandle,
    min: NativeDatetime,
    max: NativeDatetime,
  ): void {
    this.configure(ctx, "zoned_spec_set_civil_bounds", spec, (a) => [
      this.struct(a, datetimeLayout, min),
      this.struct(a, datetimeLayout, max),
    ]);
  }
  zonedSpecSetInstantBounds(
    ctx: ContextHandle,
    spec: ZonedSpecHandle,
    min: NativeTimestamp,
    max: NativeTimestamp,
  ): void {
    this.configure(ctx, "zoned_spec_set_instant_bounds", spec, (a) => [
      this.struct(a, timestampLayout, min),
      this.struct(a, timestampLayout, max),
    ]);
  }
  zonedSpecSetPrecision(ctx: ContextHandle, spec: ZonedSpecHandle, precision: number): void {
    this.configure(ctx, "zoned_spec_set_precision", spec, () => [u32(precision)]);
  }
  zonedSpecSetImaginary(ctx: ContextHandle, spec: ZonedSpecHandle, imaginary: number): void {
    this.configure(ctx, "zoned_spec_set_imaginary", spec, () => [u32(imaginary)]);
  }
  zonedSpecSetAmbiguous(ctx: ContextHandle, spec: ZonedSpecHandle, ambiguous: number): void {
    this.configure(ctx, "zoned_spec_set_ambiguous", spec, () => [u32(ambiguous)]);
  }
  freeZonedSpec(spec: ZonedSpecHandle): void {
    this.free("zoned_spec_free", spec);
  }
  generateZonedDatetime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    tzSpec: TimeZoneSpecHandle,
    zonedSpec: ZonedSpecHandle,
  ): { value: NativeZonedDatetime; zone: string } {
    return WasmArena.scoped(this.abi, (a) => {
      const value = a.alloc(zonedLayout.size, zonedLayout.align),
        zone = a.alloc(8, 4);
      const c = this.ptr(ctx);
      this.check(
        c,
        "generate_zoned_datetime",
        [c, this.ptr(tc), this.ptr(tzSpec), this.ptr(zonedSpec), value, zone],
        true,
      );
      return this.released("owned_string_free", zone, () => ({
        value: zonedLayout.read(a.view(value, zonedLayout.size)),
        zone: this.copyBuffer(a, zone, wtf8ToString),
      }));
    });
  }
  spanSpecNew(ctx: ContextHandle): SpanSpecHandle {
    const c = this.ptr(ctx);
    return this.handle(c, "span_spec_new", [c]);
  }
  spanSpecSetUnitBounds(
    ctx: ContextHandle,
    spec: SpanSpecHandle,
    unit: number,
    min: bigint,
    max: bigint,
  ): void {
    this.configure(ctx, "span_spec_set_unit_bounds", spec, () => [u32(unit), i64(min), i64(max)]);
  }
  spanSpecSetSign(ctx: ContextHandle, spec: SpanSpecHandle, sign: number): void {
    this.configure(ctx, "span_spec_set_sign", spec, () => [u32(sign)]);
  }
  freeSpanSpec(spec: SpanSpecHandle): void {
    this.free("span_spec_free", spec);
  }
  generateSpan(ctx: ContextHandle, tc: TestCaseHandle, spec: SpanSpecHandle): NativeSpan {
    const c = this.ptr(ctx);
    return this.output(
      c,
      "generate_span",
      [c, this.ptr(tc), this.ptr(spec)],
      80,
      8,
      (a, p) => {
        const view = a.view(p, 80);
        const field = (index: number) => view.getBigInt64(index * 8, true);
        return {
          years: field(0),
          months: field(1),
          weeks: field(2),
          days: field(3),
          hours: field(4),
          minutes: field(5),
          seconds: field(6),
          milliseconds: field(7),
          microseconds: field(8),
          nanoseconds: field(9),
        };
      },
      true,
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
  startSpan(ctx: ContextHandle, tc: TestCaseHandle, label: number): void {
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
  freePool(pool: PoolHandle): void {
    this.free("pool_free", pool);
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
}

function integer(value: number, min: number, max: number): number {
  if (!Number.isInteger(value) || value < min || value > max)
    throw new EngineError("Invalid temporal field");
  return value;
}
function i32(value: number): number {
  return integer(value, -0x80000000, 0x7fffffff);
}
function i64(value: bigint): bigint {
  if (value < -0x8000000000000000n || value > 0x7fffffffffffffffn)
    throw new EngineError("Invalid temporal field");
  return value;
}

/** The wasm32 layout of a by-value C struct: its size, alignment and field codec. */
interface Layout<T> {
  size: number;
  align: number;
  write(view: DataView, value: T): void;
  read(view: DataView): T;
}

function writeDate(view: DataView, date: NativeDate): void {
  view.setInt32(0, i32(date.year), true);
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
function writeDatetime(view: DataView, value: NativeDatetime): void {
  writeDate(view, value.date);
  writeTime(view, value.time, 8);
}
function readDatetime(view: DataView): NativeDatetime {
  return { date: readDate(view), time: readTime(view, 8) };
}
function writeTimestamp(view: DataView, value: NativeTimestamp): void {
  view.setBigInt64(0, i64(value.second), true);
  view.setUint32(8, integer(value.nanosecond, 0, 999999999), true);
}
function readTimestamp(view: DataView, offset = 0): NativeTimestamp {
  return {
    second: view.getBigInt64(offset, true),
    nanosecond: integer(view.getUint32(offset + 8, true), 0, 999999999),
  };
}
function readBoolean(view: DataView, offset: number): boolean {
  return integer(view.getUint8(offset), 0, 1) === 1;
}

const dateLayout: Layout<NativeDate> = { size: 8, align: 4, write: writeDate, read: readDate };
const timeLayout: Layout<NativeTime> = {
  size: 8,
  align: 4,
  write: (view, value) => writeTime(view, value),
  read: (view) => readTime(view),
};
const datetimeLayout: Layout<NativeDatetime> = {
  size: 16,
  align: 4,
  write: writeDatetime,
  read: readDatetime,
};
const timestampLayout: Layout<NativeTimestamp> = {
  size: 16,
  align: 8,
  write: writeTimestamp,
  read: (view) => readTimestamp(view),
};
/** `hegel_zoned_datetime_t` is output only: civil at 0, offsets and flags from 16, timestamp at 40. */
const zonedLayout = {
  size: 56,
  align: 8,
  read: (view: DataView): NativeZonedDatetime => ({
    civil: readDatetime(view),
    offsetSeconds: view.getInt32(16, true),
    offsetKnown: readBoolean(view, 20),
    fold: readBoolean(view, 21),
    kind: integer(view.getUint32(24, true), 0, 2),
    offsetBefore: view.getInt32(28, true),
    offsetAfter: view.getInt32(32, true),
    timestamp: readTimestamp(view, 40),
  }),
};
