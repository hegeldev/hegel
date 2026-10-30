/**
 * Resolve the libhegel path for tests: `HEGEL_LIBHEGEL_PATH`, or the in-tree
 * release build (see scripts/engine.mjs).
 */
import { nativeLibraryPath } from "../scripts/engine.mjs";

export function testLibPath(): string {
  return nativeLibraryPath();
}
