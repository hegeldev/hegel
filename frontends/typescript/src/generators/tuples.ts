/**
 * Tuple generators.
 *
 * @packageDocumentation
 */

import { TestCase } from "../testCase.js";
import { Block, type PrettyPrinter } from "../pretty.js";
import { LabelNames, labelSpec, type LabelSpec } from "../labels.js";
import { PrintableGenerator, composedBasic, type BasicGenerator, type Generator } from "./core.js";

class TuplesGenerator extends PrintableGenerator<unknown[]> {
  private readonly generators: Generator<unknown>[];
  private readonly basic: BasicGenerator<unknown[]> | null;

  constructor(generators: Generator<unknown>[]) {
    super();
    this.generators = generators;

    const basics = generators.map((g) => g.asBasic());
    if (basics.every((b) => b !== null)) {
      const validBasics = basics as BasicGenerator<unknown>[];
      this.basic = composedBasic(
        { type: "tuple", elements: validBasics.map((b) => b.schema) },
        (raw, printer) => {
          if (!Array.isArray(raw)) throw new Error(`Expected array, got ${typeof raw}`);
          const block = new Block(printer, "[", "]");
          const result = raw.map((v: unknown, i: number) =>
            block.item(() => validBasics[i].parseAndPrint(v, printer)),
          );
          block.end();
          return result;
        },
      );
    } else {
      this.basic = null;
    }
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): unknown[] {
    if (this.basic) return this.basic.doDrawAndPrint(tc, printer);
    const block = new Block(printer, "[", "]");
    const result: unknown[] = [];
    for (const generator of this.generators) {
      block.next();
      result.push(tc.drawAndPrint(generator as PrintableGenerator<unknown>, printer));
      block.accept();
    }
    block.end();
    return result;
  }

  override asBasic(): BasicGenerator<unknown[]> | null {
    return this.basic;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.TUPLES, ...this.generators.map((g) => g.label));
  }
}

/**
 * Generate tuples from the given element generators. A tuple prints as an
 * array, each element as its generator prints it.
 */
export function tuples<T extends unknown[]>(
  ...generators: { [K in keyof T]: PrintableGenerator<T[K]> }
): PrintableGenerator<T>;
export function tuples<T extends unknown[]>(
  ...generators: { [K in keyof T]: Generator<T[K]> }
): Generator<T>;
export function tuples<T extends unknown[]>(
  ...generators: { [K in keyof T]: Generator<T[K]> }
): Generator<T> {
  return new TuplesGenerator(generators as Generator<unknown>[]) as unknown as Generator<T>;
}
