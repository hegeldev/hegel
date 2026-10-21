import { defineConfig } from "vitest/config";
import { fileURLToPath } from "url";
import { nativeLibraryPath } from "./scripts/engine.mjs";

export default defineConfig({
  resolve: {
    alias: {
      "@hegeldev/hegel/generators": fileURLToPath(
        new URL("./src/generators/index.ts", import.meta.url),
      ),
      "@hegeldev/hegel": fileURLToPath(new URL("./src/index.ts", import.meta.url)),
    },
  },
  test: {
    env: { HEGEL_LIBHEGEL_PATH: nativeLibraryPath() },
    include: ["tests/**/*.test.ts"],
    testTimeout: 30000,
    silent: "passed-only",
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      exclude: ["src/index.ts", "src/generators/index.ts"],
      reporter: ["text", "json", "json-summary"],
      thresholds: {
        lines: 100,
        branches: 100,
        functions: 100,
        statements: 100,
      },
    },
  },
});
