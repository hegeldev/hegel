import type { Engine } from "./engine.js";
import type { Settings } from "./settings.js";

export interface Diagnostics {
  reportFinalValue(value: unknown, drawNumber: number): void;
  reportFinalError(error: unknown): void;
  note(message: string): void;
}

/** Only host services used by the shared run loop. */
export interface RuntimeServices extends Diagnostics {
  getEngine(): Engine;
  /** Throw for settings the host cannot honour. */
  validateSettings(settings: Settings): void;
  /**
   * Whether the host can keep a database. Without one, every run's database
   * is disabled, whatever the profile says.
   */
  persistence: boolean;
}

export const portableDiagnostics: Diagnostics = {
  reportFinalValue(value, drawNumber) {
    console.error(`var draw_${drawNumber} =`, value);
  },
  reportFinalError(error) {
    console.error(`\n${error instanceof Error ? error.message : String(error)}`);
    if (error instanceof Error && error.stack) console.error(error.stack);
  },
  note(message) {
    console.error(message);
  },
};
