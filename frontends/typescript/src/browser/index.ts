import { createRunner, installRuntime } from "../runnerCore.js";
import { loadWasm } from "./load-wasm.js";
import { browserRuntime } from "./runtime.js";

const engine = await loadWasm(new URL("./libhegel-wasm32-unknown-unknown.wasm", import.meta.url));
const runtime = browserRuntime(engine);
installRuntime(runtime);
const runner = createRunner(runtime);

export const test = runner.test;
export const testAsync = runner.testAsync;
export const resolveSettings = runner.resolveSettings;
export const engineVersion = runner.engineVersion;
export const registerProfile = runner.registerProfile;
export const setDefaultProfile = runner.setDefaultProfile;
export * as generators from "../generators/index.js";
export * as stateful from "../stateful.js";
export { TestCase } from "../testCase.js";
export {
  Verbosity,
  HealthCheck,
  Phase,
  Backend,
  NondeterminismStrictness,
  Database,
} from "../settings.js";
export type { ResolvedSettings, Settings, TestLocation } from "../settings.js";
export { EngineError } from "../engine.js";
export {
  RunError,
  FailedHealthCheckError,
  FlakyError,
  UnsatisfiableError,
  DidNotReproduceError,
} from "../errors.js";
export { Hegel } from "../runnerCore.js";
export {
  Document,
  PrettyPrinter,
  Speculation,
  prettyPrint,
  type DocumentOptions,
  type PrettyPrintable,
} from "../pretty.js";
