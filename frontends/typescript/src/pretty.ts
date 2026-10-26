/**
 * Printing drawn values.
 *
 * A drawn value prints as JavaScript syntax, the same in every runtime, with
 * its structure handed to the engine's printer: arrays, plain objects,
 * `Map`s, `Set`s, typed arrays and class instances open groups whose items
 * are separated by break points, so the engine lays a value out on one line
 * when it fits and one item per line when it does not. A class can print
 * itself by defining a {@link prettyPrint} method. Text the frontend can
 * only render flat (a symbol's description, a function) goes through
 * `hegel_printer_reflow`.
 *
 * @packageDocumentation
 */

import type { ContextHandle, Engine, PrinterHandle } from "./engine.js";

/**
 * A region of a printed document: the engine's printer for a test case, or a
 * view of an {@link EngineDocument} (an explicit example's).
 */
export interface Printer {
  /** Literal text, with no newlines. */
  text(text: string): void;
  hardBreak(): void;
  /** A one-line representation, laid out to fit. */
  reflow(text: string): void;
  /** Emit `open` and indent later break points `indent` columns further. */
  beginGroup(indent: number, open: string): void;
  /** Undo the innermost group's indentation and emit `close`. */
  endGroup(close: string): void;
  /** A break point: `sep` when its group fits on a line, else a newline. */
  breakable(sep: string): void;
  /** Text emitted only when the innermost group breaks. */
  ifBreak(text: string): void;
  /** A comment at the end of the current line, breaking every open group. */
  comment(text: string): void;
  /** Shift the indentation of later break points by `delta` columns. */
  shiftIndent(delta: number): void;
  /**
   * Buffer output until {@link commitSpeculative} keeps it or
   * {@link abortSpeculative} discards it: how a draw that is rejected part
   * way through leaves nothing behind.
   */
  beginSpeculative(): void;
  commitSpeculative(): void;
  abortSpeculative(): void;
}

interface LineState {
  atLineStart: boolean;
  owesBreak: boolean;
  indent: number;
}

/**
 * A standalone engine document (`hegel_printer_new`), written through
 * {@link Printer} views that each indent their lines by a fixed amount, as
 * the engine's `hegel_test_case_block` does for a test case's document: what
 * an explicit example prints into, so that it is laid out exactly like a
 * generated test case's report. A line's hard break is written only once the
 * next line's content arrives, so that it carries that line's indentation.
 * Notes made while a speculative region is open are held until it closes, as
 * the engine's `hegel_note` does.
 */
export class EngineDocument {
  private readonly handle: PrinterHandle;
  private state: LineState = { atLineStart: true, owesBreak: false, indent: 0 };
  private readonly saved: LineState[] = [];
  private held: [number, string][] = [];

  /** `maxWidth` is the width lines are laid out to, `null` for the engine's default. */
  constructor(
    private readonly lib: Engine,
    private readonly ctx: ContextHandle,
    maxWidth: bigint | null = null,
  ) {
    if (maxWidth === null) {
      this.handle = lib.newPrinter(ctx, null);
      return;
    }
    const options = lib.newPrinterOptions(ctx);
    try {
      lib.setPrinterMaxWidth(ctx, options, maxWidth);
      this.handle = lib.newPrinter(ctx, options);
    } finally {
      lib.freePrinterOptions(options);
    }
  }

  /** A {@link Printer} whose lines are indented `indent` columns. */
  view(indent = 0): Printer {
    const { lib, ctx, handle } = this;
    const write = (operation: () => void): void => {
      this.startLine(indent);
      operation();
    };
    return {
      text: (text) => write(() => lib.printerText(ctx, handle, text)),
      hardBreak: () => {
        this.startLine(indent);
        this.state.atLineStart = true;
        this.state.owesBreak = true;
      },
      reflow: (text) => write(() => lib.printerReflow(ctx, handle, text)),
      beginGroup: (by, open) => write(() => lib.printerBeginGroup(ctx, handle, by, open)),
      endGroup: (close) => write(() => lib.printerEndGroup(ctx, handle, close)),
      breakable: (sep) => write(() => lib.printerBreakable(ctx, handle, sep)),
      ifBreak: (text) => write(() => lib.printerIfBreak(ctx, handle, text)),
      comment: (text) => write(() => lib.printerComment(ctx, handle, text)),
      shiftIndent: (delta) => {
        lib.printerShiftIndent(ctx, handle, delta);
        this.state.indent += delta;
      },
      beginSpeculative: () => {
        lib.printerBeginSpeculative(ctx, handle);
        this.saved.push({ ...this.state });
      },
      commitSpeculative: () => {
        lib.printerCommitSpeculative(ctx, handle);
        this.saved.pop();
        this.release();
      },
      abortSpeculative: () => {
        lib.printerAbortSpeculative(ctx, handle);
        this.state = this.saved.pop()!;
        this.release();
      },
    };
  }

  /** Append a note at `indent`: each line of `message` becomes its own output line. */
  note(indent: number, message: string): void {
    const lines = message.split("\n").map((line): [number, string] => [indent, line]);
    this.held.push(...lines);
    if (this.saved.length === 0) this.release();
  }

  /** Everything printed to the document, one line per element. Seals it. */
  lines(): string[] {
    return splitLines(this.lib.printerValue(this.ctx, this.handle));
  }

  dispose(): void {
    this.lib.freePrinter(this.handle);
  }

  private release(): void {
    if (this.saved.length > 0) return;
    const held = this.held;
    this.held = [];
    for (const [indent, line] of held) {
      const view = this.view(indent);
      view.text(line);
      view.hardBreak();
    }
  }

  private startLine(indent: number): void {
    const state = this.state;
    if (!state.atLineStart) return;
    this.lib.printerShiftIndent(this.ctx, this.handle, indent - state.indent);
    state.indent = indent;
    if (state.owesBreak) this.lib.printerHardBreak(this.ctx, this.handle);
    else this.lib.printerText(this.ctx, this.handle, " ".repeat(indent));
    state.atLineStart = false;
    state.owesBreak = false;
  }
}

/** The widest a line can be: a document laid out to it never breaks a group. */
const UNBOUNDED = 0xffffffffffffffffn;

/** What `print` writes, on one line, through the engine's printer. */
export function printedLine(
  lib: Engine,
  ctx: ContextHandle,
  print: (printer: Printer) => void,
): string {
  const document = new EngineDocument(lib, ctx, UNBOUNDED);
  try {
    print(document.view());
    return document.lines().join("\n");
  } finally {
    document.dispose();
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

/** How far a broken group's items are indented. */
const INDENT = 4;

/**
 * The method a class defines to print its instances in a failure report, in
 * place of its class name and own enumerable properties. It is called with a
 * {@link PrettyPrinter} positioned where the value goes.
 *
 * @example
 * ```ts
 * class Interval {
 *   constructor(readonly lo: number, readonly hi: number) {}
 *   [hegel.prettyPrint](p: hegel.PrettyPrinter): void {
 *     p.group(4, "Interval(", ")", () => {
 *       p.value(this.lo);
 *       p.text(",");
 *       p.breakable(" ");
 *       p.value(this.hi);
 *     });
 *   }
 * }
 * ```
 */
export const prettyPrint: unique symbol = Symbol.for("hegel.prettyPrint");

/** A value whose class prints it itself; see {@link prettyPrint}. */
export interface PrettyPrintable {
  [prettyPrint](printer: PrettyPrinter): void;
}

function singleLine(text: string, what: string): string {
  if (text.includes("\n")) throw new TypeError(`${what} must not contain a newline`);
  return text;
}

/**
 * Writes a value's printed form into a failure report, through the engine's
 * layout: a group either fits on the current line, every break point in it
 * rendering as its separator, or breaks, every break point starting a new
 * line at the group's indentation. Handed to a {@link prettyPrint} method.
 */
export class PrettyPrinter {
  /** @internal */
  constructor(
    private readonly printer: Printer,
    private readonly enclosing: Set<object> = new Set(),
  ) {}

  /** Literal text, which must not contain a newline. */
  text(text: string): void {
    this.printer.text(singleLine(text, "text"));
  }

  /**
   * A break point: `sep` (by default a space) when the enclosing group fits
   * on one line, else a newline at the group's indentation.
   */
  breakable(sep = " "): void {
    this.printer.breakable(singleLine(sep, "a break point's separator"));
  }

  /** Text printed only when the enclosing group breaks, such as a trailing comma. */
  ifBreak(text: string): void {
    this.printer.ifBreak(singleLine(text, "text"));
  }

  /**
   * A comment at the end of the current line, written as it should appear
   * (`"  // like this"`). It breaks every group open around it.
   */
  comment(text: string): void {
    this.printer.comment(singleLine(text, "a comment"));
  }

  /** A newline at the current indentation. */
  hardBreak(): void {
    this.printer.hardBreak();
  }

  /** Emit `open` and indent the group's break points `indent` columns further. */
  beginGroup(indent: number, open: string): void {
    this.printer.beginGroup(indent, singleLine(open, "a group's opening text"));
  }

  /** Close the group {@link beginGroup} opened, emitting `close`. */
  endGroup(close: string): void {
    this.printer.endGroup(singleLine(close, "a group's closing text"));
  }

  /** {@link beginGroup}, then `body`, then {@link endGroup}. */
  group(indent: number, open: string, close: string, body: () => void): void {
    this.beginGroup(indent, open);
    body();
    this.endGroup(close);
  }

  /** Shift the indentation of later break points by `delta` columns. */
  shiftIndent(delta: number): void {
    this.printer.shiftIndent(delta);
  }

  /**
   * A one-line representation the engine recovers the bracket structure of,
   * so that it wraps like a value printed structurally.
   */
  reflow(text: string): void {
    this.printer.reflow(text);
  }

  /** Print `value` as a drawn value prints. */
  value(value: unknown): void {
    printValueIn(this.printer, value, this.enclosing);
  }
}

/**
 * Print `value` as JavaScript syntax: strings double-quoted with JSON
 * escapes, bigints with their `n` suffix, `-0` kept, and arrays, plain
 * objects, `Map`s, `Set`s, typed arrays and `Date`s as the expressions that
 * construct them. Instances of other classes print through their
 * {@link prettyPrint} method, or else as their class name and own enumerable
 * properties, and a value that contains itself prints the repeated reference
 * as `[Circular]`.
 */
export function printValue(printer: Printer, value: unknown): void {
  printValueIn(printer, value, new Set());
}

function printValueIn(printer: Printer, value: unknown, enclosing: Set<object>): void {
  switch (typeof value) {
    case "string":
      return printer.text(JSON.stringify(value));
    case "number":
      return printer.text(Object.is(value, -0) ? "-0" : String(value));
    case "bigint":
      return printer.text(`${value}n`);
    case "symbol":
      return printer.reflow(value.toString());
    case "function":
      return printer.reflow(`[Function ${value.name || "(anonymous)"}]`);
    case "object":
      if (value === null) return printer.text("null");
      if (enclosing.has(value)) return printer.text("[Circular]");
      enclosing.add(value);
      try {
        return printObject(printer, value, enclosing);
      } finally {
        enclosing.delete(value);
      }
    default:
      return printer.text(String(value));
  }
}

/**
 * Items between `open` and `close`, separated by `, `: on one line when they
 * fit (padded inside the delimiters by `padding`), else one item per line,
 * indented, each followed by a comma, with `close` on a line of its own.
 */
function block(
  printer: Printer,
  open: string,
  close: string,
  padding: string,
  items: readonly (() => void)[],
): void {
  if (items.length === 0) return printer.text(`${open}${close}`);
  printer.beginGroup(INDENT, open);
  printer.breakable(padding);
  items.forEach((item, index) => {
    if (index > 0) {
      printer.text(",");
      printer.breakable(" ");
    }
    item();
  });
  printer.ifBreak(",");
  printer.shiftIndent(-INDENT);
  printer.breakable(padding);
  printer.shiftIndent(INDENT);
  printer.endGroup(close);
}

function printObject(printer: Printer, value: object, enclosing: Set<object>): void {
  const inner = (item: unknown) => printValueIn(printer, item, enclosing);
  const each = (values: Iterable<unknown>) => Array.from(values, (item) => () => inner(item));
  const hook = (value as Partial<PrettyPrintable>)[prettyPrint];
  if (typeof hook === "function") return hook.call(value, new PrettyPrinter(printer, enclosing));
  if (Array.isArray(value)) return block(printer, "[", "]", "", each(value));
  if (value instanceof Map) {
    const entries = Array.from(
      value,
      ([k, v]) =>
        () =>
          block(printer, "[", "]", "", each([k, v])),
    );
    return block(printer, "new Map([", "])", "", entries);
  }
  if (value instanceof Set) return block(printer, "new Set([", "])", "", each(value));
  if (ArrayBuffer.isView(value) && !(value instanceof DataView)) {
    const items = each(value as unknown as Iterable<unknown>);
    return block(printer, `${value.constructor.name}.from([`, "])", "", items);
  }
  if (value instanceof Date) {
    return printer.text(
      Number.isNaN(value.getTime())
        ? "new Date(NaN)"
        : `new Date(${JSON.stringify(value.toISOString())})`,
    );
  }
  if (value instanceof Error) {
    return printer.reflow(`new ${value.name}(${JSON.stringify(value.message)})`);
  }
  const prototype: unknown = Object.getPrototypeOf(value);
  const fields = Object.keys(value).map((name) => () => {
    printer.text(`${key(name)}: `);
    inner((value as Record<string, unknown>)[name]);
  });
  const name =
    prototype === null || prototype === Object.prototype
      ? undefined
      : (prototype as { constructor?: { name?: string } }).constructor?.name;
  block(printer, name ? `${name} {` : "{", "}", " ", fields);
}
