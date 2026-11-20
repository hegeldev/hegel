// Smoke tests run by CI against other supported runtimes (bun and deno)

import assert from "node:assert/strict";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

// 1. A passing property.
hegel.test(
  (tc) => {
    const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
    assert.ok(x >= 0 && x <= 100, `expected 0..100, got ${x}`);
    const b = tc.draw(gs.booleans());
    assert.equal(typeof b, "boolean");
    const s = tc.draw(gs.text({ maxSize: 10 }));
    assert.equal(typeof s, "string");
  },
  { testCases: 25 },
);

// 2. A failing property.
let caught = null;
try {
  hegel.test(
    (tc) => {
      const arr = tc.draw(gs.arrays(gs.integers({ minValue: 0, maxValue: 100 }), { maxSize: 10 }));
      if (arr.some((x) => x > 50)) {
        throw new Error("found big number");
      }
    },
    { testCases: 200 },
  );
} catch (e) {
  caught = e;
}
assert.ok(caught, "expected property to fail");
assert.match(
  caught.message,
  /found big number/,
  `expected 'found big number' in error message, got: ${caught.message}`,
);

// 3. assume().
hegel.test(
  (tc) => {
    const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
    tc.assume(x > 50);
    assert.ok(x > 50);
  },
  { testCases: 25 },
);

// 4. A stateful test, with a pool.
let stepsRun = 0;
hegel.test(
  (tc) => {
    const handles = new hegel.stateful.Pool(tc);
    hegel.stateful.run(
      tc,
      {
        rules: {
          alloc: (_tc, state) => {
            const handle = state.nextHandle++;
            handles.add(handle);
            state.live.add(handle);
            stepsRun++;
          },
          free: (tc, state) => {
            const handle = tc.draw(handles.valuesConsumed());
            assert.ok(state.live.has(handle), `freed unknown handle ${handle}`);
            state.live.delete(handle);
            stepsRun++;
          },
        },
        invariants: {
          liveMatchesPool: (_tc, state) => {
            assert.equal(state.live.size, handles.size);
          },
        },
      },
      { live: new Set(), nextHandle: 0 },
    );
  },
  { testCases: 25 },
);
assert.ok(stepsRun > 0, "expected the state machine to run some steps");

// 5. Zoned draws against the host's time zone database: read through
// `Temporal` where the runtime has it, and by probing `Intl` with it hidden.
const gap = { year: 2024, month: 3, day: 31, hour: 2, minute: 30 };
function drawGap(zone, text) {
  const zones = gs.temporal.zones({ names: [zone] });
  hegel.test(
    (tc) => {
      const zoned = tc.draw(gs.temporal.zonedDatetimes({ zones, minValue: gap, maxValue: gap }));
      assert.equal(zoned.kind, "gap");
      assert.equal(zoned.offsetBefore, "+01:00");
      assert.equal(zoned.offsetAfter, "+02:00");
      assert.equal(String(zoned), text);
    },
    { testCases: 5 },
  );
}
drawGap("Europe/Paris", "2024-03-31T02:30:00+02:00[Europe/Paris][gap]");
const hostTemporal = Object.getOwnPropertyDescriptor(globalThis, "Temporal");
Reflect.deleteProperty(globalThis, "Temporal");
try {
  drawGap("Europe/Berlin", "2024-03-31T02:30:00+02:00[Europe/Berlin][gap]");
} finally {
  if (hostTemporal !== undefined) Object.defineProperty(globalThis, "Temporal", hostTemporal);
}
hegel.test(
  (tc) => {
    const zoned = tc.draw(gs.temporal.zonedDatetimes());
    assert.match(zoned.offset, /^[+-]\d{2}:\d{2}(:\d{2})?$/);
    assert.ok(String(zoned).includes(`${zoned.offset}[${zoned.timeZone}]`), String(zoned));
  },
  { testCases: 25 },
);
