/**
 * Client-side interpreter for the generator schema IR.
 *
 * The generators in `src/generators/` describe draws as plain schema records
 * (`{ type: "integer", min_value, max_value }`, `{ type: "list", elements }`,
 * …). Up to libhegel 0.23 these were CBOR-encoded and interpreted inside the
 * engine by a single `hegel_generate` call; from 0.32 the C ABI instead
 * exposes one typed entry point per primitive draw
 * (`hegel_generate_integer`, `hegel_generate_string`, …) and leaves compound
 * structure to the caller. This module walks a schema and drives those typed
 * calls, reproducing the engine's old behavior: compound draws are wrapped in
 * the matching shrinker spans, variable-length draws use the collection
 * protocol, and `one_of` values come back as `[index, value]` pairs.
 *
 * String-shaped draws go through an immutable `hegel_string_generator_t`
 * built from the schema, and the date and time draws through the engine's
 * option and spec handles. Construction is comparatively expensive (regex
 * compilation, Unicode table lookups, zone lists), so the handles are cached
 * per schema object within each run, with at most 256 entries. Eviction and
 * run teardown free the handles; no cache entry can cross engine instances.
 * Grammars (`hegel_grammar_t`, for `fromLark`) are compiled once per run for
 * each generator in the same cache and freed when the run ends.
 *
 * Time zone draws use the schema's zone source, or the default one when it
 * has none (`src/zoneSource.ts`). A zoned draw takes two steps: it draws the
 * name, gives the engine that zone's data if it has not had it yet, and draws
 * the wall-clock time from a spec of that one name, which gives the same
 * choice sequence as one step. Against the host's or the bundled database, a
 * zoned draw whose offset the host's `Intl` disagrees with is rejected like a
 * failed assumption.
 *
 * @packageDocumentation
 */

import { AssumeError, StopTestError } from "./testCase.js";
import { resolveLabel, schemaLabelSpec, type LabelSpec } from "./labels.js";
import {
  EngineError,
  NativeTimeZoneTag,
  type Engine,
  type ContextHandle,
  type TestCaseHandle,
  type StringGeneratorHandle,
  type GrammarHandle,
  type TimeZoneSpecHandle,
  type ZonedSpecHandle,
  type NativeDate,
  type NativeTime,
  type NativeDatetime,
  type NativeTimestamp,
  type NativeZonedDatetime,
  type UuidVersion,
  maxSizeArgument,
} from "./engine.js";

import { fitsInt64 } from "./bytes.js";
import { hostOffsetSeconds } from "./hostZones.js";
import {
  ZoneSource,
  bundledZoneNames,
  checksHostOffsets,
  engineZoneSource,
  loadZone,
  requireZoneName,
} from "./zoneSource.js";

const UINT32_MAX = 0xffffffff;

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** Format 16 big-endian bytes as a canonical lowercase UUID string. */
export function formatUuid(bytes: Uint8Array): string {
  if (bytes.length !== 16) {
    throw new EngineError(`Expected 16 UUID bytes, got ${bytes.length}`);
  }
  const hex = bytesToHex(bytes);
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** Format 4 network-order bytes as a dotted-quad IPv4 address. */
export function formatIpv4(bytes: Uint8Array): string {
  return Array.from(bytes).join(".");
}

/**
 * Format 16 network-order bytes as an RFC 5952 IPv6 address: lowercase hex
 * groups without leading zeros, the leftmost longest run of two or more zero
 * groups compressed to `::`, and the IPv4-mapped range rendered in the
 * conventional `::ffff:a.b.c.d` form.
 */
export function formatIpv6(bytes: Uint8Array): string {
  const groups: number[] = [];
  for (let i = 0; i < 16; i += 2) {
    groups.push((bytes[i] << 8) | bytes[i + 1]);
  }
  if (groups.slice(0, 5).every((g) => g === 0) && groups[5] === 0xffff) {
    return `::ffff:${formatIpv4(bytes.subarray(12))}`;
  }
  // Find the leftmost longest run of >= 2 zero groups to compress.
  let bestStart = -1;
  let bestLength = 1;
  let runStart = -1;
  for (let i = 0; i <= groups.length; i++) {
    if (i < groups.length && groups[i] === 0) {
      if (runStart === -1) {
        runStart = i;
      }
      continue;
    }
    if (runStart !== -1 && i - runStart > bestLength) {
      bestStart = runStart;
      bestLength = i - runStart;
    }
    runStart = -1;
  }
  const hex = groups.map((g) => g.toString(16));
  if (bestStart === -1) {
    return hex.join(":");
  }
  const head = hex.slice(0, bestStart).join(":");
  const tail = hex.slice(bestStart + bestLength).join(":");
  return `${head}::${tail}`;
}

/**
 * A stable identity key for a raw drawn value, used to detect duplicates in
 * `unique` lists and dict keys the way the engine's value equality used to.
 * Numbers and bigints of equal value get the same key (the integer draw
 * downcasts to `number` exactly when the value is safe, so one logical value
 * can surface as either type).
 */
export function valueKey(value: unknown): string {
  if (value === null) {
    return "null";
  }
  if (typeof value === "number" || typeof value === "bigint") {
    return `i:${value.toString()}`;
  }
  if (typeof value === "string") {
    return `s:${JSON.stringify(value)}`;
  }
  if (typeof value === "boolean") {
    return `b:${value}`;
  }
  if (value instanceof Uint8Array) {
    return `x:${bytesToHex(value)}`;
  }
  if (Array.isArray(value)) {
    return `a:${value.map(valueKey).join(",")}`;
  }
  // The only remaining raw value shape is a record (the date and time draws).
  const record = value as Record<string, unknown>;
  const fields = Object.keys(record)
    .sort()
    .map((key) => `${key}=${valueKey(record[key])}`);
  return `o:{${fields.join(",")}}`;
}

/** Downcast a drawn integer to `number` when exact, like the old CBOR decode. */
function toJsInteger(value: bigint): number | bigint {
  if (value >= BigInt(Number.MIN_SAFE_INTEGER) && value <= BigInt(Number.MAX_SAFE_INTEGER)) {
    return Number(value);
  }
  return value;
}

/** Read a required integer bound, accepting the schema's number or bigint. */
function requireIntegerBound(schema: Record<string, unknown>, field: string): bigint {
  const value = schema[field];
  if (value === undefined) {
    throw new Error(`integer schema requires ${field}`);
  }
  return typeof value === "bigint" ? value : BigInt(value as number);
}

function drawInteger(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  schema: Record<string, unknown>,
): number | bigint {
  const min = requireIntegerBound(schema, "min_value");
  const max = requireIntegerBound(schema, "max_value");
  const value = fitsInt64(min, max)
    ? lib.generateInteger(ctx, tc, min, max)
    : lib.generateIntegerBig(ctx, tc, min, max);
  return toJsInteger(value);
}

function drawFloat(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  schema: Record<string, unknown>,
): number {
  return lib.generateFloat(ctx, tc, {
    width: schema["width"] as number,
    minValue: (schema["min_value"] as number | undefined) ?? -Infinity,
    maxValue: (schema["max_value"] as number | undefined) ?? Infinity,
    allowNan: schema["allow_nan"] as boolean,
    allowInfinity: schema["allow_infinity"] as boolean,
    excludeMin: (schema["exclude_min"] as boolean | undefined) ?? false,
    excludeMax: (schema["exclude_max"] as boolean | undefined) ?? false,
    smallestNonzeroMagnitude: schema["smallest_nonzero_magnitude"] as number,
  });
}

/**
 * What a grammar is compiled from: a `fromLark` generator's grammar and
 * options, one object per generator.
 */
export interface GrammarSpec {
  readonly text: string;
  readonly start: string | null;
  /** The most characters, or 0 for the engine's default. */
  readonly maxSize: number;
  /** How deeply rules may nest, or 0 for the engine's default. */
  readonly maxDepth: number;
  /** The schema of the text generator terminals' characters come from. */
  readonly alphabet: Record<string, unknown> | null;
  readonly explicitNames: readonly string[];
  /** The directory imported grammar files are read from. */
  readonly importPath: string | null;
}

/**
 * Reads the grammar file at `path` (slash-separated, relative) under `dir`:
 * its text, or `null` when there is no such file.
 */
export type GrammarFileReader = (dir: string, path: string) => string | null;

function noGrammarFiles(): never {
  throw new EngineError("Reading grammar files (importPath) is not supported on this host");
}

/** An engine handle (or a group of them) built for one schema, with its release. */
export interface CachedHandle<T> {
  readonly value: T;
  free(): void;
}

/** Builds the engine handle the draws of a schema go through. */
export type HandleBuilder<T> = (
  lib: Engine,
  ctx: ContextHandle,
  schema: Record<string, unknown>,
) => CachedHandle<T>;

/** How many schema handles, and how many grammars, a cache keeps alive. */
const CACHE_LIMIT = 256;

/**
 * Run-owned cache of the engine handles draws go through: the string
 * generators and the date option, time zone, zoned datetime and span specs,
 * keyed by schema object, and the grammars of `fromLark` generators, keyed
 * by their spec. Handles never cross engines and are freed before the
 * context. Each kind holds at most {@link CACHE_LIMIT} handles, evicting the
 * oldest first, which bounds runs whose user code creates a fresh schema or
 * `fromLark` generator for every draw. A grammar is never evicted while a
 * draw borrows it: a grammar compiled during that draw (for an explicit
 * terminal) evicts the oldest idle one instead.
 */
export class HandleCache {
  private readonly handles = new Map<Record<string, unknown>, CachedHandle<unknown>>();
  private readonly grammars = new Map<GrammarSpec, GrammarHandle>();
  private readonly borrowed = new Map<GrammarSpec, number>();
  constructor(
    private readonly engine: Engine,
    private readonly readGrammarFile: GrammarFileReader = noGrammarFiles,
  ) {}

  /**
   * The compiled grammar for `spec`, gathering its imported files on first
   * use. It is not evicted until a matching {@link releaseGrammar}.
   */
  borrowGrammar(ctx: ContextHandle, spec: GrammarSpec): GrammarHandle {
    const grammar = this.grammar(ctx, spec);
    this.borrowed.set(spec, (this.borrowed.get(spec) ?? 0) + 1);
    return grammar;
  }

  /** End a {@link borrowGrammar}. */
  releaseGrammar(spec: GrammarSpec): void {
    const count = this.borrowed.get(spec)! - 1;
    if (count === 0) this.borrowed.delete(spec);
    else this.borrowed.set(spec, count);
  }

  private grammar(ctx: ContextHandle, spec: GrammarSpec): GrammarHandle {
    const cached = this.grammars.get(spec);
    if (cached !== undefined) return cached;
    const lib = this.engine;
    if (this.grammars.size >= CACHE_LIMIT) {
      for (const [oldest, handle] of this.grammars) {
        if (this.borrowed.has(oldest)) continue;
        this.grammars.delete(oldest);
        lib.freeGrammar(handle);
        break;
      }
    }
    const builder = lib.grammarBuilderNew(ctx, spec.text);
    const grammar = withCleanup(
      () => {
        for (let index = 0; ; index++) {
          const path = lib.grammarBuilderFile(ctx, builder, index);
          if (path === null) break;
          const text =
            spec.importPath === null ? null : this.readGrammarFile(spec.importPath, path);
          lib.grammarBuilderProvide(ctx, builder, index, text);
        }
        const alphabet =
          spec.alphabet === null ? null : buildStringGenerator(lib, ctx, spec.alphabet);
        return withCleanup(
          () =>
            lib.newGrammar(ctx, builder, {
              start: spec.start,
              maxSize: spec.maxSize,
              maxDepth: spec.maxDepth,
              alphabet,
              explicitNames: spec.explicitNames,
            }),
          () => {
            if (alphabet !== null) lib.freeStringGenerator(alphabet);
          },
        );
      },
      () => lib.freeGrammarBuilder(builder),
    );
    this.grammars.set(spec, grammar);
    return grammar;
  }

  /** The handle `build` makes for `schema`, built on first use. */
  get<T>(
    lib: Engine,
    ctx: ContextHandle,
    schema: Record<string, unknown>,
    build: HandleBuilder<T>,
  ): T {
    if (lib !== this.engine) throw new Error("Handle cache belongs to another engine");
    const cached = this.handles.get(schema);
    if (cached !== undefined) return cached.value as T;
    if (this.handles.size >= CACHE_LIMIT) {
      const oldest = this.handles.entries().next().value!;
      this.handles.delete(oldest[0]);
      oldest[1].free();
    }
    const handle = build(lib, ctx, schema);
    this.handles.set(schema, handle);
    return handle.value;
  }

  dispose(): void {
    const handles = [...this.handles.values()];
    this.handles.clear();
    const grammars = [...this.grammars.values()];
    this.grammars.clear();
    const errors: unknown[] = [];
    for (const handle of handles) {
      try {
        handle.free();
      } catch (error) {
        errors.push(error);
      }
    }
    for (const grammar of grammars) {
      try {
        this.engine.freeGrammar(grammar);
      } catch (error) {
        errors.push(error);
      }
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) throw new AggregateError(errors, "Handle cleanup failed");
  }
}

/** The bundled zones the host's `Intl` knows, with their flags; computed once per engine. */
interface HostZone {
  name: string;
  isLink: boolean;
  legacy: boolean;
}
const hostZonesByEngine = new WeakMap<Engine, HostZone[]>();

function hostZones(lib: Engine, ctx: ContextHandle): HostZone[] {
  let zones = hostZonesByEngine.get(lib);
  if (zones === undefined) {
    zones = bundledZoneNames().map((name) => {
      const info = lib.timezoneInfo(ctx, name);
      return { name, isLink: info.isLink, legacy: (info.tags & NativeTimeZoneTag.LEGACY) !== 0 };
    });
    hostZonesByEngine.set(lib, zones);
  }
  return zones;
}

/** The zone source a `timezone` or `zoned_datetime` schema draws against. */
function sourceOf(schema: Record<string, unknown>): ZoneSource {
  return (schema["source"] as ZoneSource | null) ?? ZoneSource.default();
}

/**
 * Restrict `spec` to `names`, or to the source's names its link and legacy
 * flags allow. The flags are set before the source, which needs a name they
 * allow, and an explicit list allows every name until it replaces them.
 */
function configureTimeZoneSpec(
  lib: Engine,
  ctx: ContextHandle,
  spec: TimeZoneSpecHandle,
  source: ZoneSource,
  names: readonly string[] | null,
  schema: Record<string, unknown>,
): void {
  const links = schema["links"] as boolean;
  const legacy = schema["legacy"] as boolean;
  const handle = engineZoneSource(lib, ctx, source);
  if (handle === null) {
    lib.timezoneSpecSetNames(
      ctx,
      spec,
      names ??
        hostZones(lib, ctx)
          .filter((zone) => (links || !zone.isLink) && (legacy || !zone.legacy))
          .map((zone) => zone.name),
    );
    return;
  }
  lib.timezoneSpecSetLinks(ctx, spec, links);
  lib.timezoneSpecSetLegacy(ctx, spec, legacy);
  if (names !== null) {
    for (const name of names) requireZoneName(lib, ctx, source, name);
  }
  lib.timezoneSpecSetSource(ctx, spec, handle);
  if (names !== null) lib.timezoneSpecSetNames(ctx, spec, names);
}

function newTimeZoneSpec(
  lib: Engine,
  ctx: ContextHandle,
  source: ZoneSource,
  names: readonly string[] | null,
  schema: Record<string, unknown>,
): CachedHandle<TimeZoneSpecHandle> {
  const spec = lib.timezoneSpecNew(ctx);
  const free = () => lib.freeTimezoneSpec(spec);
  withCleanupOnFailure(() => configureTimeZoneSpec(lib, ctx, spec, source, names, schema), free);
  return { value: spec, free };
}

function buildTimeZoneSpec(
  lib: Engine,
  ctx: ContextHandle,
  schema: Record<string, unknown>,
): CachedHandle<TimeZoneSpecHandle> {
  return newTimeZoneSpec(lib, ctx, sourceOf(schema), schema["names"] as string[] | null, schema);
}

/**
 * The handles of a zoned draw: the spec its name is drawn from (`null` when
 * it names one zone), the zoned spec, and a spec of each zone drawn so far.
 */
interface ZonedSpecs {
  source: ZoneSource;
  zones: TimeZoneSpecHandle | null;
  zoned: ZonedSpecHandle;
  single: Map<string, TimeZoneSpecHandle>;
}

function buildZonedSpecs(
  lib: Engine,
  ctx: ContextHandle,
  schema: Record<string, unknown>,
): CachedHandle<ZonedSpecs> {
  const source = sourceOf(schema);
  const names = schema["names"] as string[] | null;
  const zones = names?.length === 1 ? null : newTimeZoneSpec(lib, ctx, source, names, schema);
  const releaseZones = () => zones?.free();
  const zoned = withCleanupOnFailure(() => lib.zonedSpecNew(ctx), releaseZones);
  const single = new Map<string, TimeZoneSpecHandle>();
  const free = () =>
    withCleanup(
      () => lib.freeZonedSpec(zoned),
      () =>
        withCleanup(releaseZones, () => {
          const specs = [...single.values()];
          single.clear();
          for (const spec of specs) lib.freeTimezoneSpec(spec);
        }),
    );
  withCleanupOnFailure(() => {
    const civil = schema["civil"] as { min: NativeDatetime; max: NativeDatetime } | null;
    const instant = schema["instant"] as { min: NativeTimestamp; max: NativeTimestamp } | null;
    if (civil !== null) lib.zonedSpecSetCivilBounds(ctx, zoned, civil.min, civil.max);
    if (instant !== null) lib.zonedSpecSetInstantBounds(ctx, zoned, instant.min, instant.max);
    lib.zonedSpecSetPrecision(ctx, zoned, schema["precision"] as number);
    lib.zonedSpecSetImaginary(ctx, zoned, schema["imaginary"] as number);
    lib.zonedSpecSetAmbiguous(ctx, zoned, schema["ambiguous"] as number);
  }, free);
  return { value: { source, zones: zones?.value ?? null, zoned, single }, free };
}

function buildDateOptions(
  lib: Engine,
  ctx: ContextHandle,
  schema: Record<string, unknown>,
): CachedHandle<ReturnType<Engine["dateOptionsNew"]>> {
  const options = lib.dateOptionsNew(ctx);
  const free = () => lib.freeDateOptions(options);
  withCleanupOnFailure(() => {
    const weekdays = schema["weekdays"] as number | undefined;
    const months = schema["months"] as number | undefined;
    if (weekdays !== undefined) lib.dateOptionsSetWeekdays(ctx, options, weekdays);
    if (months !== undefined) lib.dateOptionsSetMonths(ctx, options, months);
  }, free);
  return { value: options, free };
}

interface SpanUnitBounds {
  unit: number;
  min: bigint;
  max: bigint;
}

function buildSpanSpec(
  lib: Engine,
  ctx: ContextHandle,
  schema: Record<string, unknown>,
): CachedHandle<ReturnType<Engine["spanSpecNew"]>> {
  const spec = lib.spanSpecNew(ctx);
  const free = () => lib.freeSpanSpec(spec);
  withCleanupOnFailure(() => {
    for (const { unit, min, max } of schema["units"] as SpanUnitBounds[]) {
      lib.spanSpecSetUnitBounds(ctx, spec, unit, min, max);
    }
    lib.spanSpecSetSign(ctx, spec, schema["sign"] as number);
  }, free);
  return { value: spec, free };
}

function drawZonedDatetime(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  schema: Record<string, unknown>,
  cache: HandleCache,
): { value: NativeZonedDatetime; zone: string } {
  const specs = cache.get(lib, ctx, schema, buildZonedSpecs);
  const zone =
    specs.zones === null
      ? (schema["names"] as string[])[0]
      : lib.generateTimezone(ctx, tc, specs.zones);
  loadZone(lib, ctx, specs.source, zone);
  let spec = specs.single.get(zone);
  if (spec === undefined) {
    spec = newTimeZoneSpec(lib, ctx, specs.source, [zone], schema).value;
    specs.single.set(zone, spec);
  }
  const drawn = lib.generateZonedDatetime(ctx, tc, spec, specs.zoned);
  if (!checksHostOffsets(specs.source)) return drawn;
  const { timestamp } = drawn.value;
  const epochMilliseconds =
    Number(timestamp.second) * 1000 + Math.floor(timestamp.nanosecond / 1_000_000);
  const hostOffset = hostOffsetSeconds(drawn.zone, epochMilliseconds);
  if (hostOffset !== null && hostOffset !== drawn.value.offsetSeconds) throw new AssumeError();
  return drawn;
}

function utf8OrNull(value: string | undefined): Uint8Array | null {
  return value === undefined ? null : new TextEncoder().encode(value);
}

function stringGeneratorHandle(
  lib: Engine,
  ctx: ContextHandle,
  schema: Record<string, unknown>,
): CachedHandle<StringGeneratorHandle> {
  const generator = buildStringGenerator(lib, ctx, schema);
  return { value: generator, free: () => lib.freeStringGenerator(generator) };
}

function buildStringGenerator(
  lib: Engine,
  ctx: ContextHandle,
  schema: Record<string, unknown>,
): StringGeneratorHandle {
  switch (schema["type"]) {
    case "string": {
      return lib.stringGeneratorText(ctx, {
        minSize: schema["min_size"] as number,
        maxSize: maxSizeArgument(schema["max_size"] as number | undefined),
        codec: (schema["codec"] as string | undefined) ?? null,
        minCodepoint: (schema["min_codepoint"] as number | undefined) ?? 0,
        maxCodepoint: (schema["max_codepoint"] as number | undefined) ?? UINT32_MAX,
        categories: (schema["categories"] as string[] | undefined) ?? null,
        excludeCategories: (schema["exclude_categories"] as string[] | undefined) ?? null,
        includeCharacters: utf8OrNull(schema["include_characters"] as string | undefined),
        excludeCharacters: utf8OrNull(schema["exclude_characters"] as string | undefined),
      });
    }
    case "regex": {
      const alphabetSpec = schema["alphabet"] as Record<string, unknown> | undefined;
      const alphabet =
        alphabetSpec === undefined ? null : buildStringGenerator(lib, ctx, alphabetSpec);
      return withCleanup(
        () =>
          lib.stringGeneratorRegex(
            ctx,
            schema["pattern"] as string,
            schema["fullmatch"] as boolean,
            alphabet,
          ),
        () => {
          if (alphabet !== null) lib.freeStringGenerator(alphabet);
        },
      );
    }
    case "email":
      return lib.stringGeneratorEmail(ctx);
    case "url":
      return lib.stringGeneratorUrl(ctx);
    // The dispatch in generateValue only routes one other type here.
    default:
      return lib.stringGeneratorDomain(ctx, (schema["max_length"] as number | undefined) ?? 255);
  }
}

/** Run `operation`, releasing with `cleanup` only when it throws. */
function withCleanupOnFailure<T>(operation: () => T, cleanup: () => void): T {
  let failed = false;
  return withCleanup(
    () => {
      try {
        return operation();
      } catch (error) {
        failed = true;
        throw error;
      }
    },
    () => {
      if (failed) cleanup();
    },
  );
}

/** Run `operation`, then `cleanup`; cleanup must not replace an engine fault with ordinary draw control flow. */
export function withCleanup<T>(operation: () => T, cleanup: () => void): T {
  let failed = false;
  let failure: unknown;
  let value!: T;
  try {
    value = operation();
  } catch (error) {
    failed = true;
    failure = error;
  } finally {
    try {
      cleanup();
    } catch (error) {
      if (!failed) {
        failed = true;
        failure = error;
      } else if (!(error instanceof AssumeError || error instanceof StopTestError)) {
        failure = new EngineError("Generation cleanup failed", {
          cause: new AggregateError([failure, error]),
        });
      }
    }
  }
  if (failed) throw failure;
  return value;
}

function withSpan<T>(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  spec: LabelSpec,
  operation: () => T,
): T {
  lib.startSpan(ctx, tc, resolveLabel(lib, ctx, spec));
  return withCleanup(operation, () => lib.stopSpan(ctx, tc, false));
}

function drawList(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  schema: Record<string, unknown>,
  cache: HandleCache,
): unknown[] {
  const elementSchema = schema["elements"] as Record<string, unknown>;
  const unique = schema["unique"] as boolean;
  const collection = lib.newCollection(
    ctx,
    tc,
    schema["min_size"] as number,
    schema["max_size"] as number | undefined,
  );
  return withCleanup(
    () => {
      const values: unknown[] = [];
      const seen = new Set<string>();
      while (lib.collectionMore(ctx, tc, collection)) {
        const value = drawChild(lib, ctx, tc, elementSchema, cache);
        if (unique) {
          const key = valueKey(value);
          if (seen.has(key)) {
            lib.collectionReject(ctx, tc, collection, "duplicate element");
            continue;
          }
          seen.add(key);
        }
        values.push(value);
      }
      return values;
    },
    () => lib.freeCollection(collection),
  );
}

function drawDict(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  schema: Record<string, unknown>,
  cache: HandleCache,
): unknown[] {
  const keySchema = schema["keys"] as Record<string, unknown>;
  const valueSchema = schema["values"] as Record<string, unknown>;
  const collection = lib.newCollection(
    ctx,
    tc,
    schema["min_size"] as number,
    schema["max_size"] as number | undefined,
  );
  return withCleanup(
    () => {
      const entries: unknown[] = [];
      const seen = new Set<string>();
      while (lib.collectionMore(ctx, tc, collection)) {
        const key = drawChild(lib, ctx, tc, keySchema, cache);
        const value = drawChild(lib, ctx, tc, valueSchema, cache);
        const identity = valueKey(key);
        if (seen.has(identity)) {
          lib.collectionReject(ctx, tc, collection, "duplicate key");
          continue;
        }
        seen.add(identity);
        entries.push([key, value]);
      }
      return entries;
    },
    () => lib.freeCollection(collection),
  );
}

/**
 * Draw the component `schema` of a compound schema, inside its own span, as
 * a generator drawing a component through the test case would.
 */
function drawChild(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  schema: Record<string, unknown>,
  cache: HandleCache,
): unknown {
  return withSpan(lib, ctx, tc, schemaLabelSpec(schema), () =>
    generateValue(lib, ctx, tc, schema, cache),
  );
}

/**
 * Draw a raw value for the given generator schema by dispatching to the typed
 * `hegel_generate_*` calls. Throws {@link StopTestError} / {@link AssumeError}
 * / {@link LibhegelError} as the underlying draws do.
 */
export function generateValue(
  lib: Engine,
  ctx: ContextHandle,
  tc: TestCaseHandle,
  schema: Record<string, unknown>,
  cache?: HandleCache,
): unknown {
  if (cache === undefined) {
    const owned = new HandleCache(lib);
    return withCleanup(
      () => generateValue(lib, ctx, tc, schema, owned),
      () => owned.dispose(),
    );
  }
  const type = schema["type"] as string;
  switch (type) {
    case "boolean":
      return lib.generateBoolean(ctx, tc, (schema["p"] as number | undefined) ?? 0.5);
    case "integer":
      return drawInteger(lib, ctx, tc, schema);
    case "float":
      return drawFloat(lib, ctx, tc, schema);
    case "binary":
      return lib.generateBytes(
        ctx,
        tc,
        schema["min_size"] as number,
        maxSizeArgument(schema["max_size"] as number | undefined),
      );
    case "string":
    case "regex":
    case "email":
    case "url":
    case "domain":
      return lib.generateString(ctx, tc, cache.get(lib, ctx, schema, stringGeneratorHandle));
    case "ip_address":
      return schema["version"] === 4
        ? formatIpv4(lib.generateIpv4(ctx, tc))
        : formatIpv6(lib.generateIpv6(ctx, tc));
    case "date": {
      const options =
        schema["weekdays"] === undefined && schema["months"] === undefined
          ? null
          : cache.get(lib, ctx, schema, buildDateOptions);
      return lib.generateDate(
        ctx,
        tc,
        schema["min"] as NativeDate,
        schema["max"] as NativeDate,
        options,
      );
    }
    case "time":
      return lib.generateTime(
        ctx,
        tc,
        schema["min"] as NativeTime,
        schema["max"] as NativeTime,
        schema["precision"] as number,
      );
    case "datetime":
      return lib.generateDatetime(
        ctx,
        tc,
        schema["min"] as NativeDatetime,
        schema["max"] as NativeDatetime,
        schema["precision"] as number,
      );
    case "timestamp":
      return lib.generateTimestamp(
        ctx,
        tc,
        schema["min"] as NativeTimestamp,
        schema["max"] as NativeTimestamp,
        schema["precision"] as number,
      );
    case "offset":
      return lib.generateOffset(
        ctx,
        tc,
        schema["min"] as number,
        schema["max"] as number,
        schema["granularity"] as number,
        false,
      ).seconds;
    case "timezone":
      return lib.generateTimezone(ctx, tc, cache.get(lib, ctx, schema, buildTimeZoneSpec));
    case "zoned_datetime":
      return drawZonedDatetime(lib, ctx, tc, schema, cache);
    case "span":
      return lib.generateSpan(ctx, tc, cache.get(lib, ctx, schema, buildSpanSpec));
    case "uuid":
      return formatUuid(lib.generateUuid(ctx, tc, schema["version"] as UuidVersion | undefined));
    case "constant":
      return schema["value"];
    case "one_of": {
      const options = schema["generators"] as Record<string, unknown>[];
      const index = Number(lib.generateInteger(ctx, tc, 0n, BigInt(options.length - 1)));
      return [index, drawChild(lib, ctx, tc, options[index], cache)];
    }
    case "tuple": {
      const elements = schema["elements"] as Record<string, unknown>[];
      return elements.map((element) => drawChild(lib, ctx, tc, element, cache));
    }
    case "list":
      return drawList(lib, ctx, tc, schema, cache);
    case "dict":
      return drawDict(lib, ctx, tc, schema, cache);
    default:
      throw new Error(`Unsupported generator schema type: ${String(type)}`);
  }
}
