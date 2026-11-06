/**
 * The host's view of time zones, through `Intl.DateTimeFormat`.
 *
 * The host zone source reads its zone data from here: through `Temporal` where
 * the host has it, else by probing `Intl`. Draws against the host's or the
 * bundled database are also checked against `Intl`, which catches a value the
 * two disagree on.
 *
 * @packageDocumentation
 */

/** The instants `Date` and `Intl` can represent: ±100,000,000 days from the epoch, in milliseconds. */
const HOST_INSTANT_LIMIT_MILLISECONDS = 8.64e15;

const formatters = new Map<string, Intl.DateTimeFormat | null>();

/** The host's formatter for `zone`, or `null` when the host's `Intl` does not know it. Cached per zone. */
export function hostZoneFormatter(zone: string): Intl.DateTimeFormat | null {
  let formatter = formatters.get(zone);
  if (formatter === undefined) {
    try {
      formatter = new Intl.DateTimeFormat("en-US-u-nu-latn", {
        timeZone: zone,
        timeZoneName: "longOffset",
      });
    } catch {
      formatter = null;
    }
    formatters.set(zone, formatter);
  }
  return formatter;
}

/** Whether the host's `Intl` knows `zone`. */
export function hostKnowsZone(zone: string): boolean {
  return hostZoneFormatter(zone) !== null;
}

/** `GMT-04:00` or `GMT-00:44:30` to seconds; plain `GMT` (UTC) has no offset part. */
function parseGmtOffset(text: string): number {
  const match = /^GMT([+-])(\d{1,2}):(\d{2})(?::(\d{2}))?$/.exec(text);
  if (match === null) return 0;
  const magnitude =
    Number(match[2]) * 3600 +
    Number(match[3]) * 60 +
    (match[4] === undefined ? 0 : Number(match[4]));
  return match[1] === "-" ? -magnitude : magnitude;
}

/**
 * The UTC offset the host gives `zone` at `epochMilliseconds`, in seconds, or
 * `null` when the instant is outside the host's range. Throws for a zone the
 * host does not know.
 */
export function hostOffsetSeconds(zone: string, epochMilliseconds: number): number | null {
  if (Math.abs(epochMilliseconds) > HOST_INSTANT_LIMIT_MILLISECONDS) return null;
  const formatter = hostZoneFormatter(zone);
  if (formatter === null) throw new Error(`The time zone ${zone} is not known to this host`);
  const text = formatter.format(epochMilliseconds);
  return parseGmtOffset(text.slice(text.lastIndexOf("GMT")));
}

/** The part of a `Temporal.ZonedDateTime` the host source reads. */
export interface TemporalZonedDateTimeLike {
  readonly offsetNanoseconds: number;
  readonly epochMilliseconds: number;
  getTimeZoneTransition(direction: "next"): TemporalZonedDateTimeLike | null;
}

/** The part of the `Temporal` namespace the host source reads. */
export interface TemporalLike {
  Instant: {
    fromEpochMilliseconds(epochMilliseconds: number): {
      toZonedDateTimeISO(timeZone: string): TemporalZonedDateTimeLike;
    };
  };
}

/** The host's `Temporal`, when it has one. */
export function hostTemporal(): TemporalLike | undefined {
  return (globalThis as { Temporal?: TemporalLike }).Temporal;
}

/** The host's offset lookups for one zone, in the shape a zone source oracle takes. */
export interface HostOracle {
  offsetAt(epochSeconds: number): number;
  nextTransition?(epochSeconds: number): number | null;
}

/** `zone` through `Temporal`, which reports each transition exactly. */
export function temporalOracle(temporal: TemporalLike, zone: string): HostOracle {
  const at = (epochSeconds: number) =>
    temporal.Instant.fromEpochMilliseconds(epochSeconds * 1000).toZonedDateTimeISO(zone);
  return {
    offsetAt: (epochSeconds) => at(epochSeconds).offsetNanoseconds / 1e9,
    nextTransition: (epochSeconds) => {
      const next = at(epochSeconds).getTimeZoneTransition("next");
      return next === null ? null : Math.ceil(next.epochMilliseconds / 1000);
    },
  };
}

/** `zone` through `Intl.DateTimeFormat`, which only answers offsets. */
export function intlOracle(zone: string): HostOracle {
  return { offsetAt: (epochSeconds) => hostOffsetSeconds(zone, epochSeconds * 1000)! };
}

/**
 * The IANA release of the host's time zone database where the runtime reports
 * it (Node's `process.versions.tz`), else `""`.
 */
export function hostTzdbVersion(scope: unknown = globalThis): string {
  const runtime = Reflect.get(scope as object, "process") as
    | { versions?: { tz?: string } }
    | undefined;
  return runtime?.versions?.tz ?? "";
}
