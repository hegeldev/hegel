/**
 * Printing drawn values.
 *
 * Printing is the generator's job: a {@link generators.PrintableGenerator}
 * draws a value and writes its representation to a {@link PrettyPrinter} in
 * the same step, so a container prints element by element and an element's
 * own printing shows up inside it. Everything is laid out by the engine's
 * printer: the frontend only emits text, break points and groups through
 * the C ABI.
 *
 * {@link PrettyPrinter.value} prints a finished value as JavaScript syntax
 * (arrays, plain objects, `Map`s, `Set`s, typed arrays and class instances
 * open groups whose items are separated by break points). It is what
 * value-producing generators (`map`, `just`, `sampledFrom`, `composite`)
 * print with, and what {@link generators.Generator.printAsValue} makes a
 * generator print with. A class can print its instances itself by defining
 * a {@link prettyPrint} method.
 *
 * @packageDocumentation
 */

import type { ContextHandle, Engine, PrinterHandle } from "./engine.js";

/**
 * A region of a printed document: the engine's printer for a test case, a
 * view of an {@link EngineDocument} (an explicit example's), or a deferred
 * region opened inside either.
 */
export interface Printer {
  text(text: string): void;
  hardBreak(): void;
  reflow(text: string): void;
  beginGroup(indent: number, open: string): void;
  endGroup(close: string): void;
  breakable(sep: string): void;
  ifBreak(text: string): void;
  comment(text: string): void;
  shiftIndent(delta: number): void;
  /**
   * Buffer output until {@link commitSpeculative} keeps it or
   * {@link abortSpeculative} discards it: how a draw that is rejected part
   * way through leaves nothing behind.
   */
  beginSpeculative(): void;
  commitSpeculative(): void;
  abortSpeculative(): void;
  /** A child region anchored at the current position, or `null` when this region has died. */
  deferred(): Printer | null;
  /** Whether writes to this region still reach the document. */
  isLive(): boolean;
}

/** What owns the engine handles behind a family of {@link Printer}s. */
export interface PrinterOwner {
  readonly lib: Engine;
  readonly ctx: ContextHandle;
  /** Whether the document has been read or its handles released: writes are then dropped. */
  released(): boolean;
  /** Take ownership of a deferred region's handle, released with the document. */
  adopt(handle: PrinterHandle): void;
  /** Run an engine operation, reporting a failure as the owner does. */
  run<T>(operation: () => T): T;
}

/**
 * A {@link Printer} writing straight to `handle`. A deferred region's
 * handle (`deferredRegion`) can die, when the document is resolved or a
 * speculative region around its anchor is aborted, and its writes then do
 * nothing.
 */
export function enginePrinter(
  owner: PrinterOwner,
  handle: PrinterHandle,
  deferredRegion: boolean,
): Printer {
  const { lib, ctx } = owner;
  const live = (): boolean =>
    !owner.released() && (!deferredRegion || owner.run(() => lib.printerIsLive(ctx, handle)));
  const write = (operation: () => void): void => {
    if (live()) owner.run(operation);
  };
  return {
    text: (text) => write(() => lib.printerText(ctx, handle, text)),
    hardBreak: () => write(() => lib.printerHardBreak(ctx, handle)),
    reflow: (text) => write(() => lib.printerReflow(ctx, handle, text)),
    beginGroup: (indent, open) => write(() => lib.printerBeginGroup(ctx, handle, indent, open)),
    endGroup: (close) => write(() => lib.printerEndGroup(ctx, handle, close)),
    breakable: (sep) => write(() => lib.printerBreakable(ctx, handle, sep)),
    ifBreak: (text) => write(() => lib.printerIfBreak(ctx, handle, text)),
    comment: (text) => write(() => lib.printerComment(ctx, handle, text)),
    shiftIndent: (delta) => write(() => lib.printerShiftIndent(ctx, handle, delta)),
    beginSpeculative: () => write(() => lib.printerBeginSpeculative(ctx, handle)),
    commitSpeculative: () => write(() => lib.printerCommitSpeculative(ctx, handle)),
    abortSpeculative: () => write(() => lib.printerAbortSpeculative(ctx, handle)),
    deferred: () => (live() ? openDeferred(owner, handle) : null),
    isLive: live,
  };
}

function openDeferred(owner: PrinterOwner, handle: PrinterHandle): Printer {
  const child = owner.run(() => owner.lib.printerDeferred(owner.ctx, handle));
  owner.adopt(child);
  return enginePrinter(owner, child, true);
}

/**
 * Whether any of the deferred regions `handles` is still live, so that the
 * document has a deferred session to resolve: regions opened inside an
 * aborted speculation have died with it.
 */
export function anyLive(lib: Engine, ctx: ContextHandle, handles: PrinterHandle[]): boolean {
  return handles.some((handle) => lib.printerIsLive(ctx, handle));
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
  private readonly regions: PrinterHandle[] = [];
  private sealed = false;
  private readonly owner: PrinterOwner;

  /** `maxWidth` is the width lines are laid out to, `null` for the engine's default. */
  constructor(
    private readonly lib: Engine,
    private readonly ctx: ContextHandle,
    maxWidth: bigint | null = null,
  ) {
    this.owner = {
      lib,
      ctx,
      released: () => this.sealed,
      adopt: (handle) => this.regions.push(handle),
      run: (operation) => operation(),
    };
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
    const open = (): boolean => !this.sealed;
    const write = (operation: () => void): void => {
      if (!open()) return;
      this.startLine(indent);
      operation();
    };
    const control = (operation: () => void): void => {
      if (open()) operation();
    };
    return {
      text: (text) => write(() => lib.printerText(ctx, handle, text)),
      hardBreak: () =>
        control(() => {
          this.startLine(indent);
          this.state.atLineStart = true;
          this.state.owesBreak = true;
        }),
      reflow: (text) => write(() => lib.printerReflow(ctx, handle, text)),
      beginGroup: (by, open) => write(() => lib.printerBeginGroup(ctx, handle, by, open)),
      endGroup: (close) => write(() => lib.printerEndGroup(ctx, handle, close)),
      breakable: (sep) => write(() => lib.printerBreakable(ctx, handle, sep)),
      ifBreak: (text) => write(() => lib.printerIfBreak(ctx, handle, text)),
      comment: (text) => write(() => lib.printerComment(ctx, handle, text)),
      shiftIndent: (delta) =>
        control(() => {
          lib.printerShiftIndent(ctx, handle, delta);
          this.state.indent += delta;
        }),
      beginSpeculative: () =>
        control(() => {
          lib.printerBeginSpeculative(ctx, handle);
          this.saved.push({ ...this.state });
        }),
      commitSpeculative: () =>
        control(() => {
          lib.printerCommitSpeculative(ctx, handle);
          this.saved.pop();
          this.release();
        }),
      abortSpeculative: () =>
        control(() => {
          lib.printerAbortSpeculative(ctx, handle);
          this.state = this.saved.pop()!;
          this.release();
        }),
      deferred: () => {
        if (!open()) return null;
        this.startLine(indent);
        return openDeferred(this.owner, handle);
      },
      isLive: open,
    };
  }

  /** Append a note at `indent`: each line of `message` becomes its own output line. */
  note(indent: number, message: string): void {
    const lines = message.split("\n").map((line): [number, string] => [indent, line]);
    this.held.push(...lines);
    if (this.saved.length === 0) this.release();
  }

  /** Everything printed to the document, with its deferred regions spliced in. Seals it. */
  value(): string {
    if (!this.sealed && anyLive(this.lib, this.ctx, this.regions)) {
      this.lib.printerResolve(this.ctx, this.handle);
    }
    this.sealed = true;
    return this.lib.printerValue(this.ctx, this.handle);
  }

  /** Everything printed to the document, one line per element. Seals it. */
  lines(): string[] {
    return splitLines(this.value());
  }

  dispose(): void {
    this.sealed = true;
    for (const region of this.regions.splice(0)) this.lib.freePrinter(region);
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
  print: (printer: PrettyPrinter) => void,
): string {
  const document = new EngineDocument(lib, ctx, UNBOUNDED);
  try {
    print(new PrettyPrinter(document.view()));
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

/** A plain object's key as it is written in an object literal. */
export function propertyKey(name: string): string {
  return IDENTIFIER.test(name) ? name : JSON.stringify(name);
}

/** How far a broken group's items are indented. */
const INDENT = 4;

/**
 * The method a class defines to print its instances when they are printed
 * as values (by {@link PrettyPrinter.value}), in place of its class name and
 * own enumerable properties. It is called with a {@link PrettyPrinter}
 * positioned where the value goes. It plays the part of a custom `Debug`
 * implementation in Rust: to print a generator's values some other way,
 * give the generator a printer with
 * {@link generators.Generator.printWith | printWith}.
 *
 * @example
 * ```ts
 * class Interval {
 *   constructor(readonly lo: number, readonly hi: number) {}
 *   [hegel.prettyPrint](p: hegel.PrettyPrinter): void {
 *     p.group(9, "Interval(", ")", () => {
 *       p.value(this.lo);
 *       p.separator();
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
 * The write surface of a failure report, laid out by the engine: a group
 * either fits on the current line, every break point in it rendering as its
 * separator, or breaks, every break point starting a new line at the
 * group's indentation.
 *
 * A {@link generators.PrintableGenerator} receives one in
 * {@link generators.PrintableGenerator.doDrawAndPrint | doDrawAndPrint},
 * positioned where the value goes, and so do
 * {@link generators.Generator.printWith | printWith} callbacks and
 * {@link prettyPrint} methods. {@link PrettyPrinter.noop} discards
 * everything, and a {@link Document} gives one to print into outside a test.
 */
export class PrettyPrinter {
  /** @internal */
  readonly region: Printer | null;
  private readonly enclosing: Set<object>;

  /** @internal */
  constructor(region: Printer | null, enclosing: Set<object> = new Set()) {
    this.region = region;
    this.enclosing = enclosing;
  }

  /**
   * A printer that discards everything printed to it. A generator drawing
   * silently draws through its printing path with this printer, so that
   * both paths make the same choices.
   */
  static noop(): PrettyPrinter {
    return new PrettyPrinter(null);
  }

  /**
   * Whether printing to this printer produces output: `false` for
   * {@link noop}, for a {@link deferred} region that has died, and once the
   * test case it belongs to is over. Use it to skip work whose only purpose
   * is to be printed.
   */
  shouldPrint(): boolean {
    return this.region !== null && this.region.isLive();
  }

  /** Literal text. A newline in it starts a new line, as {@link hardBreak} does. */
  text(text: string): void {
    const region = this.region;
    if (region === null) return;
    text.split("\n").forEach((segment, index) => {
      if (index > 0) region.hardBreak();
      if (segment !== "") region.text(segment);
    });
  }

  /**
   * A break point: `sep` (by default a space) when the enclosing group fits
   * on one line, else a newline at the group's indentation.
   */
  breakable(sep = " "): void {
    this.region?.breakable(singleLine(sep, "a break point's separator"));
  }

  /** Text printed only when the enclosing group breaks, such as a trailing comma. */
  ifBreak(text: string): void {
    this.region?.ifBreak(singleLine(text, "text"));
  }

  /**
   * A comment at the end of the current line, printed as `  // text`. It
   * breaks every group open around it.
   */
  comment(text: string): void {
    this.region?.comment(`  // ${singleLine(text, "a comment")}`);
  }

  /** A newline at the current indentation. */
  hardBreak(): void {
    this.region?.hardBreak();
  }

  /** Emit `open` and indent the group's break points `indent` columns further. */
  beginGroup(indent: number, open: string): void {
    this.region?.beginGroup(indent, singleLine(open, "a group's opening text"));
  }

  /** Close the group {@link beginGroup} opened, emitting `close`. */
  endGroup(close: string): void {
    this.region?.endGroup(singleLine(close, "a group's closing text"));
  }

  /**
   * {@link beginGroup}, then `body` (given this printer), then
   * {@link endGroup}. Returns what `body` returns.
   */
  group<R>(indent: number, open: string, close: string, body: (printer: PrettyPrinter) => R): R {
    this.beginGroup(indent, open);
    const result = body(this);
    this.endGroup(close);
    return result;
  }

  /**
   * Print `items` between `open` and `close` the way built-in containers
   * print: on one line, separated by `, `, when they fit, and otherwise one
   * item per line, indented four columns, each followed by a comma, with
   * `close` on a line of its own. (Rust's `seq` aligns broken items under
   * `open` instead; write that layout with {@link group} and
   * {@link separator}.)
   */
  seq<T>(
    open: string,
    close: string,
    items: Iterable<T>,
    print: (printer: PrettyPrinter, item: T) => void,
  ): void {
    const block = new Block(this, open, close);
    for (const item of items) block.item(() => print(this, item));
    block.end();
  }

  /** The separator between two items of a group: `,` and a break point. */
  separator(): void {
    this.text(",");
    this.breakable(" ");
  }

  /** Shift the indentation of later break points by `delta` columns. */
  shiftIndent(delta: number): void {
    this.region?.shiftIndent(delta);
  }

  /**
   * A one-line representation the engine recovers the bracket structure of,
   * so that it wraps like a value printed structurally.
   */
  reflow(text: string): void {
    this.region?.reflow(text);
  }

  /**
   * Print `value` as JavaScript syntax: strings double-quoted with JSON
   * escapes, bigints with their `n` suffix, `-0` kept, and arrays, plain
   * objects, `Map`s, `Set`s, typed arrays and `Date`s as the expressions
   * that construct them. Instances of other classes print through their
   * {@link prettyPrint} method, or else as their class name and own
   * enumerable properties, and a value that contains itself prints the
   * repeated reference as `[Circular]`.
   */
  value(value: unknown): void {
    if (this.shouldPrint()) printValueIn(this, value, this.enclosing);
  }

  /**
   * Open a speculative region: output printed from now on is held back
   * until {@link Speculation.commit} keeps it or {@link Speculation.abort}
   * discards it. A generator that may retract a draw (a filter's rejected
   * attempt, a duplicate element) prints each attempt speculatively:
   *
   * ```ts
   * const speculation = printer.speculate();
   * try {
   *   const value = tc.drawAndPrint(source, printer);
   *   if (accepted(value)) speculation.commit();
   * } finally {
   *   speculation.abort();
   * }
   * ```
   */
  speculate(): Speculation {
    return new Speculation(this);
  }

  /**
   * A printer for a hole in the document at the current position, which can
   * be written to later: what it prints appears here, laid out as if it had
   * been printed inline. This is how a value whose description is only known
   * once the test uses it (a stub function recording its calls, say) prints.
   * Writes to it do nothing once the test case is over, or when a
   * speculative region around its position was aborted.
   */
  deferred(): PrettyPrinter {
    return new PrettyPrinter(this.region?.deferred() ?? null);
  }
}

/** An open speculative region; see {@link PrettyPrinter.speculate}. */
export class Speculation {
  private open = true;

  /** @internal */
  constructor(
    /** The printer to print the speculative output through. */
    readonly printer: PrettyPrinter,
  ) {
    printer.region?.beginSpeculative();
  }

  /** Close the region, keeping its output. Does nothing once the region is closed. */
  commit(): void {
    if (!this.open) return;
    this.open = false;
    this.printer.region?.commitSpeculative();
  }

  /** Close the region, discarding its output. Does nothing once the region is closed. */
  abort(): void {
    if (!this.open) return;
    this.open = false;
    this.printer.region?.abortSpeculative();
  }
}

let documentEngine: () => Engine = () => {
  throw new Error("No Hegel engine is loaded: import @hegeldev/hegel before creating a Document");
};

/**
 * Set where a {@link Document} gets its engine: what the Node and browser
 * entry points do when loaded.
 *
 * @internal
 */
export function installDocumentEngine(engine: () => Engine): void {
  documentEngine = engine;
}

/** Options for a {@link Document}. */
export interface DocumentOptions {
  /** The width lines are kept within where the groups allow it (default 79). */
  maxWidth?: number;
}

/**
 * A standalone document laid out by the engine, for printing outside a
 * test: to check what a {@link generators.Generator.printWith | printWith}
 * callback or a {@link generators.PrintableGenerator} prints, say. Print
 * into {@link printer}, then call {@link finish} once to read the result
 * and release the document.
 *
 * @example
 * ```ts
 * const doc = new hegel.Document({ maxWidth: 8 });
 * doc.printer.seq("[", "]", [1, 2, 3], (p, n) => p.value(n));
 * doc.finish(); // "[\n    1,\n    2,\n    3,\n]"
 * ```
 */
export class Document {
  /** The printer to write the document's content through. */
  readonly printer: PrettyPrinter;
  private readonly lib: Engine;
  private readonly ctx: ContextHandle;
  private readonly document: EngineDocument;
  private finished = false;

  constructor(options: DocumentOptions = {}) {
    const maxWidth = options.maxWidth;
    if (maxWidth !== undefined && (!Number.isSafeInteger(maxWidth) || maxWidth < 1)) {
      throw new RangeError(`maxWidth must be a positive integer, got ${String(maxWidth)}`);
    }
    this.lib = documentEngine();
    this.ctx = this.lib.newContext();
    this.document = new EngineDocument(
      this.lib,
      this.ctx,
      maxWidth === undefined ? null : BigInt(maxWidth),
    );
    this.printer = new PrettyPrinter(this.document.view());
  }

  /**
   * Splice in the content of any deferred regions, lay the document out and
   * return it, releasing the document. Call it once.
   */
  finish(): string {
    if (this.finished) throw new Error("Document.finish was already called");
    this.finished = true;
    try {
      return this.document.value();
    } finally {
      this.document.dispose();
      this.lib.freeContext(this.ctx);
    }
  }
}

/**
 * Items between `open` and `close`, written one at a time: on one line,
 * separated by `, ` (and padded inside the delimiters by `padding`), when
 * they fit, else one item per line, indented, each followed by a comma,
 * with `close` on a line of its own. An item printed speculatively calls
 * {@link next} inside its speculation and {@link accept} once it is kept.
 *
 * @internal
 */
export class Block {
  private count = 0;

  constructor(
    private readonly printer: PrettyPrinter,
    open: string,
    private readonly close: string,
    private readonly padding = "",
  ) {
    printer.beginGroup(INDENT, open);
  }

  /** The break before the next item. */
  next(): void {
    if (this.count === 0) this.printer.breakable(this.padding);
    else this.printer.separator();
  }

  /** Count the item printed since {@link next}. */
  accept(): void {
    this.count++;
  }

  item<R>(body: () => R): R {
    this.next();
    const result = body();
    this.accept();
    return result;
  }

  end(): void {
    const printer = this.printer;
    if (this.count > 0) {
      printer.ifBreak(",");
      printer.shiftIndent(-INDENT);
      printer.breakable(this.padding);
      printer.shiftIndent(INDENT);
    }
    printer.endGroup(this.close);
  }
}

function printValueIn(printer: PrettyPrinter, value: unknown, enclosing: Set<object>): void {
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

function block(
  printer: PrettyPrinter,
  open: string,
  close: string,
  padding: string,
  items: readonly (() => void)[],
): void {
  const writer = new Block(printer, open, close, padding);
  for (const item of items) writer.item(item);
  writer.end();
}

function printObject(printer: PrettyPrinter, value: object, enclosing: Set<object>): void {
  const inner = (item: unknown) => printValueIn(printer, item, enclosing);
  const each = (values: Iterable<unknown>) => Array.from(values, (item) => () => inner(item));
  const hook = (value as Partial<PrettyPrintable>)[prettyPrint];
  if (typeof hook === "function") return hook.call(value, printer);
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
    printer.text(`${propertyKey(name)}: `);
    inner((value as Record<string, unknown>)[name]);
  });
  const name =
    prototype === null || prototype === Object.prototype
      ? undefined
      : (prototype as { constructor?: { name?: string } }).constructor?.name;
  block(printer, name ? `${name} {` : "{", "}", " ", fields);
}
