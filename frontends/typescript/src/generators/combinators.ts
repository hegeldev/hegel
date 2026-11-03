/**
 * Value combinators: just, none, sampledFrom, oneOf, optional.
 *
 * @packageDocumentation
 */

import { TestCase, generateRaw } from "../testCase.js";
import type { PrettyPrinter } from "../pretty.js";
import { LabelNames, labelSpec, type LabelSpec } from "../labels.js";
import {
  BasicGenerator,
  PrintableGenerator,
  ValueGenerator,
  composedBasic,
  type Generator,
} from "./core.js";

class JustGenerator<T> extends ValueGenerator<T> {
  constructor(private readonly value: T) {
    super();
  }

  doDraw(_tc: TestCase): T {
    return this.value;
  }

  override asBasic(): BasicGenerator<T> {
    const value = this.value;
    return new BasicGenerator({ type: "constant", value: null }, () => value);
  }
}

/** Generate a constant value. */
export function just<T>(value: T): PrintableGenerator<T> {
  return new JustGenerator(value);
}

/** Always generate `null`; the same as `just(null)`. */
export function none(): PrintableGenerator<null> {
  return new JustGenerator(null);
}

class SampledFromGenerator<T> extends ValueGenerator<T> {
  private readonly elements: T[];
  private readonly schema: Record<string, unknown>;

  constructor(elements: readonly T[]) {
    super();
    if (elements.length === 0) {
      throw new Error("sampledFrom requires at least one element");
    }
    this.elements = [...elements];
    this.schema = { type: "integer", min_value: 0, max_value: this.elements.length - 1 };
  }

  doDraw(tc: TestCase): T {
    return this.elements[generateRaw(tc, this.schema) as number];
  }

  override asBasic(): BasicGenerator<T> {
    const elements = this.elements;
    return new BasicGenerator(this.schema, (raw) => elements[raw as number]);
  }
}

/**
 * Pick from a fixed list of values, shrinking toward the first. Throws if
 * `elements` is empty.
 */
export function sampledFrom<T>(elements: readonly T[]): PrintableGenerator<T> {
  return new SampledFromGenerator(elements);
}

class OneOfGenerator<T> extends PrintableGenerator<T> {
  private readonly sources: Generator<T>[];
  private readonly basic: BasicGenerator<T> | null;

  constructor(sources: Generator<T>[]) {
    super();
    if (sources.length === 0) {
      throw new Error("oneOf requires at least one generator");
    }
    this.sources = sources;

    const basics = sources.map((g) => g.asBasic());
    if (basics.every((b) => b !== null)) {
      const validBasics = basics as BasicGenerator<T>[];
      const childSchemas = validBasics.map((b) => b.schema);
      this.basic = composedBasic({ type: "one_of", generators: childSchemas }, (raw, printer) => {
        if (!Array.isArray(raw)) throw new Error(`Expected array, got ${typeof raw}`);
        const index = raw[0] as number;
        return validBasics[index].parseAndPrint(raw[1], printer);
      });
    } else {
      this.basic = null;
    }
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    if (this.basic) return this.basic.doDrawAndPrint(tc, printer);
    const index = generateRaw(tc, {
      type: "integer",
      min_value: 0,
      max_value: this.sources.length - 1,
    }) as number;
    return tc.drawAndPrint(this.sources[index] as PrintableGenerator<T>, printer);
  }

  override asBasic(): BasicGenerator<T> | null {
    return this.basic;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.ONE_OF, ...this.sources.map((source) => source.label));
  }
}

/**
 * Choose from multiple generators of the same type. The value prints as the
 * chosen generator prints it.
 */
export function oneOf<T>(...generators: PrintableGenerator<T>[]): PrintableGenerator<T>;
export function oneOf<T>(...generators: Generator<T>[]): Generator<T>;
export function oneOf<T>(...generators: Generator<T>[]): Generator<T> {
  return new OneOfGenerator(generators);
}

class OptionalGenerator<T> extends PrintableGenerator<T | null> {
  constructor(private readonly inner: Generator<T>) {
    super();
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T | null {
    const isSome = generateRaw(tc, { type: "boolean" }) as boolean;
    return isSome
      ? tc.drawAndPrint(this.inner as PrintableGenerator<T>, printer)
      : printNull(printer);
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.OPTIONAL, this.inner.label);
  }
}

function printNull(printer: PrettyPrinter): null {
  printer.text("null");
  return null;
}

/**
 * Generate either a value from the inner generator, printed as the inner
 * generator prints it, or null.
 */
export function optional<T>(inner: PrintableGenerator<T>): PrintableGenerator<T | null>;
export function optional<T>(inner: Generator<T>): Generator<T | null>;
export function optional<T>(inner: Generator<T>): Generator<T | null> {
  return new OptionalGenerator(inner);
}
