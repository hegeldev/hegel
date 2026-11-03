/** Node composition root. The browser entry composes runnerCore with its own runtime. */
import { createRunner, installRuntime } from "./runnerCore.js";
import { nodeRuntime } from "./nodeRuntime.js";

export {
  Verbosity,
  HealthCheck,
  Phase,
  Backend,
  NondeterminismStrictness,
  Database,
  Hegel,
  EngineDataSource as NativeDataSource,
  runTestCase,
  runTestCaseAsync,
} from "./runnerCore.js";
export type { Settings, TestLocation, TestCaseResult } from "./runnerCore.js";
installRuntime(nodeRuntime);
const runner = createRunner(nodeRuntime);
export const {
  test,
  testAsync,
  resolveSettings,
  engineVersion,
  registerProfile,
  setDefaultProfile,
} = runner;
