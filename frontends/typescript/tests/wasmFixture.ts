import { readFileSync } from "node:fs";
import { WasmAbi } from "../src/browser/abi.js";
import { WasmEngine } from "../src/browser/engine.js";
import { createHostImports } from "../src/browser/host.js";
import { wasmPath } from "../scripts/engine.mjs";

export const wasmBytes = Uint8Array.from(readFileSync(wasmPath())).buffer;
export const wasmModule = new WebAssembly.Module(wasmBytes);

export function wasmFixture() {
  const host = createHostImports();
  const instance = new WebAssembly.Instance(wasmModule, host.imports);
  const abi = new WasmAbi(instance.exports);
  host.setMemory(abi.memory);
  return { abi, engine: new WasmEngine(abi), exports: instance.exports };
}
