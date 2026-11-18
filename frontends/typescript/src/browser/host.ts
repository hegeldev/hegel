import type { OutputSink } from "../engine.js";
import { consoleOutput } from "../runtime.js";
import { WasmMemory } from "./arena.js";

/**
 * The engine's error stream, which is where a run's output goes: its bytes
 * arrive in pieces, so they are buffered up to each newline and every whole
 * line is passed to {@link sink}. The engine sets the sink to the
 * destination of the run it is calling into.
 */
export class HostStderr {
  sink: OutputSink = consoleOutput;
  private readonly decoder = new TextDecoder();
  private pending = "";

  write(bytes: Uint8Array): void {
    const lines = (this.pending + this.decoder.decode(bytes, { stream: true })).split("\n");
    this.pending = lines.pop()!;
    for (const line of lines) this.sink(line);
  }
}

export function createHostImports() {
  let memory: WasmMemory | undefined;
  const stderr = new HostStderr();
  return {
    setMemory(value: WebAssembly.Memory): void {
      memory = new WasmMemory(value);
    },
    stderr,
    imports: {
      hegel_host: {
        entropy_fill(ptr: number, length: number): number {
          try {
            if (!memory) return 0;
            crypto.getRandomValues(memory.bytes(ptr >>> 0, length >>> 0));
            return 1;
          } catch {
            return 0;
          }
        },
        monotonic_nanos(): bigint {
          try {
            const nanos = performance.now() * 1_000_000;
            if (!Number.isFinite(nanos) || nanos < 0) return -1n;
            const value = BigInt(Math.trunc(nanos));
            return value <= 0x7fffffffffffffffn ? value : -1n;
          } catch {
            return -1n;
          }
        },
        stderr_write(ptr: number, length: number): void {
          try {
            if (memory) stderr.write(memory.bytes(ptr >>> 0, length >>> 0));
          } catch {
            return;
          }
        },
      },
    },
  };
}
