# Browser support

`@hegeldev/hegel` supports browsers through the WebAssembly build of libhegel, the engine in the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo this package is part of; the package ships the module of the engine version it was built with.

## Use the same imports

```ts
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

hegel.test((tc) => {
  const n = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
  if (n < 0) throw new Error("negative integer");
});

await hegel.testAsync(async (tc) => {
  const n = tc.draw(gs.integers());
  await Promise.resolve(n);
});
```

The browser entry fetches and compiles the packaged raw Rust engine during module loading with top-level await. No initialization call is needed. After the import completes, `test()` is synchronous and returns `void`; `testAsync()` returns `Promise<void>`.

Use modern ESM with an ES2022 build target, WebAssembly bigint integration, and Web Crypto in a secure context, normally HTTPS or localhost. Serve the `.wasm` file from a URL allowed by your fetch/CORS and Content Security Policy. CSP deployments may need `wasm-unsafe-eval`. Serve `application/wasm` for streaming compilation. The loader also handles an incorrect MIME type by instantiating the response bytes. Network, compile, link and runtime errors still escape.

Generation and shrinking run synchronously on the main browser thread. A long test can freeze the page, even when the property uses `testAsync()`. There are no workers or engine-output callbacks.

The browser has no database: every run's database is disabled, whatever the settings profile resolves, and `Database.default` and `Database.disabled` both mean that. Passing `Database.fromPath(...)` in browser test settings throws, including an empty path; constructing the database value does not. There is no IndexedDB, localStorage, filesystem or cloud database. Without a filesystem, `gs.fromLark` cannot read imported grammar files: its `importPath` option fails the test in the browser, though imports of Lark's bundled grammars (`%import common.NUMBER`) work. A failing test's report (its draws, notes and how to reproduce it) goes to `console.error`, exactly as on Node. The engine reports a test's location to Antithesis only from the native library. Concurrent state machines are not supported in JavaScript at all, in browsers as on Node: the host is single-threaded. Every other setting, profiles included, works as it does on Node. A test's identity comes from its call site, read with `Error.captureStackTrace`; a browser without it (Firefox) reports no test location, and the key that `derandomize` derives its seed from is then the test function's name, or none for an anonymous function: give such a test a key with `new hegel.Hegel(...).databaseKey(key)`.

All generators use the same schema interpreter and Rust engine as Node, including canonical UUID strings from `uuids()` and version-restricted UUIDs from `uuids({ version: 4 })`. Temporal strings preserve nanoseconds (six fractional digits for whole microseconds, nine otherwise). Rust text generation excludes surrogate codepoints; the adapter preserves WTF-8 where the engine returns it. On Node the text and regex generators follow the Unicode version of Node's own tables (`process.versions.unicode`); a browser reports no Unicode version, so there they follow the engine's own tables, the newest Unicode version it has.

## Bundle the asset

Root export conditions are ordered `types`, `node`, `browser`, `default`. Node takes the native Koffi entry, even if `browser` is also enabled. The default is the browser entry so web tools without a browser condition cannot accidentally import Koffi. Do not add `node` to a browser bundler's conditions.

The entry contains the literal expression:

```js
new URL("./libhegel-wasm32-unknown-unknown.wasm", import.meta.url);
```

The asset is also exported as `@hegeldev/hegel/libhegel.wasm`. Koffi and native optional platform packages remain install-time dependencies of this single npm package, but they are absent from browser bundles. Node, Bun and Deno retain their native conditional-export path.

### Vite

Vite handles the static URL and emits the asset. Set the target and deployment base:

```js
export default {
  base: "/tests/",
  build: { target: "es2022", assetsInlineLimit: 0 },
};
```

### webpack

webpack 5 handles the URL as an asset. An ESM output configuration is:

```js
export default {
  target: ["web", "es2022"],
  experiments: { outputModule: true },
  output: { module: true, publicPath: "/tests/" },
};
```

### esbuild and Rollup

These configurations preserve the URL relative to the output module but do not copy the Wasm file. For a single output module, copy the exported asset beside that module under its original filename. This is build-time asset handling, not a runtime initialization API.

```js
import { copyFileSync } from "node:fs";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

// Run after emitting out/app.js:
copyFileSync(
  require.resolve("@hegeldev/hegel/libhegel.wasm"),
  "out/libhegel-wasm32-unknown-unknown.wasm",
);
```

Use esbuild with `bundle: true`, `platform: "browser"`, `format: "esm"`, `target: "es2022"` and `outfile: "out/app.js"`.

Use Rollup with `@rollup/plugin-node-resolve` and `output: { format: "es", file: "out/app.js" }`. Default node-resolve conditions work because the package default is the browser entry. If your application splits chunks or changes directories, copy the asset beside the chunk containing Hegel's URL expression or configure an asset plugin to rewrite that URL. The tested copy recipe is for a single output module.

## Maintainers: the Wasm module is one more build of the in-tree engine

The browser entry runs the same libhegel version as the native library: the engine in `../../libhegel`, compiled to `wasm32-unknown-unknown` by `just typescript::build-libhegel` (the version it reports is checked against the generated `src/libhegel-version.ts`). The tests load the module from that build (or from `HEGEL_WASM_PATH`, the counterpart of `HEGEL_LIBHEGEL_PATH`), and `npm run build` copies it into `dist/browser/`, where the browser entry's `new URL(...)` expression finds it. The library never downloads anything at runtime; it fetches only the asset the application serves.

To run everything locally:

```sh
just typescript::setup
just typescript::setup-browser
just typescript::check
just typescript::check-browser
```

`just typescript::check-browser` (`tests/browser/run.mjs`; the bundlers and Playwright are that fixture's own dependencies, not the library's) packs and installs the actual main and host-platform tarballs into a temporary consumer. It checks the public declarations without Node types, runs native sync/async, UUID, shrinking and database-persistence tests with no library-path override, then bundles the browser entry with Vite, webpack, esbuild and Rollup. Chromium runs every bundle under `/nested/` with the correct Wasm MIME type, and Vite also covers the incorrect-MIME fallback. Browser checks cover exports, sync/async calls, UUIDs, shrinking to 50, database rejection, emitted assets (byte-identical to the in-tree build), request counts and dependency graphs (no Koffi, Node built-ins or polyfills). This matrix does not yet establish Firefox or Safari compatibility.

When the C ABI changes, `src/browser/abi.ts` must be updated with it: every raw Wasm signature (in particular the by-value structs, which lower to pointers) must still match the module, which the `typescript-ffi` skill walks through. At startup the loader checks the module's version string against `src/libhegel-version.ts`, nothing more.
