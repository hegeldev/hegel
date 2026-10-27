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
  /** The inclusive lower bound. Not together with `minValueExclusive`. */
  minValue?: number;
  /** The inclusive upper bound. Not together with `maxValueExclusive`. */
  maxValue?: number;
  /** A lower bound that is itself excluded: every value is above it. */
  minValueExclusive?: number;
  /** An upper bound that is itself excluded: every value is below it. */
  maxValueExclusive?: number;
  allowNan?: boolean;
  allowInfinity?: boolean;
  /**
   * Whether subnormal ("denormalised") values may be drawn. By default they
   * are whenever the bounds admit any; pass `false` when the code under test
   * may run with flush-to-zero floating point, where a subnormal input
   * silently becomes zero.
   */
  allowSubnormal?: boolean;
}

function subnormalDefault(min: number | undefined, max: number | undefined, normal: number) {
  if (min !== undefined && max !== undefined) {
    if (min === max) return -normal < min && min < normal;
    return min < normal && max > -normal;
  }
  if (min !== undefined) return min < normal;
  if (max !== undefined) return max > -normal;
  return true;
}

class FloatsGenerator extends ValueGenerator<number> {
  private readonly schema: Record<string, unknown>;

  constructor(options?: FloatOptions) {
    super();
    if (options?.minValue !== undefined && options.minValueExclusive !== undefined) {
      throw new Error("Cannot give both minValue and minValueExclusive");
    }
    if (options?.maxValue !== undefined && options.maxValueExclusive !== undefined) {
      throw new Error("Cannot give both maxValue and maxValueExclusive");
    }
    const excludeMin = options?.minValueExclusive !== undefined;
    const excludeMax = options?.maxValueExclusive !== undefined;
    const min = excludeMin ? options.minValueExclusive : options?.minValue;
    const max = excludeMax ? options.maxValueExclusive : options?.maxValue;
    const hasMin = min !== undefined;
    const hasMax = max !== undefined;
    const width = options?.width ?? 64;
    // NaN only when completely unbounded; infinity when at least one side is open
    const allowNan = options?.allowNan ?? (!hasMin && !hasMax);
    const allowInfinity = options?.allowInfinity ?? (!hasMin || !hasMax);

    const normal = width === 32 ? 2 ** -126 : 2 ** -1022;
    const allowSubnormal = options?.allowSubnormal ?? subnormalDefault(min, max, normal);
    if (allowSubnormal) {
      if (min !== undefined && min >= normal) {
        throw new Error(
          `allowSubnormal is true, but ${excludeMin ? "minValueExclusive" : "minValue"} excludes ` +
            `all values below the smallest positive normal ${normal}`,
        );
      }
      if (max !== undefined && max <= -normal) {
        throw new Error(
          `allowSubnormal is true, but ${excludeMax ? "maxValueExclusive" : "maxValue"} excludes ` +
            `all values above the smallest negative normal ${-normal}`,
        );
      }
    } else if (min !== undefined && max !== undefined) {
      const containsZero = min <= 0 && max >= 0;
      if (!containsZero && max < normal && min > -normal) {
        throw new Error(
          `allowSubnormal is false, but that leaves no ${width}-bit floating-point values in [${min}, ${max}]`,
        );
      }
    }

    const schema: Record<string, unknown> = {
      type: "float",
      width,
      allow_nan: allowNan,
      allow_infinity: allowInfinity,
      smallest_nonzero_magnitude: allowSubnormal
        ? width === 32
          ? 2 ** -149
          : Number.MIN_VALUE
        : normal,
    };

    if (hasMin) {
      schema["min_value"] = min;
      schema["exclude_min"] = excludeMin;
    }
    if (hasMax) {
      schema["max_value"] = max;
      schema["exclude_max"] = excludeMax;
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
 * By default, NaN is allowed only when no bounds are set, infinity is
 * allowed when at least one bound is missing, and subnormal values are
 * allowed whenever the bounds admit any. A bound is inclusive (`minValue`,
 * `maxValue`) or exclusive (`minValueExclusive`, `maxValueExclusive`); giving
 * both forms of one bound is an error. Pass `width: 32` for values a
 * `Float32Array` holds exactly.
 *
 * ```ts
 * const unit = gs.floats({ minValue: 0, maxValue: 1 });
 * const positive = gs.floats({ minValueExclusive: 0 });
 * ```
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
