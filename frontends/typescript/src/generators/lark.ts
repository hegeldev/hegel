/**
 * Strings of a grammar written in Lark's syntax: fromLark.
 *
 * @packageDocumentation
 */

import type { TestCase } from "../testCase.js";
import type { GrammarSpec } from "../generate.js";
import { LabelNames, labelSpec, type LabelSpec } from "../labels.js";
import { ValueGenerator, type Generator, type PrintableGenerator } from "./core.js";
import { alphabetSchema } from "./strings.js";

/** Options for {@link fromLark}. */
export interface LarkOptions {
  /** The rule to generate strings of (default `start`, as in Lark). */
  start?: string;
  /**
   * Generators for terminals whose values come from the test rather than from
   * the grammar, by terminal name. Required for a terminal the grammar
   * `%declare`s without defining; any other terminal may be overridden.
   *
   * Each occurrence of the terminal in a generated string is a value drawn
   * from its generator. The values are beyond the lexing checks the engine
   * makes on the rest of the string: an `%ignore`d terminal goes on either
   * side of each where the grammar has one, and it is the generator's part to
   * draw values that read as the terminal where they land (a value under a
   * quoted-string terminal should contain no quote, say). The values count
   * toward `maxSize` like any other characters: a string they push over it is
   * drawn again with less room for the grammar's own text, and a draw that
   * cannot fit them after a few tries rejects the test case.
   */
  explicit?: Readonly<Record<string, Generator<string>>>;
  /**
   * The characters terminals drawn from their patterns may contain: a string
   * of them, or a {@link characters} generator.
   */
  alphabet?: string | Generator<string>;
  /** The most characters a string may have (default 8192, at most 1048576). */
  maxSize?: number;
  /** How deeply rules may nest (default 200, at most 500). */
  maxDepth?: number;
  /**
   * The directory `%import`ed grammars are read from. `%import a.b.X` reads
   * `a/b.lark` from it, and Lark's bundled `common`, `python`, `unicode` and
   * `lark` grammars stand in when it has no such file. A relative import
   * (`%import .lib.X`) in `a/b.lark` reads `a/lib.lark`, and in the grammar
   * itself reads `lib.lark`; no bundled grammar stands in for it. Without an
   * `importPath`, only the bundled grammars can be imported. Not supported in
   * the browser, which has no filesystem.
   */
  importPath?: string;
}

function positiveInteger(value: number | undefined, name: string): number {
  if (value === undefined) return 0;
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new RangeError(`${name} must be an integer of at least 1, got ${String(value)}`);
  }
  return value;
}

class LarkGenerator extends ValueGenerator<string> {
  private readonly spec: GrammarSpec;
  private readonly terminals: readonly Generator<string>[];

  constructor(grammar: string, options: LarkOptions) {
    super();
    const explicit = Object.entries(options.explicit ?? {});
    this.terminals = explicit.map(([, generator]) => generator);
    this.spec = {
      text: grammar,
      start: options.start ?? null,
      maxSize: positiveInteger(options.maxSize, "maxSize"),
      maxDepth: positiveInteger(options.maxDepth, "maxDepth"),
      alphabet: options.alphabet === undefined ? null : alphabetSchema(options.alphabet),
      explicitNames: explicit.map(([name]) => name),
      importPath: options.importPath ?? null,
    };
  }

  doDraw(tc: TestCase): string {
    return tc.dataSource().drawGrammar(this.spec, (index) => {
      const terminal = tc.drawSilent(this.terminals[index]);
      if (typeof terminal !== "string") {
        throw new TypeError(
          `explicit terminal ${this.spec.explicitNames[index]} must generate strings, got ${typeof terminal}`,
        );
      }
      return terminal;
    });
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(
      LabelNames.FROM_LARK,
      labelSpec(this.spec.text),
      labelSpec(this.spec.start ?? "start"),
      ...this.spec.explicitNames.flatMap((name, index) => [
        labelSpec(name),
        this.terminals[index].label,
      ]),
    );
  }
}

/**
 * Generate strings matching a grammar written in the syntax of the
 * [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing
 * library, starting from its `start` rule.
 *
 * The whole grammar language is supported: rules, terminals with string and
 * regex patterns and priorities, `%import` (including Lark's bundled
 * `common`, `python`, `unicode` and `lark` grammars), `%ignore`, `%declare`,
 * `%override`, `%extend` and templates. Generated strings are ones Lark's
 * Earley parser accepts for the grammar, with `%ignore`d terminals such as
 * whitespace and comments inserted wherever a lexer would skip them; a string
 * Lark's basic lexer would read differently is avoided where the grammar
 * allows it. A draw that cannot produce such a string after several attempts
 * rejects the test case, as a failed `assume` does.
 *
 * The grammar is checked and compiled the first time a test draws from the
 * generator; a grammar Lark would reject, an undefined start rule, a
 * `maxSize` below the smallest string the grammar can produce, or an
 * `explicit` generator for a terminal the grammar does not use fails the
 * test then. Strings range up to 8192 characters by default, biased toward
 * the larger end; use `maxSize` and `maxDepth` to keep them smaller.
 *
 * @example
 * ```ts
 * const expressions = fromLark(`
 *   ?start: sum
 *   ?sum: product | sum "+" product
 *   ?product: atom | product "*" atom
 *   ?atom: NUMBER | "(" sum ")"
 *   %import common.NUMBER
 *   %import common.WS
 *   %ignore WS
 * `);
 * ```
 */
export function fromLark(grammar: string, options: LarkOptions = {}): PrintableGenerator<string> {
  return new LarkGenerator(grammar, options);
}
