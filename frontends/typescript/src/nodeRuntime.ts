import { inspect } from "node:util";
import { getLibhegel } from "./session.js";
import { portableDiagnostics, type RuntimeServices } from "./runtime.js";

export const nodeRuntime: RuntimeServices = {
  ...portableDiagnostics,
  getEngine: getLibhegel,
  validateSettings(_settings) {},
  persistence: true,
  reportFinalValue(value, drawNumber) {
    console.error(`var draw_${drawNumber} = ${inspect(value, { depth: null })};`);
  },
};
