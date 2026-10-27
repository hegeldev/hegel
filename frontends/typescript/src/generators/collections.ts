/**
 * Collection generators: arrays, sets, maps and permutations.
 *
 * @packageDocumentation
 */

import { TestCase, Collection, generateRaw } from "../testCase.js";
import { Block, type PrettyPrinter } from "../pretty.js";
import { LabelNames, labelSpec, type LabelSpec } from "../labels.js";
import {
  PrintableGenerator,
  ValueGenerator,
  composedBasic,
  type BasicGenerator,
  type Generator,
} from "./core.js";
import { StructuralKeys } from "./structuralKey.js";

export interface CollectionOptions {
  minSize?: number;
  maxSize?: number;
}

export interface ArrayOptions<T = unknown> extends CollectionOptions {
  /**
   * Require the elements to be distinct. Cannot be combined with `uniqueBy`.
   *
   * Elements are compared structurally: primitives as by `Set` (so `0` and
   * `-0` are equal and `NaN` equals itself), and arrays, plain objects,
   * `Map`s, `Set`s, `Date`s and typed arrays by their contents. Other
   * objects, such as class instances, are compared by identity, so every
   * freshly built one is distinct: give `uniqueBy` a key function for those.
   */
  unique?: boolean;
  /**
   * A key function, or an array of them: for each, the keys of the elements
   * must be distinct, compared structurally as `unique` compares elements
   * (so a key may be an array or an object). A duplicate element is
   * discarded and another drawn in its place.
   */
  uniqueBy?: ((value: T) => unknown) | readonly ((value: T) => unknown)[];
}

/**
 * Print one item of `block` speculatively: `body` prints it and returns
 * whether it is kept, and an item it rejects leaves nothing behind.
 */
function speculateItem(printer: PrettyPrinter, block: Block, body: () => boolean): void {
  const speculation = printer.speculate();
  try {
    block.next();
    if (body()) {
      speculation.commit();
      block.accept();
    }
  } finally {
    speculation.abort();
  }
}

/** A map entry, printed as the `[key, value]` pair that constructs it. */
function printEntry<K, V>(printer: PrettyPrinter, key: () => K, value: () => V): [K, V] {
  const block = new Block(printer, "[", "]");
  const entry: [K, V] = [block.item(key), block.item(value)];
  block.end();
  return entry;
}

// ---------------------------------------------------------------------------
// Arrays
// ---------------------------------------------------------------------------

class ArraysGenerator<T> extends PrintableGenerator<T[]> {
  private readonly elements: Generator<T>;
  private readonly minSize: number;
  private readonly maxSize: number | null;
  private readonly unique: boolean;
  private readonly keys: readonly ((value: T) => unknown)[] | null;
  private readonly basic: BasicGenerator<T[]> | null;

  constructor(elements: Generator<T>, options?: ArrayOptions<T>) {
    super();
    const minSize = options?.minSize ?? 0;
    const maxSize = options?.maxSize ?? null;
    const unique = options?.unique ?? false;
    const uniqueBy = options?.uniqueBy;

    if (maxSize !== null && minSize > maxSize) {
      throw new Error("Cannot have maxSize < minSize");
    }
    if (unique && uniqueBy !== undefined) {
      throw new Error("Cannot pass both unique and uniqueBy");
    }
    const keys = typeof uniqueBy === "function" ? [uniqueBy] : (uniqueBy ?? null);
    if (keys !== null && keys.length === 0) {
      throw new Error("uniqueBy must be a function or a non-empty array of functions");
    }

    this.elements = elements;
    this.minSize = minSize;
    this.maxSize = maxSize;
    this.unique = unique;
    this.keys = keys;

    const elementBasic = elements.asBasic();
    if (elementBasic && keys === null) {
      const schema: Record<string, unknown> = {
        type: "list",
        unique,
        elements: elementBasic.schema,
        min_size: minSize,
      };
      if (maxSize !== null) schema["max_size"] = maxSize;

      this.basic = composedBasic(schema, (raw, printer) => {
        if (!Array.isArray(raw)) throw new Error(`Expected array, got ${typeof raw}`);
        const block = new Block(printer, "[", "]");
        const result = raw.map((v: unknown) =>
          block.item(() => elementBasic.parseAndPrint(v, printer)),
        );
        block.end();
        return result;
      });
    } else {
      this.basic = null;
    }
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T[] {
    if (this.basic) return this.basic.doDrawAndPrint(tc, printer);
    const collection = new Collection(tc, this.minSize, this.maxSize ?? undefined);
    const result: T[] = [];
    const structural = new StructuralKeys();
    const keyFunctions = this.keys ?? (this.unique ? [(element: T) => element] : []);
    const seen = keyFunctions.map(() => new Set<string>());
    const block = new Block(printer, "[", "]");
    while (collection.more()) {
      speculateItem(printer, block, () => {
        const element = tc.drawAndPrint(this.elements as PrintableGenerator<T>, printer);
        const keys = keyFunctions.map((key) => structural.key(key(element)));
        if (keys.some((key, i) => seen[i].has(key))) {
          collection.reject("duplicate element");
          return false;
        }
        keys.forEach((key, i) => seen[i].add(key));
        result.push(element);
        return true;
      });
    }
    block.end();
    return result;
  }

  override asBasic(): BasicGenerator<T[]> | null {
    return this.basic;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.LISTS, this.elements.label);
  }
}

/**
 * Generate arrays with elements from the given generator, with between
 * `minSize` (default 0) and `maxSize` elements (by default the engine decides
 * when to stop). Arrays shrink toward fewer elements. See
 * {@link ArrayOptions} for `unique` and `uniqueBy`.
 *
 * @example
 * ```ts
 * const pairs = arrays(tuples(text(), integers()), { uniqueBy: (pair) => pair[0] });
 * ```
 *
 * An array prints element by element, each as `elements` prints it.
 */
export function arrays<T>(
  elements: PrintableGenerator<T>,
  options?: ArrayOptions<T>,
): PrintableGenerator<T[]>;
export function arrays<T>(elements: Generator<T>, options?: ArrayOptions<T>): Generator<T[]>;
export function arrays<T>(elements: Generator<T>, options?: ArrayOptions<T>): Generator<T[]> {
  return new ArraysGenerator(elements, options);
}

class SetsGenerator<T> extends PrintableGenerator<Set<T>> {
  private readonly elements: Generator<T>;
  private readonly minSize: number;
  private readonly maxSize: number | null;
  private readonly basic: BasicGenerator<Set<T>> | null;

  constructor(elements: Generator<T>, options?: CollectionOptions) {
    super();
    const minSize = options?.minSize ?? 0;
    const maxSize = options?.maxSize ?? null;

    if (maxSize !== null && minSize > maxSize) {
      throw new Error("Cannot have maxSize < minSize");
    }

    this.elements = elements;
    this.minSize = minSize;
    this.maxSize = maxSize;

    const elementBasic = elements.asBasic();
    if (elementBasic) {
      const schema: Record<string, unknown> = {
        type: "list",
        unique: true,
        elements: elementBasic.schema,
        min_size: minSize,
      };
      if (maxSize !== null) schema["max_size"] = maxSize;

      this.basic = composedBasic(schema, (raw, printer) => {
        if (!Array.isArray(raw)) throw new Error(`Expected array, got ${typeof raw}`);
        const result = new Set<T>();
        const block = new Block(printer, "new Set([", "])");
        for (const v of raw) {
          speculateItem(printer, block, () => {
            const element = elementBasic.parseAndPrint(v, printer);
            if (result.has(element)) return false;
            result.add(element);
            return true;
          });
        }
        block.end();
        return result;
      });
    } else {
      this.basic = null;
    }
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): Set<T> {
    if (this.basic) return this.basic.doDrawAndPrint(tc, printer);
    const collection = new Collection(tc, this.minSize, this.maxSize ?? undefined);
    const result = new Set<T>();
    const block = new Block(printer, "new Set([", "])");
    while (collection.more()) {
      speculateItem(printer, block, () => {
        const element = tc.drawAndPrint(this.elements as PrintableGenerator<T>, printer);
        if (result.has(element)) {
          collection.reject("duplicate element");
          return false;
        }
        result.add(element);
        return true;
      });
    }
    block.end();
    return result;
  }

  override asBasic(): BasicGenerator<Set<T>> | null {
    return this.basic;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.SETS, this.elements.label);
  }
}

/**
 * Generate Sets with elements from the given generator. A set prints
 * element by element, each as `elements` prints it.
 */
export function sets<T>(
  elements: PrintableGenerator<T>,
  options?: CollectionOptions,
): PrintableGenerator<Set<T>>;
export function sets<T>(elements: Generator<T>, options?: CollectionOptions): Generator<Set<T>>;
export function sets<T>(elements: Generator<T>, options?: CollectionOptions): Generator<Set<T>> {
  return new SetsGenerator(elements, options);
}

class MapsGenerator<K, V> extends PrintableGenerator<Map<K, V>> {
  private readonly keys: Generator<K>;
  private readonly values: Generator<V>;
  private readonly minSize: number;
  private readonly maxSize: number | null;
  private readonly basic: BasicGenerator<Map<K, V>> | null;

  constructor(keys: Generator<K>, values: Generator<V>, options?: CollectionOptions) {
    super();
    const minSize = options?.minSize ?? 0;
    const maxSize = options?.maxSize ?? null;

    if (maxSize !== null && minSize > maxSize) {
      throw new Error("Cannot have maxSize < minSize");
    }

    this.keys = keys;
    this.values = values;
    this.minSize = minSize;
    this.maxSize = maxSize;

    const keyBasic = keys.asBasic();
    const valueBasic = values.asBasic();

    if (keyBasic && valueBasic) {
      const schema: Record<string, unknown> = {
        type: "dict",
        keys: keyBasic.schema,
        values: valueBasic.schema,
        min_size: minSize,
      };
      if (maxSize !== null) schema["max_size"] = maxSize;

      this.basic = composedBasic(schema, (raw, printer) => {
        if (!Array.isArray(raw)) throw new Error(`Expected array, got ${typeof raw}`);
        const map = new Map<K, V>();
        const block = new Block(printer, "new Map([", "])");
        for (const entry of raw) {
          if (!Array.isArray(entry) || entry.length !== 2) {
            throw new Error("Expected [key, value] pair");
          }
          speculateItem(printer, block, () => {
            const [key, value] = printEntry(
              printer,
              () => keyBasic.parseAndPrint(entry[0], printer),
              () => valueBasic.parseAndPrint(entry[1], printer),
            );
            if (map.has(key)) return false;
            map.set(key, value);
            return true;
          });
        }
        block.end();
        return map;
      });
    } else {
      this.basic = null;
    }
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): Map<K, V> {
    if (this.basic) return this.basic.doDrawAndPrint(tc, printer);
    const collection = new Collection(tc, this.minSize, this.maxSize ?? undefined);
    const result = new Map<K, V>();
    const block = new Block(printer, "new Map([", "])");
    while (collection.more()) {
      speculateItem(printer, block, () => {
        const [key, value] = printEntry(
          printer,
          () => tc.drawAndPrint(this.keys as PrintableGenerator<K>, printer),
          () => tc.drawAndPrint(this.values as PrintableGenerator<V>, printer),
        );
        if (result.has(key)) {
          collection.reject("duplicate key");
          return false;
        }
        result.set(key, value);
        return true;
      });
    }
    block.end();
    return result;
  }

  override asBasic(): BasicGenerator<Map<K, V>> | null {
    return this.basic;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.DICTS, this.keys.label, this.values.label);
  }
}

/**
 * Generate Maps with keys and values from the given generators. A map
 * prints entry by entry, each key and value as its generator prints it.
 * When two keys collide, the first entry is kept.
 */
export function maps<K, V>(
  keys: PrintableGenerator<K>,
  values: PrintableGenerator<V>,
  options?: CollectionOptions,
): PrintableGenerator<Map<K, V>>;
export function maps<K, V>(
  keys: Generator<K>,
  values: Generator<V>,
  options?: CollectionOptions,
): Generator<Map<K, V>>;
export function maps<K, V>(
  keys: Generator<K>,
  values: Generator<V>,
  options?: CollectionOptions,
): Generator<Map<K, V>> {
  return new MapsGenerator(keys, values, options);
}

class PermutationsGenerator<T> extends ValueGenerator<T[]> {
  private readonly elements: readonly T[];

  constructor(elements: readonly T[]) {
    super();
    this.elements = [...elements];
  }

  doDraw(tc: TestCase): T[] {
    const remaining = [...this.elements];
    const result: T[] = [];
    const collection = new Collection(tc, remaining.length, remaining.length);
    while (remaining.length > 0 && collection.more()) {
      const index = generateRaw(tc, {
        type: "integer",
        min_value: 0,
        max_value: remaining.length - 1,
      }) as number;
      result.push(remaining.splice(index, 1)[0]);
    }
    return result;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.PERMUTATION);
  }
}

/**
 * Generate reorderings of `elements`: arrays holding every element exactly
 * once, shrinking toward the original order.
 */
export function permutations<T>(elements: readonly T[]): PrintableGenerator<T[]> {
  return new PermutationsGenerator(elements);
}
