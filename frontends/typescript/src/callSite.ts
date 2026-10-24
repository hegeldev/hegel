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
 * The name of the test the test runner is running, from the matcher state
 * that Jest, and Vitest after it, keep on `globalThis` (Vitest's
 * `"describe > test"`, Jest's `"describe test"`), or `null` under any other
 * runner.
 */
export function runnerTestName(): string | null {
  const matchers = (globalThis as Record<symbol, unknown>)[Symbol.for("$$jest-matchers-object")] as
    | { state?: { currentTestName?: unknown } }
    | undefined;
  const name = matchers?.state?.currentTestName;
  return typeof name === "string" && name !== "" ? name : null;
}

/**
 * The test location of a test function defined at `site`: the function's own
 * name, else the name the test runner gives the running test, else the
 * enclosing function's, else its line, so that two anonymous tests in one
 * file stay distinct.
 */
export function locationOf(site: CallSite, testFn: AnyFunction): TestLocation {
  const name = testFn.name || runnerTestName() || site.name || `<anonymous at line ${site.line}>`;
  const module = site.file.replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "");
  return { file: site.file, beginLine: site.line, class: module, function: name };
}

/**
 * `file` as a path that does not depend on where the project is checked out:
 * relative to `root` when it is inside it, and the path alone of a URL (so a
 * browser test's key does not depend on the host or port serving it).
 */
export function projectPath(file: string, root: string): string {
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(file)) return new URL(file).pathname;
  const posix = (path: string) => path.replace(/\\/g, "/").replace(/^\/([a-z]:\/)/i, "$1");
  const relativeTo = posix(root).replace(/\/$/, "");
  const path = posix(file);
  return relativeTo !== "" && path.startsWith(`${relativeTo}/`)
    ? path.slice(relativeTo.length + 1)
    : path;
}

/**
 * The database key of a test at `location`: its file, relative to the
 * project's `root`, plus its name.
 */
export function databaseKeyOf(location: TestLocation, root: string): string {
  return `${projectPath(location.file, root)}::${location.function}`;
}
