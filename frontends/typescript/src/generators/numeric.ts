/**
 * Numeric and boolean generators.
 *
 * @packageDocumentation
 */

import { TestCase, generateRaw } from "../testCase.js";
import { BasicGenerator, ValueGenerator, type PrintableGenerator } from "./core.js";

export interface IntegerOptions {
  minValue?: number;
  maxValue?: number;
}

function parseInteger(raw: unknown): number {
  // The schema interpreter hands back BigInt for values outside the safe
  // integer range, which in-range bounds guarantee cannot happen here; the
  // conversion below is for uniformity with hand-rolled DataSources.
  if (typeof raw === "bigint") return Number(raw);
  return raw as number;
}

class IntegersGenerator extends ValueGenerator<number> {
  private readonly schema: Record<string, unknown>;

  constructor(options?: IntegerOptions) {
    super();
    const min = options?.minValue ?? Number.MIN_SAFE_INTEGER;
    const max = options?.maxValue ?? Number.MAX_SAFE_INTEGER;
    if (min > max) throw new Error("Cannot have maxValue < minValue");
    if (min < Number.MIN_SAFE_INTEGER || max > Number.MAX_SAFE_INTEGER) {
      throw new Error(
        "integers() bounds must be within Number.MIN_SAFE_INTEGER..Number.MAX_SAFE_INTEGER. Use bigIntegers() for larger ranges.",
      );
    }
    this.schema = { type: "integer", min_value: BigInt(min), max_value: BigInt(max) };
  }

  doDraw(tc: TestCase): number {
    return parseInteger(generateRaw(tc, this.schema));
  }

  override asBasic(): BasicGenerator<number> {
    return new BasicGenerator(this.schema, parseInteger);
  }
}

/**
 * Generate integers as JS numbers. Defaults to the safe integer range.
 * Throws if bounds are outside Number.MAX_SAFE_INTEGER.
 * Use bigIntegers() for arbitrary-precision integers.
 */
export function integers(options?: IntegerOptions): PrintableGenerator<number> {
  return new IntegersGenerator(options);
}

export interface BigIntegerOptions {
  minValue?: bigint;
  maxValue?: bigint;
}

function parseBigInt(raw: unknown): bigint {
  if (typeof raw === "bigint") return raw;
  return BigInt(raw as number);
}

const UNBOUNDED_MAGNITUDE = 2n ** 127n;

class BigIntegersGenerator extends ValueGenerator<bigint> {
  private readonly schema: Record<string, unknown>;

  constructor(options?: BigIntegerOptions) {
    super();
    const min = options?.minValue;
    const max = options?.maxValue;
    if (min !== undefined && max !== undefined && min > max) {
      throw new Error("Cannot have maxValue < minValue");
    }
    let lo = min ?? -UNBOUNDED_MAGNITUDE;
    if (min === undefined && max !== undefined && max - UNBOUNDED_MAGNITUDE + 1n < lo) {
      lo = max - UNBOUNDED_MAGNITUDE + 1n;
    }
    let hi = max ?? UNBOUNDED_MAGNITUDE - 1n;
    if (max === undefined && min !== undefined && min + UNBOUNDED_MAGNITUDE - 1n > hi) {
      hi = min + UNBOUNDED_MAGNITUDE - 1n;
    }
    this.schema = { type: "integer", min_value: lo, max_value: hi };
  }

  doDraw(tc: TestCase): bigint {
    return parseBigInt(generateRaw(tc, this.schema));
  }

  override asBasic(): BasicGenerator<bigint> {
    return new BasicGenerator(this.schema, parseBigInt);
  }
}

/**
 * Generate arbitrary-precision integers as BigInt, in `[minValue, maxValue]`.
 *
 * Either bound may be omitted. A missing bound is taken to be `2n ** 127n`
 * values beyond the other bound, or beyond zero when that is further, so
 * `bigIntegers({ minValue: 2n ** 200n })` draws values from `2n ** 200n`
 * upward. Values shrink toward zero, or toward the bound nearest zero.
 */
export function bigIntegers(options?: BigIntegerOptions): PrintableGenerator<bigint> {
  return new BigIntegersGenerator(options);
}

export interface FloatOptions {
  /**
   * The precision of the values: 64 (the default) for any double, 32 for
   * values exactly representable as a 32-bit float (`Math.fround(x) === x`).
   * With width 32, the bounds must be exactly representable too.
   */
  width?: 32 | 64;
  minValue?: number;
  maxValue?: number;
  excludeMin?: boolean;
  excludeMax?: boolean;
  allowNan?: boolean;
  allowInfinity?: boolean;
}

class FloatsGenerator extends ValueGenerator<number> {
  private readonly schema: Record<string, unknown>;

  constructor(options?: FloatOptions) {
    super();
    const hasMin = options?.minValue !== undefined;
    const hasMax = options?.maxValue !== undefined;
    const width = options?.width ?? 64;
    // NaN only when completely unbounded; infinity when at least one side is open
    const allowNan = options?.allowNan ?? (!hasMin && !hasMax);
    const allowInfinity = options?.allowInfinity ?? (!hasMin || !hasMax);

    const schema: Record<string, unknown> = {
      type: "float",
      width,
      allow_nan: allowNan,
      allow_infinity: allowInfinity,
    };

    if (hasMin) {
      schema["min_value"] = options!.minValue;
      schema["exclude_min"] = options?.excludeMin ?? false;
    }
    if (hasMax) {
      schema["max_value"] = options!.maxValue;
      schema["exclude_max"] = options?.excludeMax ?? false;
    }
    this.schema = schema;
  }

  doDraw(tc: TestCase): number {
    return generateRaw(tc, this.schema) as number;
  }

  override asBasic(): BasicGenerator<number> {
    return new BasicGenerator(this.schema);
  }
}

/**
 * Generate floating-point numbers.
 *
 * By default, NaN is allowed only when no bounds are set, and infinity
 * is allowed when at least one bound is missing. Pass `width: 32` for values
 * a `Float32Array` holds exactly.
 */
export function floats(options?: FloatOptions): PrintableGenerator<number> {
  return new FloatsGenerator(options);
}

class BooleansGenerator extends ValueGenerator<boolean> {
  private readonly schema = { type: "boolean" };

  doDraw(tc: TestCase): boolean {
    return generateRaw(tc, this.schema) as boolean;
  }

  override asBasic(): BasicGenerator<boolean> {
    return new BasicGenerator(this.schema);
  }
}

/** Generate boolean values. */
export function booleans(): PrintableGenerator<boolean> {
  return new BooleansGenerator();
}
