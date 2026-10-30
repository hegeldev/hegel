import { LIBHEGEL_VERSION } from "../libhegel-version.js";

/**
 * The Wasm module the browser entry loads: the `wasm32-unknown-unknown` build
 * of the in-tree libhegel, packaged into `dist/browser/` by `npm run build`
 * (see scripts/engine.mjs). The loader checks the module's version string
 * against the engine version the package was built with.
 */
export const WASM_ARTIFACT = {
  version: LIBHEGEL_VERSION,
  asset: "libhegel-wasm32-unknown-unknown.wasm",
} as const;
