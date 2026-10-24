/**
 * Generator base class, BasicGenerator descriptor, and internal combinators.
 *
 * @packageDocumentation
 */

import { TestCase, generateRaw } from "../testCase.js";
import {
  FILTER_ATTEMPT,
  LabelNames,
  labelSpec,
  schemaLabelSpec,
  type LabelSpec,
} from "../labels.js";

/**
 * Base class for all generators: an immutable description of how to draw a
 * value of type T from a {@link TestCase}. The same generator can be drawn
 * from any number of times, in any number of tests.
 */
export abstract class Generator<T> {
  private _label: LabelSpec | undefined;

  /** @internal */
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
   * Transform generated values using a function.
   * When the source has a schema, the schema is preserved.
   */
  map<U>(f: (value: T) => U): Generator<U> {
    return new MappedGenerator(this, f);
  }

  /**
   * Generate a value, then use it to choose another generator.
   */
  flatMap<U>(f: (value: T) => Generator<U>): Generator<U> {
    return new FlatMappedGenerator(this, f);
  }

  /**
   * Only keep values that satisfy the predicate.
   * Retries up to 3 times, then rejects the test case.
   */
  filter(predicate: (value: T) => boolean): Generator<T> {
    return new FilteredGenerator(this, predicate);
  }
}

/**
 * Schema-based generator descriptor. Holds a schema and an optional parse
 * callback that transforms the raw drawn value into a value of type T.
 *
 * Other generators produce a BasicGenerator from `asBasic()` when they can be
 * expressed as a single schema, letting parent generators (oneOf, arrays, ...)
 * compose a unified schema rather than falling back to span-based draws.
 */
export class BasicGenerator<T> extends Generator<T> {
  readonly schema: Record<string, unknown>;
  private readonly parse: ((raw: unknown) => T) | null;

  constructor(schema: Record<string, unknown>, parse?: (raw: unknown) => T) {
    super();
    this.schema = schema;
    this.parse = parse ?? null;
  }

  doDraw(tc: TestCase): T {
    return this.parseRaw(generateRaw(tc, this.schema));
  }

  override asBasic(): BasicGenerator<T> {
    return this;
  }

  parseRaw(raw: unknown): T {
    return this.parse ? this.parse(raw) : (raw as T);
  }
}

class MappedGenerator<T, U> extends Generator<U> {
  constructor(
    private readonly source: Generator<T>,
    private readonly f: (value: T) => U,
  ) {
    super();
  }

  doDraw(tc: TestCase): U {
    const sourceBasic = this.source.asBasic();
    if (sourceBasic) {
      return this.f(sourceBasic.doDraw(tc));
    }
    tc.startSpan(this.label);
    const result = this.f(this.source.doDraw(tc));
    tc.stopSpan();
    return result;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.MAP, this.source.label);
  }

  override asBasic(): BasicGenerator<U> | null {
    const sourceBasic = this.source.asBasic();
    if (!sourceBasic) return null;
    const f = this.f;
    return new BasicGenerator(sourceBasic.schema, (raw) => f(sourceBasic.parseRaw(raw)));
  }
}

class FilteredGenerator<T> extends Generator<T> {
  constructor(
    private readonly source: Generator<T>,
    private readonly predicate: (value: T) => boolean,
  ) {
    super();
  }

  doDraw(tc: TestCase): T {
    for (let i = 0; i < 3; i++) {
      tc.startSpan(FILTER_ATTEMPT);
      const value = this.source.doDraw(tc);
      if (this.predicate(value)) {
        tc.stopSpan();
        return value;
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

class FlatMappedGenerator<T, U> extends Generator<U> {
  constructor(
    private readonly source: Generator<T>,
    private readonly f: (value: T) => Generator<U>,
  ) {
    super();
  }

  doDraw(tc: TestCase): U {
    tc.startSpan(this.label);
    const intermediate = this.source.doDraw(tc);
    const nextGen = this.f(intermediate);
    const result = nextGen.doDraw(tc);
    tc.stopSpan();
    return result;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.FLAT_MAP, this.source.label);
  }
}
