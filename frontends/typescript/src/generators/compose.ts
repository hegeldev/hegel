/**
 * Composition generators: composite and record.
 *
 * @packageDocumentation
 */

import { TestCase } from "../testCase.js";
import { Block, propertyKey, type PrettyPrinter } from "../pretty.js";
import { LabelNames, labelSpec, type LabelSpec } from "../labels.js";
import {
  PrintableGenerator,
  ValueGenerator,
  composedBasic,
  type BasicGenerator,
  type Generator,
} from "./core.js";

class ComposedGenerator<T> extends ValueGenerator<T> {
  constructor(private readonly fn: (tc: TestCase) => T) {
    super();
  }

  doDraw(tc: TestCase): T {
    return this.fn(tc);
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.COMPOSITE);
  }
}

/**
 * Create a generator from an imperative function, drawing from `tc`. The
 * draws it makes are not printed: the value it returns prints as
 * {@link PrettyPrinter.value} prints it. To print it some other way, use
 * {@link Generator.printWith}, or write a {@link PrintableGenerator}.
 */
export function composite<T>(fn: (tc: TestCase) => T): PrintableGenerator<T> {
  return new ComposedGenerator(fn);
}

/** An object literal, `{ key: value }`, each field's value drawn by `field`. */
function printFields<T>(
  printer: PrettyPrinter,
  keys: readonly string[],
  field: (index: number) => unknown,
): T {
  const obj: Record<string, unknown> = {};
  const block = new Block(printer, "{", "}", " ");
  for (let i = 0; i < keys.length; i++) {
    block.next();
    printer.text(`${propertyKey(keys[i])}: `);
    obj[keys[i]] = field(i);
    block.accept();
  }
  block.end();
  return obj as T;
}

class RecordGenerator<T extends Record<string, unknown>> extends PrintableGenerator<T> {
  private readonly keys: (keyof T & string)[];
  private readonly generators: Generator<T[keyof T & string]>[];
  private readonly basic: BasicGenerator<T> | null;

  constructor(schema: { [K in keyof T]: Generator<T[K]> }) {
    super();
    this.keys = Object.keys(schema) as (keyof T & string)[];
    this.generators = this.keys.map((k) => schema[k]) as Generator<T[keyof T & string]>[];

    const basics = this.generators.map((g) => g.asBasic());
    if (basics.every((b) => b !== null)) {
      const validBasics = basics as BasicGenerator<unknown>[];
      const keys = this.keys;
      this.basic = composedBasic(
        { type: "tuple", elements: validBasics.map((b) => b.schema) },
        (raw, printer) => {
          if (!Array.isArray(raw)) throw new Error(`Expected array, got ${typeof raw}`);
          return printFields(printer, keys, (i) => validBasics[i].parseAndPrint(raw[i], printer));
        },
      );
    } else {
      this.basic = null;
    }
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    if (this.basic) return this.basic.doDrawAndPrint(tc, printer);
    tc.startSpan(this.label);
    const obj = printFields<T>(printer, this.keys, (i) =>
      tc.drawAndPrint(this.generators[i] as PrintableGenerator<unknown>, printer),
    );
    tc.stopSpan();
    return obj;
  }

  override asBasic(): BasicGenerator<T> | null {
    return this.basic;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.FIXED_DICTS, ...this.generators.map((g) => g.label));
  }
}

/**
 * Generate plain objects by drawing each field from its generator.
 * Uses the schema-based path (tuple schema) when all fields have asBasic(),
 * otherwise falls back to composite draws with a FIXED_DICT span.
 *
 * @example
 * ```ts
 * const userGen = record({
 *   name: text({ minSize: 1 }),
 *   age: integers({ minValue: 0, maxValue: 120 }),
 *   active: booleans(),
 * });
 * ```
 *
 * A record prints as an object literal, each field as its generator prints it.
 */
export function record<T extends Record<string, unknown>>(schema: {
  [K in keyof T]: PrintableGenerator<T[K]>;
}): PrintableGenerator<T>;
export function record<T extends Record<string, unknown>>(schema: {
  [K in keyof T]: Generator<T[K]>;
}): Generator<T>;
export function record<T extends Record<string, unknown>>(schema: {
  [K in keyof T]: Generator<T[K]>;
}): Generator<T> {
  return new RecordGenerator(schema);
}
