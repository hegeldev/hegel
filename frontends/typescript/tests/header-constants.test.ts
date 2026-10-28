import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";
import { FILTER_ATTEMPTS, MAX_SIZE_DEFAULT, STATE_MACHINE_DONE } from "../src/engine.js";

const header = readFileSync(new URL("../../../libhegel/include/hegel.h", import.meta.url), "utf8");

const C_LIMITS: Record<string, bigint> = {
  INT64_MIN: -(2n ** 63n),
  UINT64_MAX: 2n ** 64n - 1n,
};

function macro(name: string): bigint {
  const match = new RegExp(`^#define ${name} (.+)$`, "m").exec(header);
  expect(match, name).not.toBe(null);
  const expression = match![1].trim().replace(/^\((.*)\)$/, "$1");
  const offset = /^(\w+)(?: - (\d+))?$/.exec(expression)!;
  const base = C_LIMITS[offset[1]] ?? BigInt(offset[1]);
  return base - BigInt(offset[2] ?? 0);
}

describe("constants copied from hegel.h", () => {
  it.each([
    ["HEGEL_FILTER_ATTEMPTS", BigInt(FILTER_ATTEMPTS)],
    ["HEGEL_MAX_SIZE_DEFAULT", MAX_SIZE_DEFAULT],
    ["HEGEL_STATE_MACHINE_DONE", STATE_MACHINE_DONE],
  ])("%s matches the header", (name, value) => {
    expect(macro(name)).toBe(value);
  });
});
