/**
 * Generator base classes, the BasicGenerator descriptor, and the internal
 * combinators.
 *
 * @packageDocumentation
 */

import { TestCase, generateRaw } from "../testCase.js";
import { FILTER_ATTEMPTS } from "../engine.js";
import { PrettyPrinter } from "../pretty.js";
import {
  FILTER_ATTEMPT,
  LabelNames,
  labelSpec,
  schemaLabelSpec,
  type LabelSpec,
} from "../labels.js";

const SILENT = PrettyPrinter.noop();

/**
 * Base class for all generators: an immutable description of how to draw a
 * value of type T from a {@link TestCase}. The same generator can be drawn
 * from any number of times, in any number of tests.
 *
 * A `Generator` draws silently, with {@link TestCase.drawSilent}. To be
 * drawn with {@link TestCase.draw}, which prints what it drew when a test
 * fails, a generator must be a {@link PrintableGenerator}. Every built-in
 * generator is one whenever its components are; a hand-written `Generator`
 * becomes one with {@link printWith} or {@link printAsValue}.
 */
export abstract class Generator<T> {
  private _label: LabelSpec | undefined;

  /**
   * Draw a value from `tc`, printing nothing. A generator drawing from other
   * generators draws them with their own `doDraw`, or with
   * {@link TestCase.drawSilent}.
   */
  abstract doDraw(tc: TestCase): T;

  /**
   * What identifies this generator to the shrinker: the label of the spans
   * it opens, derived by the engine from the generator's kind and its
   * components'. A generator with a schema form is labelled by its schema, so
   * that it has the same label however it is drawn.
   *
   * @internal
   */
  get label(): LabelSpec {
    if (this._label === undefined) {
      const basic = this.asBasic();
      this._label = basic === null ? this.makeLabel() : schemaLabelSpec(basic.schema);
    }
    return this._label;
  }

  /** @internal */
  protected makeLabel(): LabelSpec {
    return labelSpec(LabelNames.GENERATOR);
  }

  /**
   * Return a BasicGenerator descriptor (schema + parse) if this generator can
   * be expressed as one, otherwise null. Used by parent generators that want
   * to compose a schema from their children.
   *
   * @internal
   */
  asBasic(): BasicGenerator<T> | null {
    return null;
  }

  /**
   * Transform generated values using a function. When the source has a
   * schema, the schema is preserved. The result prints the transformed value
   * as {@link PrettyPrinter.value} does; {@link Mapped.printAsCall} prints
   * the source's draw instead.
   */
  map<U>(f: (value: T) => U): Mapped<U, this> {
    return new Mapped<U, this>(this, f as (value: unknown) => U);
  }

  /**
   * Generate a value, then use it to choose another generator. The first
   * value is not printed; the result prints as the chosen generator does,
   * so it is printable when `f` returns printable generators.
   */
  flatMap<U>(f: (value: T) => PrintableGenerator<U>): PrintableGenerator<U>;
  flatMap<U>(f: (value: T) => Generator<U>): Generator<U>;
  flatMap<U>(f: (value: T) => Generator<U>): Generator<U> {
    return new FlatMappedGenerator(this, f);
  }

  /**
   * Only keep values that satisfy the predicate. A draw tries the engine's
   * number of values (3) and rejects the test case if none satisfies it. It
   * prints as this generator does, printing only the accepted value.
   */
  filter(predicate: (value: T) => boolean): Generator<T> {
    return new FilteredGenerator(this, predicate);
  }

  /**
   * Print each drawn value by calling `print` with the value and the printer
   * positioned where it goes: the way to give a generator whose values have
   * no useful JavaScript representation (a function, a handle) a printed
   * form, or to print a value differently from how it prints as a value.
   * The generator keeps this one's label and schema.
   *
   * @example
   * ```ts
   * const digits = integers({ minValue: 0, maxValue: 9 }).printWith((d, p) => p.text(`Digit(${d})`));
   * ```
   */
  printWith(print: (value: T, printer: PrettyPrinter) => void): PrintableGenerator<T> {
    return new PrintedWith(this, print);
  }

  /**
   * Print each drawn value as {@link PrettyPrinter.value} does: the way to
   * make a hand-written generator that only implements {@link doDraw}
   * printable.
   */
  printAsValue(): PrintableGenerator<T> {
    return new PrintedWith(this, (value, printer) => printer.value(value));
  }
}

/**
 * A generator that can print what it draws: drawn with {@link TestCase.draw},
 * it writes the value's representation to the report's printer as it draws
 * it. A container prints element by element through its elements'
 * generators, so an element's own printing shows up inside it.
 *
 * Implement {@link doDrawAndPrint}, drawing inner generators with
 * {@link TestCase.drawAndPrint} so that they print in place, and leave
 * {@link doDraw} to its default, which runs the same body with
 * {@link PrettyPrinter.noop}: both must make exactly the same draws.
 *
 * @example
 * ```ts
 * class Pairs extends generators.PrintableGenerator<[number, number]> {
 *   doDrawAndPrint(tc: TestCase, p: PrettyPrinter): [number, number] {
 *     return p.group(5, "Pair(", ")", () => {
 *       const a = tc.drawAndPrint(generators.integers(), p);
 *       p.separator();
 *       const b = tc.drawAndPrint(generators.integers(), p);
 *       return [a, b];
 *     });
 *   }
 * }
 * ```
 */
export abstract class PrintableGenerator<T> extends Generator<T> {
  /** Draw a value from `tc`, writing its representation to `printer` as it is drawn. */
  abstract doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T;

  /** {@link doDrawAndPrint} with a printer that discards everything. */
  doDraw(tc: TestCase): T {
    return this.doDrawAndPrint(tc, SILENT);
  }

  override filter(predicate: (value: T) => boolean): PrintableGenerator<T> {
    return new FilteredGenerator(this, predicate);
  }
}

/**
 * A generator of finished values, printed as {@link PrettyPrinter.value}
 * prints them: leaves, constants and values built by code.
 *
 * @internal
 */
export abstract class ValueGenerator<T> extends PrintableGenerator<T> {
  abstract override doDraw(tc: TestCase): T;

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    const value = this.doDraw(tc);
    printer.value(value);
    return value;
  }
}

/**
 * Schema-based generator descriptor: a schema the engine draws in one call,
 * and a callback that turns the raw drawn value into a value of type T,
 * printed as {@link PrettyPrinter.value} prints it.
 *
 * Other generators produce a BasicGenerator from `asBasic()` when they can be
 * expressed as a single schema, letting parent generators (oneOf, arrays, ...)
 * compose a unified schema rather than falling back to span-based draws.
 */
export class BasicGenerator<T> extends PrintableGenerator<T> {
  readonly schema: Record<string, unknown>;
  private readonly parse: ((raw: unknown) => T) | null;

  constructor(schema: Record<string, unknown>, parse?: (raw: unknown) => T) {
    super();
    this.schema = schema;
    this.parse = parse ?? null;
  }

  override doDraw(tc: TestCase): T {
    return this.parseRaw(generateRaw(tc, this.schema));
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    return this.parseAndPrint(generateRaw(tc, this.schema), printer);
  }

  override asBasic(): BasicGenerator<T> {
    return this;
  }

  /** The value a raw draw of {@link schema} stands for. */
  parseRaw(raw: unknown): T {
    return this.parse ? this.parse(raw) : (raw as T);
  }

  /** {@link parseRaw}, printing the value as this generator prints it. */
  parseAndPrint(raw: unknown, printer: PrettyPrinter): T {
    const value = this.parseRaw(raw);
    printer.value(value);
    return value;
  }
}

class ComposedBasic<T> extends BasicGenerator<T> {
  constructor(
    schema: Record<string, unknown>,
    private readonly print: (raw: unknown, printer: PrettyPrinter) => T,
  ) {
    super(schema);
  }

  override parseRaw(raw: unknown): T {
    return this.print(raw, SILENT);
  }

  override parseAndPrint(raw: unknown, printer: PrettyPrinter): T {
    return this.print(raw, printer);
  }
}

/**
 * A BasicGenerator composed from other generators' schemas, which prints as
 * `parseAndPrint` does: the same callback parses silently, given a printer
 * that discards everything.
 *
 * @internal
 */
export function composedBasic<T>(
  schema: Record<string, unknown>,
  parseAndPrint: (raw: unknown, printer: PrettyPrinter) => T,
): BasicGenerator<T> {
  return new ComposedBasic(schema, parseAndPrint);
}

/**
 * The result of {@link Generator.map}: prints the transformed value as
 * {@link PrettyPrinter.value} does.
 */
export class Mapped<
  U,
  S extends Generator<unknown> = Generator<unknown>,
> extends PrintableGenerator<U> {
  /** @internal */
  constructor(
    private readonly source: S,
    private readonly f: (value: unknown) => U,
  ) {
    super();
  }

  override doDraw(tc: TestCase): U {
    const sourceBasic = this.source.asBasic();
    if (sourceBasic) {
      return this.f(sourceBasic.doDraw(tc));
    }
    tc.startSpan(this.label);
    const result = this.f(this.source.doDraw(tc));
    tc.stopSpan();
    return result;
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): U {
    const value = this.doDraw(tc);
    printer.value(value);
    return value;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.MAP, this.source.label);
  }

  override asBasic(): BasicGenerator<U> | null {
    const sourceBasic = this.source.asBasic();
    if (!sourceBasic) return null;
    const f = this.f;
    return composedBasic(sourceBasic.schema, (raw, printer) => {
      const value = f(sourceBasic.parseRaw(raw));
      printer.value(value);
      return value;
    });
  }

  /**
   * Print the source's draw as an argument of `name`, instead of the
   * transformed value: `integers().map(makeKey).printAsCall("makeKey")`
   * prints `makeKey(3)`. Needs a printable source.
   */
  printAsCall(this: Mapped<U, PrintableGenerator<unknown>>, name: string): PrintableGenerator<U> {
    if (name === "") throw new Error("printAsCall requires a non-empty function name");
    return new PrintedAsCall(this, this.source, this.f, name);
  }
}

class PrintedAsCall<T, U> extends PrintableGenerator<U> {
  private readonly open: string;

  constructor(
    private readonly mapped: Generator<U>,
    private readonly source: PrintableGenerator<T>,
    private readonly f: (value: T) => U,
    name: string,
  ) {
    super();
    this.open = `${name}(`;
  }

  override get label(): LabelSpec {
    return this.mapped.label;
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): U {
    const sourceBasic = this.source.asBasic();
    if (sourceBasic) return this.call(printer, () => sourceBasic.doDrawAndPrint(tc, printer));
    tc.startSpan(this.label);
    const result = this.call(printer, () => tc.drawAndPrint(this.source, printer));
    tc.stopSpan();
    return result;
  }

  private call(printer: PrettyPrinter, input: () => T): U {
    printer.beginGroup(this.open.length, this.open);
    const value = input();
    printer.endGroup(")");
    return this.f(value);
  }

  override asBasic(): BasicGenerator<U> | null {
    const sourceBasic = this.source.asBasic();
    if (!sourceBasic) return null;
    return composedBasic(sourceBasic.schema, (raw, printer) =>
      this.call(printer, () => sourceBasic.parseAndPrint(raw, printer)),
    );
  }
}

class PrintedWith<T> extends PrintableGenerator<T> {
  constructor(
    private readonly source: Generator<T>,
    private readonly print: (value: T, printer: PrettyPrinter) => void,
  ) {
    super();
  }

  override get label(): LabelSpec {
    return this.source.label;
  }

  override doDraw(tc: TestCase): T {
    return this.source.doDraw(tc);
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    return this.printed(this.source.doDraw(tc), printer);
  }

  private printed(value: T, printer: PrettyPrinter): T {
    if (printer.shouldPrint()) this.print(value, printer);
    return value;
  }

  override asBasic(): BasicGenerator<T> | null {
    const sourceBasic = this.source.asBasic();
    if (!sourceBasic) return null;
    return composedBasic(sourceBasic.schema, (raw, printer) =>
      this.printed(sourceBasic.parseRaw(raw), printer),
    );
  }
}

class FilteredGenerator<T> extends PrintableGenerator<T> {
  constructor(
    private readonly source: Generator<T>,
    private readonly predicate: (value: T) => boolean,
  ) {
    super();
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    for (let i = 0; i < FILTER_ATTEMPTS; i++) {
      tc.startSpan(FILTER_ATTEMPT);
      const speculation = printer.speculate();
      try {
        const value = tc.drawAndPrint(this.source as PrintableGenerator<T>, printer);
        if (this.predicate(value)) {
          speculation.commit();
          tc.stopSpan();
          return value;
        }
      } finally {
        speculation.abort();
      }
      tc.stopSpan(true);
    }
    return tc.reject();
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.FILTER, this.source.label);
  }
}

class FlatMappedGenerator<T, U> extends PrintableGenerator<U> {
  constructor(
    private readonly source: Generator<T>,
    private readonly f: (value: T) => Generator<U>,
  ) {
    super();
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): U {
    tc.startSpan(this.label);
    const intermediate = this.source.doDraw(tc);
    const next = this.f(intermediate) as PrintableGenerator<U>;
    const result = tc.drawAndPrint(next, printer);
    tc.stopSpan();
    return result;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.FLAT_MAP, this.source.label);
  }
}
