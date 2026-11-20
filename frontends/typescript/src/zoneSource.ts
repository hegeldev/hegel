/**
 * Zone sources: where the time zone data that zoned draws are classified,
 * targeted and resolved against comes from.
 *
 * A {@link ZoneSource} is a plain JavaScript object; the engine-side source
 * behind it is built once per engine, declares the source's names, and
 * receives a zone's data the first time a draw lands on that zone. That
 * upload happens in JavaScript on every runtime (the browser's Wasm build has
 * no callbacks), so the native and Wasm paths draw identically.
 *
 * @packageDocumentation
 */

import {
  NativeZoneAfter,
  NativeZoneRuleDay,
  type Engine,
  type ContextHandle,
  type ZoneSourceHandle,
  type NativeZoneData,
  type NativeZoneRuleDate,
  type NativeZoneTransition,
  type NativeZoneType,
} from "./engine.js";
import { daysFromCivil, integerField } from "./calendar.js";
import {
  hostKnowsZone,
  hostTemporal,
  hostTzdbVersion,
  intlOracle,
  temporalOracle,
} from "./hostZones.js";
import { TIMEZONE_NAMES } from "./timezone-names.js";
import { entryEngine } from "./runtime.js";

/** A local time type: an offset from UTC, with its daylight saving flag and abbreviation. */
export interface ZoneType {
  /** Seconds east of UTC, less than two days in magnitude. */
  offsetSeconds: number;
  /** Whether this is daylight saving time. Informational only; defaults to `false`. */
  isDst?: boolean;
  /** Such as `"EST"`. Defaults to `""`. */
  abbreviation?: string;
}

/** From `instant` (seconds since the Unix epoch) on, `type` is in force. */
export interface ZoneTransition {
  instant: number | bigint;
  type: ZoneType;
}

/**
 * The day of a yearly rule's transition, as a POSIX TZ string names it
 * (RFC 9636 §3.3.1): weekday `weekday` (0 is Sunday) of week `week` (1 to 5,
 * 5 meaning the last) of month `month`; day `julianDay` of the year in
 * `[1, 365]`, never counting February 29; or day `dayOfYear` in `[0, 365]`,
 * counting it. The transition happens `time` seconds (02:00 by default, up to
 * ±167 hours) after that day's local midnight, in the offset in force before
 * it.
 */
export type RuleDate =
  | { month: number; week: number; weekday: number; time?: number }
  | { julianDay: number; time?: number }
  | { dayOfYear: number; time?: number };

/** A yearly rule: `standard` time, and daylight saving time from `dst.start` to `dst.end`. */
export interface ZoneRule {
  standard: ZoneType;
  dst?: { type: ZoneType; start: RuleDate; end: RuleDate };
}

/**
 * A zone's data as fields: the type in force before its first transition, the
 * transitions in ascending order, and what follows the last of them. That is
 * the last type held for ever, unless `rule` gives a yearly rule or
 * `inferAfter` names the last year the transitions cover, from whose last two
 * years the engine infers one.
 */
export interface ZoneData {
  initial: ZoneType;
  transitions?: readonly ZoneTransition[];
  rule?: ZoneRule;
  inferAfter?: number;
}

/**
 * A library that answers offset lookups but does not expose its zone data.
 * It is probed into transitions over the years `firstYear` (1800 by default)
 * to `lastYear` (2100 by default): with `nextTransition`, transition by
 * transition; without it, once a day, bisecting each change to the second.
 * Before `firstYear` the type at its start holds; after `lastYear`, `after`
 * decides: `"infer"` (the default) infers a yearly rule from the last two
 * years, `"hold"` keeps the last type.
 */
export interface ZoneOracle {
  /** The offset in seconds, or the whole local time type, in force at `epochSeconds`. */
  offsetAt(epochSeconds: number): number | ZoneType;
  /** The first instant after `epochSeconds` at which the type changes, or `null` for none. */
  nextTransition?(epochSeconds: number): number | null;
  firstYear?: number;
  lastYear?: number;
  after?: "hold" | "infer";
}

/**
 * One zone's data in any of the shapes a zone source takes: the bytes of a
 * TZif file (RFC 9636, version 2 or later, such as one under
 * `/usr/share/zoneinfo`), {@link ZoneData}, a {@link ZoneOracle}, or a link
 * to another name of the same source.
 */
export type ZoneDefinition =
  | { tzif: Uint8Array }
  | { data: ZoneData }
  | { oracle: ZoneOracle }
  | { link: string };

/** Options for {@link ZoneSource.custom}. */
export interface ZoneSourceOptions {
  /** Zones by name. */
  zones?: Readonly<Record<string, ZoneDefinition>>;
  /** More names the source knows, whose data `load` supplies when a draw first needs it. */
  names?: readonly string[];
  /** The data of a name, or `undefined` when the source does not know it; called at most once per name. */
  load?: (name: string) => ZoneDefinition | undefined;
  /** The release of the source's database, for {@link ZoneSource.version}. */
  version?: string;
}

/** What one name uploads to an engine-side source. */
type Upload = { tzif: Uint8Array } | { zone: NativeZoneData } | { link: string };

interface EngineSource {
  handle: ZoneSourceHandle;
  known: Set<string>;
  loaded: Set<string>;
}

interface Internals {
  checksHost: boolean;
  /** What the source is, for the error about a name it does not know. */
  owner: string;
  names(): readonly string[];
  knows(name: string): boolean;
  /** A name's data, or `null` when the source does not know it; `null` itself for the bundled database, which the engine has. */
  upload: ((name: string) => Upload | null) | null;
  version(): string;
  uploads: Map<string, Upload | null>;
  engines: WeakMap<Engine, EngineSource>;
}

const internalsOf = new WeakMap<ZoneSource, Internals>();

function internals(source: ZoneSource): Internals {
  return internalsOf.get(source)!;
}

const DAY = 86_400;
const HOST_FIRST_YEAR = 1800;
const HOST_LAST_YEAR = 2100;
const INTL_SAMPLE_SECONDS = 3 * DAY;
const MAX_PROBED_TRANSITIONS = 100_000;

const NO_TYPE: NativeZoneType = { utoff: 0, isDst: false, abbreviation: "" };
const NO_DATE: NativeZoneRuleDate = { kind: 0, day: 0, month: 0, week: 0, weekday: 0, time: 0 };

const I32_MIN = -0x80000000;
const I32_MAX = 0x7fffffff;

function nativeType(type: ZoneType): NativeZoneType {
  const abbreviation = type.abbreviation ?? "";
  if (typeof abbreviation !== "string") {
    throw new Error(`abbreviation must be a string, got ${String(abbreviation)}`);
  }
  return {
    utoff: integerField("offsetSeconds", type.offsetSeconds, I32_MIN, I32_MAX),
    isDst: type.isDst === true,
    abbreviation,
  };
}

function sameType(a: NativeZoneType, b: NativeZoneType): boolean {
  return a.utoff === b.utoff && a.isDst === b.isDst && a.abbreviation === b.abbreviation;
}

function ruleDate(date: RuleDate): NativeZoneRuleDate {
  const time = integerField("time", date.time ?? 7200, I32_MIN, I32_MAX);
  if ("month" in date) {
    return {
      kind: NativeZoneRuleDay.MONTH_WEEK_DAY,
      day: 0,
      month: integerField("month", date.month, 0, 0xff),
      week: integerField("week", date.week, 0, 0xff),
      weekday: integerField("weekday", date.weekday, 0, 0xff),
      time,
    };
  }
  const julian = "julianDay" in date;
  return {
    kind: julian ? NativeZoneRuleDay.JULIAN : NativeZoneRuleDay.ZERO_BASED,
    day: integerField(
      julian ? "julianDay" : "dayOfYear",
      julian ? date.julianDay : date.dayOfYear,
      0,
      0xffff,
    ),
    month: 0,
    week: 0,
    weekday: 0,
    time,
  };
}

function nativeZoneData(data: ZoneData): NativeZoneData {
  const { rule, inferAfter } = data;
  if (rule !== undefined && inferAfter !== undefined) {
    throw new Error("A zone's data takes a rule or inferAfter, not both");
  }
  const dst = rule?.dst;
  return {
    initial: nativeType(data.initial),
    transitions: (data.transitions ?? []).map((transition) => ({
      instant: BigInt(transition.instant),
      type: nativeType(transition.type),
    })),
    after:
      rule !== undefined
        ? NativeZoneAfter.RULE
        : inferAfter !== undefined
          ? NativeZoneAfter.INFER
          : NativeZoneAfter.HOLD,
    standard: rule === undefined ? NO_TYPE : nativeType(rule.standard),
    hasDst: dst !== undefined,
    dst: dst === undefined ? NO_TYPE : nativeType(dst.type),
    dstStart: dst === undefined ? NO_DATE : ruleDate(dst.start),
    dstEnd: dst === undefined ? NO_DATE : ruleDate(dst.end),
    lastYear:
      inferAfter === undefined ? 0 : integerField("inferAfter", inferAfter, I32_MIN, I32_MAX),
  };
}

/**
 * @internal Probe `oracle` into zone data, sampling every `step` seconds when it has no
 * `nextTransition`.
 */
export function probeOracle(oracle: ZoneOracle, step = DAY): NativeZoneData {
  const after = oracle.after ?? "infer";
  if (after !== "hold" && after !== "infer") {
    throw new Error(`after must be "hold" or "infer", got ${String(after)}`);
  }
  const firstYear = integerField("firstYear", oracle.firstYear ?? HOST_FIRST_YEAR, -999999, 999999);
  const lastYear = integerField("lastYear", oracle.lastYear ?? HOST_LAST_YEAR, firstYear, 999999);
  if (after === "infer" && lastYear === firstYear) {
    throw new Error("Inferring a rule needs a probe range of two years at least");
  }
  const start = daysFromCivil(firstYear, 1, 1) * DAY;
  const last = daysFromCivil(lastYear + 1, 1, 1) * DAY - 1;
  const typeAt = (epochSeconds: number): NativeZoneType => {
    const answer = oracle.offsetAt(epochSeconds);
    return nativeType(typeof answer === "number" ? { offsetSeconds: answer } : answer);
  };
  const transitions: NativeZoneTransition[] = [];
  let current = typeAt(start);
  const initial = current;
  const record = (instant: number, type: NativeZoneType): void => {
    if (transitions.length === MAX_PROBED_TRANSITIONS) {
      throw new Error(`The oracle changes type more than ${MAX_PROBED_TRANSITIONS} times`);
    }
    transitions.push({ instant: BigInt(instant), type });
    current = type;
  };
  let time = start;
  if (oracle.nextTransition !== undefined) {
    for (;;) {
      const next = oracle.nextTransition(time);
      if (next === null || next > last) break;
      if (!(next > time)) {
        throw new Error(`nextTransition(${time}) answered ${next}, which is not later`);
      }
      const type = typeAt(next);
      if (!sameType(type, current)) record(next, type);
      time = next;
    }
  } else {
    while (time < last) {
      let high = Math.min(time + step, last);
      let highType = typeAt(high);
      if (sameType(highType, current)) {
        time = high;
        continue;
      }
      let low = time;
      while (high - low > 1) {
        const middle = low + Math.floor((high - low) / 2);
        const middleType = typeAt(middle);
        if (sameType(middleType, current)) {
          low = middle;
        } else {
          high = middle;
          highType = middleType;
        }
      }
      record(high, highType);
      time = high;
    }
  }
  return {
    initial,
    transitions,
    after: after === "hold" ? NativeZoneAfter.HOLD : NativeZoneAfter.INFER,
    standard: NO_TYPE,
    hasDst: false,
    dst: NO_TYPE,
    dstStart: NO_DATE,
    dstEnd: NO_DATE,
    lastYear,
  };
}

function uploadOf(definition: ZoneDefinition): Upload {
  if ("tzif" in definition) {
    if (!(definition.tzif instanceof Uint8Array)) throw new Error("tzif must be a Uint8Array");
    return { tzif: definition.tzif };
  }
  if ("data" in definition) return { zone: nativeZoneData(definition.data) };
  if ("oracle" in definition) return { zone: probeOracle(definition.oracle) };
  if ("link" in definition && typeof definition.link === "string") return { link: definition.link };
  throw new Error(`A zone is defined by tzif, data, oracle or link, got ${String(definition)}`);
}

/** The bundled names the host's `Intl` knows; worked out once. */
let hostNames: Set<string> | null = null;

function bundledHostNameSet(): Set<string> {
  hostNames ??= new Set(TIMEZONE_NAMES.filter(hostKnowsZone));
  return hostNames;
}

function bundledHostNames(): readonly string[] {
  return [...bundledHostNameSet()];
}

/** @internal The host's data for `zone`: through `Temporal` where there is one, else through `Intl`. */
export function hostZoneData(zone: string): NativeZoneData {
  const temporal = hostTemporal();
  return temporal === undefined
    ? probeOracle(intlOracle(zone), INTL_SAMPLE_SECONDS)
    : probeOracle(temporalOracle(temporal, zone));
}

/**
 * Where the time zone data that {@link zones} and {@link zonedDatetimes}
 * draw against comes from: the transitions they aim at, and how a wall-clock
 * time resolves in each zone. Zone names, their link and legacy flags and the
 * order they shrink in come from the engine's bundled catalogue whatever the
 * source.
 *
 * A generator given no `source` uses {@link ZoneSource.default}, which it
 * reads once, when it is built: {@link ZoneSource.host}, the database
 * `Temporal` and `Intl` use, until {@link ZoneSource.setDefault} changes it.
 * Build a source once, outside the test body: each one keeps the zone data
 * it has read.
 */
export class ZoneSource {
  private static readonly BUNDLED = new ZoneSource({
    checksHost: true,
    owner: "the bundled database, limited to the names this host knows",
    names: bundledHostNames,
    knows: (name) => bundledHostNameSet().has(name),
    upload: null,
    version: () => entryEngine().tzdbVersion(),
  });

  private static readonly HOST = new ZoneSource({
    checksHost: true,
    owner: "this host's database",
    names: bundledHostNames,
    knows: hostKnowsZone,
    upload: (name) => ({ zone: hostZoneData(name) }),
    version: () => hostTzdbVersion(),
  });

  private static current: ZoneSource = ZoneSource.HOST;

  private constructor(details: Omit<Internals, "uploads" | "engines">) {
    internalsOf.set(this, { ...details, uploads: new Map(), engines: new WeakMap() });
  }

  /**
   * The engine's bundled IANA database ({@link tzdbVersion}), limited to the
   * names the host's `Intl` knows. Draws against it are checked against
   * `Intl`, and one whose offset `Intl` disagrees with is rejected like a
   * failed assumption.
   */
  static bundled(): ZoneSource {
    return ZoneSource.BUNDLED;
  }

  /**
   * The host's own database: what `Temporal` and `Intl` compute. Each zone's
   * data is read the first time a draw lands on it, once per process: through
   * `Temporal.ZonedDateTime#getTimeZoneTransition` where the host has
   * `Temporal`, exactly, else by probing `Intl.DateTimeFormat` every three
   * days and bisecting each change. Either way it covers 1800 to 2100, and the
   * yearly rule of 2099 and 2100 is inferred for later years. Its names are the
   * bundled names `Intl` knows, plus any other name `Intl` knows when named
   * explicitly. As a safety net beyond that range, a draw whose offset `Intl`
   * disagrees with is rejected like a failed assumption. A `Temporal` without
   * `getTimeZoneTransition` (an implementation of an earlier draft) is not
   * used.
   */
  static host(): ZoneSource {
    return ZoneSource.HOST;
  }

  /**
   * A source of zone data from anywhere, for an integration with a date
   * library: zones given up front, and names whose data a loader supplies
   * the first time a draw needs it.
   *
   * ```ts
   * const source = gs.temporal.ZoneSource.custom({
   *   names: supportedZones,
   *   load: (name) => ({ oracle: { offsetAt: (s) => offsetOf(name, s) } }),
   * });
   * const zoned = gs.temporal.zonedDatetimes({ source });
   * ```
   */
  static custom(options: ZoneSourceOptions = {}): ZoneSource {
    const zones = new Map(Object.entries(options.zones ?? {}));
    const names = [...new Set([...zones.keys(), ...(options.names ?? [])])];
    const { load } = options;
    const version = options.version ?? "";
    if (typeof version !== "string") throw new Error(`version must be a string, got ${version}`);
    const loaded = new Map<string, ZoneDefinition | undefined>();
    const define = (name: string): ZoneDefinition | undefined => {
      if (zones.has(name)) return zones.get(name);
      if (!loaded.has(name)) loaded.set(name, load?.(name));
      return loaded.get(name);
    };
    return new ZoneSource({
      checksHost: false,
      owner: "a custom source",
      names: () => names,
      knows: (name) => define(name) !== undefined,
      upload: (name) => {
        const definition = define(name);
        return definition === undefined ? null : uploadOf(definition);
      },
      version: () => version,
    });
  }

  /**
   * The source a generator built now uses when not given one:
   * {@link ZoneSource.host} unless {@link ZoneSource.setDefault} changed it.
   */
  static default(): ZoneSource {
    return ZoneSource.current;
  }

  /**
   * Make `source` the one generators built from now on use when not given
   * one, and return the default it replaces, to restore later. A generator
   * reads the default when it is built, so one built earlier keeps the
   * source it read.
   */
  static setDefault(source: ZoneSource): ZoneSource {
    if (!(source instanceof ZoneSource)) {
      throw new Error(`setDefault needs a ZoneSource, got ${String(source)}`);
    }
    const previous = ZoneSource.current;
    ZoneSource.current = source;
    return previous;
  }

  /**
   * The release of the source's database: the bundled release for
   * {@link ZoneSource.bundled}, the host's where the runtime reports it
   * (Node's `process.versions.tz`) for {@link ZoneSource.host}, and what was
   * given for a custom source. `""` when unknown.
   */
  get version(): string {
    return internals(this).version();
  }
}

/** @internal Whether `source` knows `name`. */
export function zoneSourceKnows(source: ZoneSource, name: string): boolean {
  return internals(source).knows(name);
}

/** @internal The error for a name `source` does not know. */
export function unknownZone(source: ZoneSource, name: string): Error {
  return new Error(
    `The time zone ${name} is not known to the zone source in use (${internals(source).owner}): pass the zone source that has it as source`,
  );
}

/** @internal Whether draws against `source` are checked against `Intl`. */
export function checksHostOffsets(source: ZoneSource): boolean {
  return internals(source).checksHost;
}

/** @internal The names a time zone draw against the bundled database chooses from. */
export function bundledZoneNames(): readonly string[] {
  return bundledHostNames();
}

function engineSource(lib: Engine, ctx: ContextHandle, details: Internals): EngineSource {
  let state = details.engines.get(lib);
  if (state === undefined) {
    const handle = lib.zoneSourceNew(ctx);
    const names = details.names();
    lib.zoneSourceDeclare(ctx, handle, names);
    state = { handle, known: new Set(names), loaded: new Set() };
    details.engines.set(lib, state);
  }
  return state;
}

/**
 * @internal The engine-side source for `source`, built once per engine, or
 * `null` for the bundled database, which is the engine's own default.
 */
export function engineZoneSource(
  lib: Engine,
  ctx: ContextHandle,
  source: ZoneSource,
): ZoneSourceHandle | null {
  const details = internals(source);
  return details.upload === null ? null : engineSource(lib, ctx, details).handle;
}

/**
 * @internal Declare to the engine-side source of `source` (not the bundled
 * database) the `names` it has not been told of, each of which `source`
 * knows.
 */
export function declareZoneNames(
  lib: Engine,
  ctx: ContextHandle,
  source: ZoneSource,
  names: readonly string[],
): void {
  const state = engineSource(lib, ctx, internals(source));
  const undeclared = names.filter((name) => !state.known.has(name));
  if (undeclared.length === 0) return;
  lib.zoneSourceDeclare(ctx, state.handle, undeclared);
  for (const name of undeclared) state.known.add(name);
}

/** @internal Give the engine-side source `name`'s data, once per engine. */
export function loadZone(lib: Engine, ctx: ContextHandle, source: ZoneSource, name: string): void {
  const details = internals(source);
  const { upload } = details;
  if (upload === null) return;
  const state = engineSource(lib, ctx, details);
  if (state.loaded.has(name)) return;
  let data = details.uploads.get(name);
  if (data === undefined) {
    data = upload(name);
    details.uploads.set(name, data);
  }
  if (data === null) throw unknownZone(source, name);
  if ("link" in data) {
    lib.zoneSourceAddLink(ctx, state.handle, name, data.link);
    state.loaded.add(name);
    state.known.add(name);
    loadZone(lib, ctx, source, data.link);
    return;
  }
  if ("tzif" in data) lib.zoneSourceAddTzif(ctx, state.handle, name, data.tzif);
  else lib.zoneSourceAddZone(ctx, state.handle, name, data.zone);
  state.loaded.add(name);
  state.known.add(name);
}
