/**
 * The identity of a test, derived from where it was defined: the source of
 * its database key and its Antithesis test location.
 *
 * @packageDocumentation
 */

import type { TestLocation } from "./settings.js";

/** A stack frame: the file, line and (when it has one) function name. */
export interface CallSite {
  file: string;
  line: number;
  name: string | null;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFunction = (...args: any[]) => unknown;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Callable = AnyFunction | (abstract new (...args: any[]) => unknown);

type CaptureStackTrace = (target: object, above?: Callable) => void;

const V8_FRAME = /^\s*at (?:async )?(?:(.+?) \()?(.+?):(\d+):\d+\)?$/;
const GECKO_FRAME = /^(.*?)@(.+?):(\d+):\d+$/;

/** Parse one line of `Error.stack`, in V8 or SpiderMonkey/JavaScriptCore form. */
export function parseFrame(line: string): CallSite | null {
  const match = V8_FRAME.exec(line) ?? GECKO_FRAME.exec(line);
  if (match === null) return null;
  const [, name, file, lineNumber] = match;
  return { file: file.replace(/^file:\/\//, ""), line: Number(lineNumber), name: name || null };
}

/**
 * The frame that called `above`, or `null` where the runtime cannot say
 * (no `Error.captureStackTrace`, or a stack it does not format as expected).
 */
export function callSite(above: Callable): CallSite | null {
  const capture = (Error as { captureStackTrace?: CaptureStackTrace }).captureStackTrace;
  if (capture === undefined) return null;
  const holder: { stack?: string } = {};
  capture(holder, above);
  for (const line of (holder.stack ?? "").split("\n")) {
    const frame = parseFrame(line);
    if (frame !== null) return frame;
  }
  return null;
}

/**
 * The test location of a test function defined at `site`: the function's own
 * name, else the enclosing function's, else its line, so that two anonymous
 * tests in one file stay distinct.
 */
export function locationOf(site: CallSite, testFn: AnyFunction): TestLocation {
  const name = testFn.name || site.name || `<anonymous at line ${site.line}>`;
  const module = site.file.replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "");
  return { file: site.file, beginLine: site.line, class: module, function: name };
}

/** The database key of a test at `location`: its file plus its name. */
export function databaseKeyOf(location: TestLocation): string {
  return `${location.file}::${location.function}`;
}
