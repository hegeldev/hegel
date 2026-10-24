import type { Engine } from "../engine.js";
import { EngineError } from "../engine.js";
import { portableDiagnostics, type RuntimeServices } from "../runtime.js";

export function browserRuntime(engine: Engine): RuntimeServices {
  return {
    ...portableDiagnostics,
    getEngine: () => engine,
    validateSettings(settings) {
      if (settings.database?.kind === "path") {
        throw new EngineError("Filesystem databases are not supported in the browser");
      }
    },
    persistence: false,
  };
}
