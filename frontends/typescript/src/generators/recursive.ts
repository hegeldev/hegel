/**
 * Recursive and forward-referenced generators: recursive and deferred.
 *
 * @packageDocumentation
 */

import type { TestCase } from "../testCase.js";
import type { PrettyPrinter } from "../pretty.js";
import { LabelNames, labelSpec, type LabelSpec } from "../labels.js";
import { Generator, PrintableGenerator } from "./core.js";

const DEFAULT_MAX_DEPTH = 32;
const DEFAULT_MAX_LEAVES = 100;

/**
 * Unwinds a recursive attempt that drew more leaves than its budget, back to
 * the generator that opened `recursionId`: a recursive generator nested in
 * another's sub-values must not catch the outer one's.
 */
class LeafBudgetExceeded extends Error {
  constructor(readonly recursionId: number) {
    super("recursive attempt exceeded its leaf budget");
    this.name = "LeafBudgetExceeded";
  }
}

/** Unwinds a finished recursive attempt the engine discarded as mispriced. */
class AttemptMispriced extends Error {
  constructor() {
    super("recursive attempt was mispriced");
    this.name = "AttemptMispriced";
  }
}

/** Options for {@link recursive}. */
export interface RecursiveOptions {
  /** How deeply branches may nest (default 32); 0 generates only leaves. */
  maxDepth?: number;
  /** The most leaves one value may contain (default 100). */
  maxLeaves?: number;
}

/**
 * The generator a {@link recursive} `extend` function receives: each value it
 * draws is a sub-value of the value under construction, itself a leaf or a
 * further branch.
 */
class SubtreeGenerator<T> extends PrintableGenerator<T> {
  constructor(
    private readonly owner: RecursiveGenerator<T>,
    private readonly recursionId: number,
    private readonly depth: number,
  ) {
    super();
  }

  override get label(): LabelSpec {
    return this.owner.label;
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    tc.startSpan(this.label);
    const value = this.drawSubtree(tc, printer);
    tc.stopSpan();
    return value;
  }

  /** @internal */
  drawSubtree(tc: TestCase, printer: PrettyPrinter): T {
    let value: T;
    if (tc.dataSource().recursionBranch(this.recursionId, this.depth)) {
      const child = new SubtreeGenerator(this.owner, this.recursionId, this.depth + 1);
      value = tc.drawAndPrint(this.owner.extend(child) as PrintableGenerator<T>, printer);
    } else {
      if (!tc.dataSource().recursionLeaf(this.recursionId)) {
        throw new LeafBudgetExceeded(this.recursionId);
      }
      value = tc.drawAndPrint(this.owner.base as PrintableGenerator<T>, printer);
    }
    if (this.depth === 0 && !tc.dataSource().recursionFinish(this.recursionId)) {
      throw new AttemptMispriced();
    }
    return value;
  }
}

class RecursiveGenerator<T> extends PrintableGenerator<T> {
  constructor(
    readonly base: Generator<T>,
    readonly extend: (children: PrintableGenerator<T>) => Generator<T>,
    private readonly maxDepth: number,
    private readonly maxLeaves: number,
  ) {
    super();
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    tc.startSpan(this.label);
    const baseDepth = tc.openSpanDepth;
    const recursionId = tc.dataSource().newRecursion(this.maxDepth, this.maxLeaves);
    for (;;) {
      const speculation = printer.speculate();
      try {
        const value = new SubtreeGenerator(this, recursionId, 0).drawSubtree(tc, printer);
        speculation.commit();
        tc.stopSpan();
        return value;
      } catch (error) {
        if (error instanceof LeafBudgetExceeded && error.recursionId === recursionId) {
          tc.resetOpenSpansTo(baseDepth);
          tc.dataSource().recursionRetry(recursionId);
        } else if (error instanceof AttemptMispriced) {
          tc.resetOpenSpansTo(baseDepth);
        } else {
          throw error;
        }
      } finally {
        speculation.abort();
      }
    }
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.RECURSIVE, this.base.label);
  }
}

function checkCount(name: string, value: number, min: number): number {
  if (!Number.isSafeInteger(value) || value < min) {
    throw new Error(`${name} must be an integer >= ${min}, got ${String(value)}`);
  }
  return value;
}

/**
 * Generate recursively structured data, such as trees or JSON documents,
 * whose leaves come from `base`.
 *
 * `extend` receives a generator of sub-values (each a leaf or a further
 * branch) and returns a generator combining some of them into one compound
 * value; it is called afresh for each branch generated. Values range from a
 * single leaf up to the limits of `maxDepth` (how deeply branches nest,
 * default 32) and `maxLeaves` (the most leaves in one value, default 100: an
 * attempt drawing more is discarded and drawn again, and the test case is
 * rejected when several attempts in a row overrun). The engine decides where
 * values branch, so recursive values are distributed the same way in every
 * Hegel frontend. Values shrink toward a single leaf.
 *
 * A value prints as its leaves and branches do, through `base` and the
 * generators `extend` returns, so it is printable when they are.
 *
 * Throws if `maxDepth` is negative or `maxLeaves` is less than 1. For
 * mutually recursive definitions, use {@link deferred}.
 *
 * @example
 * ```ts
 * type Json = null | boolean | number | string | Json[];
 * const json: Generator<Json> = recursive<Json>(
 *   oneOf<Json>(just(null), booleans(), floats(), text()),
 *   (children) => arrays(children, { maxSize: 5 }),
 * );
 * ```
 */
export function recursive<T>(
  base: PrintableGenerator<T>,
  extend: (children: PrintableGenerator<T>) => PrintableGenerator<T>,
  options?: RecursiveOptions,
): PrintableGenerator<T>;
export function recursive<T>(
  base: Generator<T>,
  extend: (children: PrintableGenerator<T>) => Generator<T>,
  options?: RecursiveOptions,
): Generator<T>;
export function recursive<T>(
  base: Generator<T>,
  extend: (children: PrintableGenerator<T>) => Generator<T>,
  options?: RecursiveOptions,
): Generator<T> {
  const maxDepth = checkCount("maxDepth", options?.maxDepth ?? DEFAULT_MAX_DEPTH, 0);
  const maxLeaves = checkCount("maxLeaves", options?.maxLeaves ?? DEFAULT_MAX_LEAVES, 1);
  return new RecursiveGenerator(base, extend, maxDepth, maxLeaves);
}

const DEFERRED = labelSpec(LabelNames.DEFERRED);
const calculating = new Set<DeferredGenerator<unknown>>();

class DeferredGenerator<T> extends PrintableGenerator<T> {
  private generator: Generator<T> | undefined;
  private resolvedLabel: LabelSpec | undefined;

  constructor(private readonly definition: () => Generator<T>) {
    super();
  }

  private resolve(): Generator<T> {
    if (this.generator === undefined) {
      let generator = this.definition();
      const seen = new Set<Generator<unknown>>([this]);
      while (generator instanceof DeferredGenerator) {
        if (seen.has(generator)) {
          throw new Error("Cannot define a deferred generator as itself");
        }
        seen.add(generator);
        const next = generator as DeferredGenerator<T>;
        generator = next.generator ?? next.definition();
      }
      this.generator = generator;
    }
    return this.generator;
  }

  override get label(): LabelSpec {
    if (this.resolvedLabel !== undefined) return this.resolvedLabel;
    const self = this as DeferredGenerator<unknown>;
    if (calculating.has(self)) return DEFERRED;
    let generator: Generator<T>;
    try {
      generator = this.resolve();
    } catch {
      return DEFERRED;
    }
    calculating.add(self);
    try {
      this.resolvedLabel = generator.label;
    } finally {
      calculating.delete(self);
    }
    return this.resolvedLabel;
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): T {
    return tc.drawAndPrint(this.resolve() as PrintableGenerator<T>, printer);
  }
}

/**
 * A generator defined by a function called the first time it is drawn from
 * (or its label is needed), so a definition can refer to generators defined
 * after it, including itself:
 *
 * ```ts
 * type Tree = null | [Tree, Tree];
 * const tree: Generator<Tree> = deferred(() => oneOf<Tree>(just(null), tuples(tree, tree)));
 * ```
 *
 * A definition that always refers back to itself is cut off by the engine's
 * limit on nested spans, which rejects the test case. A definition that
 * resolves only to itself throws when drawn from. For recursive data with
 * engine-controlled size, prefer {@link recursive}. It prints as the
 * generator `definition` returns does.
 */
export function deferred<T>(definition: () => PrintableGenerator<T>): PrintableGenerator<T>;
export function deferred<T>(definition: () => Generator<T>): Generator<T>;
export function deferred<T>(definition: () => Generator<T>): Generator<T> {
  return new DeferredGenerator(definition);
}
