import { readFileSync } from "node:fs";
import { join } from "node:path";
import { getLibhegel } from "./session.js";
import { consoleOutput, type RuntimeServices } from "./runtime.js";

function readGrammarFile(dir: string, path: string): string | null {
  try {
    return readFileSync(join(dir, path), "utf8");
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    if (code === "ENOENT" || code === "ENOTDIR") return null;
    throw error;
  }
}

export const nodeRuntime: RuntimeServices = {
  getEngine: getLibhegel,
  validateSettings(_settings) {},
  persistence: true,
  projectRoot: process.cwd(),
  readGrammarFile,
  output: consoleOutput,
};
