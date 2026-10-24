/**
 * Printing drawn values.
 *
 * A drawn value prints as one line of JavaScript syntax ({@link repr}), the
 * same in every runtime, handed to the engine's printer through
 * `hegel_printer_reflow`: the engine recovers the bracket structure of the
 * line and lays it out like a value printed structurally, so a long value
 * wraps to the document's width.
 *
 * @packageDocumentation
 */

/**
 * A region of a printed document: the engine's printer for a test case, or a
 * {@link TextPrinter} where there is no engine (an explicit example).
 */
export interface Printer {
  /** Literal text, with no newlines. */
  text(text: string): void;
  hardBreak(): void;
  /** A one-line representation, laid out to fit. */
  reflow(text: string): void;
  /**
   * Buffer output until {@link commitSpeculative} keeps it or
   * {@link abortSpeculative} discards it: how a draw that is rejected part
   * way through leaves nothing behind.
   */
  beginSpeculative(): void;
  commitSpeculative(): void;
  abortSpeculative(): void;
}

/**
 * A {@link Printer} that lays nothing out: each value stays on one line. Notes
 * made while a speculative region is open are held until it closes, as the
 * engine's printer does.
 */
export class TextPrinter implements Printer {
  private readonly regions: string[][] = [[]];
  private held: string[] = [];

  private get current(): string[] {
    return this.regions[this.regions.length - 1];
  }

  text(text: string): void {
    this.current.push(text);
  }

  hardBreak(): void {
    this.current.push("\n");
  }

  reflow(text: string): void {
    this.current.push(text);
  }

  beginSpeculative(): void {
    this.regions.push([]);
  }

  commitSpeculative(): void {
    const region = this.regions.pop()!;
    this.current.push(...region);
    this.release();
  }

  abortSpeculative(): void {
    this.regions.pop();
    this.release();
  }

  /** Append a note: each line of `message` becomes its own output line. */
  note(message: string): void {
    const lines = message.split("\n").map((line) => `${line}\n`);
    if (this.regions.length > 1) this.held.push(...lines);
    else this.current.push(...lines);
  }

  private release(): void {
    if (this.regions.length === 1) {
      this.current.push(...this.held);
      this.held = [];
    }
  }

  /** Everything printed, one line per element. */
  lines(): string[] {
    return splitLines(this.regions[0].join(""));
  }
}

/** The lines of a rendered document, without the empty line after its final newline. */
export function splitLines(document: string): string[] {
  if (document === "") return [];
  const lines = document.split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  return lines;
}

const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;

function key(name: string): string {
  return IDENTIFIER.test(name) ? name : JSON.stringify(name);
}

/**
 * `value` as one line of JavaScript syntax: strings double-quoted with
 * JSON escapes, bigints with their `n` suffix, `-0` kept, and arrays, plain
 * objects, `Map`s, `Set`s, typed arrays and `Date`s as the expressions that
 * construct them. Instances of other classes print as their class name and
 * own enumerable properties, and a value that contains itself prints the
 * repeated reference as `[Circular]`.
 */
export function repr(value: unknown): string {
  return reprIn(value, new Set());
}

function reprIn(value: unknown, enclosing: Set<object>): string {
  switch (typeof value) {
    case "string":
      return JSON.stringify(value);
    case "number":
      return Object.is(value, -0) ? "-0" : String(value);
    case "bigint":
      return `${value}n`;
    case "symbol":
      return value.toString();
    case "function":
      return `[Function ${value.name || "(anonymous)"}]`;
    case "object":
      if (value === null) return "null";
      if (enclosing.has(value)) return "[Circular]";
      enclosing.add(value);
      try {
        return reprObject(value, (inner) => reprIn(inner, enclosing));
      } finally {
        enclosing.delete(value);
      }
    default:
      return String(value);
  }
}

function reprObject(value: object, inner: (value: unknown) => string): string {
  const list = (values: Iterable<unknown>): string => Array.from(values, inner).join(", ");
  if (Array.isArray(value)) return `[${list(value)}]`;
  if (value instanceof Map) {
    return `new Map([${Array.from(value, ([k, v]) => `[${inner(k)}, ${inner(v)}]`).join(", ")}])`;
  }
  if (value instanceof Set) return `new Set([${list(value)}])`;
  if (ArrayBuffer.isView(value) && !(value instanceof DataView)) {
    return `${value.constructor.name}.from([${list(value as unknown as Iterable<unknown>)}])`;
  }
  if (value instanceof Date) {
    return Number.isNaN(value.getTime())
      ? "new Date(NaN)"
      : `new Date(${JSON.stringify(value.toISOString())})`;
  }
  if (value instanceof Error) return `new ${value.name}(${JSON.stringify(value.message)})`;
  const prototype: unknown = Object.getPrototypeOf(value);
  const fields = Object.keys(value).map(
    (name) => `${key(name)}: ${inner((value as Record<string, unknown>)[name])}`,
  );
  const body = fields.length === 0 ? "{}" : `{ ${fields.join(", ")} }`;
  if (prototype === null || prototype === Object.prototype) return body;
  const name = (prototype as { constructor?: { name?: string } }).constructor?.name;
  return name ? `${name} ${body}` : body;
}
