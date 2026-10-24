import { getLibhegel } from "./session.js";
import { consoleOutput, type RuntimeServices } from "./runtime.js";

export const nodeRuntime: RuntimeServices = {
  getEngine: getLibhegel,
  validateSettings(_settings) {},
  persistence: true,
  output: consoleOutput,
};
