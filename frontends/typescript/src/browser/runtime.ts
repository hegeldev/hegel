import type { Engine } from "../engine.js";
import { EngineError } from "../engine.js";
import { consoleOutput, type RuntimeServices } from "../runtime.js";

export function browserRuntime(engine: Engine): RuntimeServices {
  return {
    getEngine: () => engine,
    validateSettings(settings) {
      if (settings.database?.kind === "path") {
        throw new EngineError("Filesystem databases are not supported in the browser");
      }
    },
    persistence: false,
    projectRoot: "",
    output: consoleOutput,
  };
}
