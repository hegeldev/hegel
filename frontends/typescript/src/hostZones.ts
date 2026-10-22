/**
 * The host's view of time zones, through `Intl.DateTimeFormat`.
 *
 * The engine draws zones from its bundled IANA database; a drawn value is only
 * useful when the host's own database knows the zone too and agrees on the
 * offset, so draws are filtered and checked against `Intl`.
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
  const parts = formatter.formatToParts(epochMilliseconds);
  return parseGmtOffset(
    parts
      .filter((part) => part.type === "timeZoneName")
      .map((part) => part.value)
      .join(""),
  );
}
