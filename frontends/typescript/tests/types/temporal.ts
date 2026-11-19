import * as gs from "../../src/generators/index.js";
import type { TestCase } from "../../src/testCase.js";

const { Instant } = gs.temporal;

declare const datetime: gs.temporal.PlainDateTime;
declare const zoned: gs.temporal.ZonedDateTime;
declare const Temporal: {
  ZonedDateTime: new (
    epochNanoseconds: bigint,
    timeZone: string,
  ) => { readonly timeZoneId: string };
};

export function deprecatedStringsStayPrintable(tc: TestCase): void {
  const date: string = tc.draw(gs.dates());
  const time: string = tc.draw(gs.times());
  const both: string = tc.draw(gs.datetimes());
  const all: [string, string][] = tc.draw(gs.arrays(gs.tuples(gs.dates(), gs.times())));
  void [date, time, both, all];
}

export function wallClockTimesAreNotInstants(): void {
  // @ts-expect-error a wall-clock datetime names no instant
  gs.temporal.instants({ minValue: datetime });
  // @ts-expect-error a wall-clock datetime names no instant
  gs.temporal.instants({ maxValue: datetime });
  // @ts-expect-error a wall-clock datetime names no instant
  gs.temporal.zonedDatetimes({ minInstant: datetime });
  // @ts-expect-error a wall-clock datetime names no instant
  Instant.from(datetime);
  // @ts-expect-error a wall-clock datetime names no instant
  Instant.compare(datetime, datetime);
  gs.temporal.instants({ minValue: zoned, maxValue: zoned.toInstant() });
  gs.temporal.zonedDatetimes({ minInstant: zoned, maxInstant: 0n });
  gs.temporal.datetimes({ minValue: datetime, maxValue: zoned });
}

export function conversions(tc: TestCase): void {
  const converted: { readonly timeZoneId: string } = tc
    .draw(gs.temporal.zonedDatetimes({ source: gs.temporal.ZoneSource.bundled() }))
    .toTemporal(Temporal);
  const previous: gs.temporal.ZoneSource = gs.temporal.ZoneSource.setDefault(
    gs.temporal.ZoneSource.host(),
  );
  const options: gs.temporal.ZonedDatetimeOptions = {
    zones: gs.temporal.zones({ allowLinks: false, allowLegacy: true }).filter(() => true),
    source: previous,
  };
  const bounds: gs.temporal.UnitBounds = { minValue: 0, maxValue: 1 };
  const plain: gs.temporal.DatetimeOptions = { minValue: datetime };
  void [converted, options, bounds, plain];
}
