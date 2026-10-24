import type { Engine } from "./engine.js";
import type { Settings } from "./settings.js";
import type { GrammarFileReader } from "./generate.js";

/** Only host services used by the shared run loop. */
export interface RuntimeServices {
  getEngine(): Engine;
  /** Throw for settings the host cannot honour. */
  validateSettings(settings: Settings): void;
  /**
   * Whether the host can keep a database. Without one, every run's database
   * is disabled, whatever the profile says.
   */
  persistence: boolean;
  /**
   * Read a grammar file a `fromLark` grammar imports from its `importPath`.
   * Absent where the host has no filesystem.
   */
  readGrammarFile?: GrammarFileReader;
  /** Print one line of a run's report. */
  output(line: string): void;
}

/** Where a run's report goes on every host: the console's error stream. */
export function consoleOutput(line: string): void {
  console.error(line);
}
