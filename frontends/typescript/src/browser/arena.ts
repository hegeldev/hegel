import { EngineError } from "../engine.js";
import { u32, type Allocator } from "./abi.js";

/** Views are created on demand, never retained across engine calls or allocation. */
export class WasmMemory {
  constructor(readonly memory: WebAssembly.Memory) {}

  bytes(ptr: number, length: number): Uint8Array {
    u32(ptr);
    u32(length);
    if ((ptr === 0 && length !== 0) || ptr + length > this.memory.buffer.byteLength) {
      throw new EngineError(`Invalid Wasm memory range ${ptr}+${length}`);
    }
    return new Uint8Array(this.memory.buffer, ptr, length);
  }

  view(ptr: number, length: number): DataView {
    const bytes = this.bytes(ptr, length);
    return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  pointer(ptr: number): number {
    return this.view(ptr, 4).getUint32(0, true);
  }

  boolean(ptr: number): boolean {
    const value = this.view(ptr, 1).getUint8(0);
    if (value > 1) throw new EngineError("Invalid Wasm boolean result");
    return value === 1;
  }

  /** Decode the `hegel_string_t` (`{data, len}`) stored at `ptr`. */
  string(ptr: number): string {
    const view = this.view(ptr, 8);
    const bytes = this.bytes(view.getUint32(0, true), view.getUint32(4, true));
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch (cause) {
      throw new EngineError("Invalid UTF-8 in Wasm string", { cause });
    }
  }
}

/** One synchronous call scope; owned engine results use their own destructors. */
export class WasmArena extends WasmMemory {
  private readonly allocations: Array<{ ptr: number; size: number; align: number }> = [];

  constructor(private readonly allocator: Allocator) {
    super(allocator.memory);
  }

  alloc(size: number, align = 1): number {
    u32(size);
    u32(align);
    if (size === 0 || align === 0 || (align & (align - 1)) !== 0) {
      throw new EngineError("Invalid Wasm allocation layout");
    }
    const ptr = this.allocator.alloc(size, align);
    if (ptr === 0) throw new EngineError("Wasm allocation failed");
    u32(ptr);
    // Register before validation so a bad result cannot skip temporary cleanup.
    this.allocations.push({ ptr, size, align });
    if (ptr % align !== 0) throw new EngineError("Misaligned Wasm allocation");
    this.bytes(ptr, size).fill(0);
    return ptr;
  }

  input(value: Uint8Array | null): number {
    if (value === null) return 0;
    // A present empty list is distinct from NULL in the text-generator ABI.
    const ptr = this.alloc(Math.max(1, value.length));
    this.bytes(ptr, value.length).set(value);
    return ptr;
  }

  private writeString(ptr: number, value: Uint8Array | null): void {
    const data = this.input(value);
    const view = this.view(ptr, 8);
    view.setUint32(0, data, true);
    view.setUint32(4, value?.length ?? 0, true);
  }

  /** A `hegel_string_t` viewing `value`: `null` is the `{NULL, 0}` empty string. */
  bytesValue(value: Uint8Array | null): number {
    const ptr = this.alloc(8, 4);
    this.writeString(ptr, value);
    return ptr;
  }

  /** A `hegel_string_t` of `value`'s UTF-8, or NULL for `null`. */
  text(value: string | null): number {
    return value === null ? 0 : this.bytesValue(new TextEncoder().encode(value));
  }

  /** An array of `hegel_string_t`, or NULL for `null`. */
  stringList(values: readonly string[] | null): number {
    if (values === null) return 0;
    const ptr = this.alloc(Math.max(8, values.length * 8), 4);
    values.forEach((value, index) => {
      this.writeString(ptr + index * 8, new TextEncoder().encode(value));
    });
    return ptr;
  }

  private dispose(): void {
    const errors: unknown[] = [];
    for (const { ptr, size, align } of this.allocations.reverse()) {
      try {
        this.allocator.dealloc(ptr, size, align);
      } catch (error) {
        errors.push(error);
      }
    }
    if (errors.length)
      throw new EngineError("Wasm temporary cleanup failed", {
        cause: new AggregateError(errors),
      });
  }

  static scoped<T>(allocator: Allocator, body: (arena: WasmArena) => T): T {
    const arena = new WasmArena(allocator);
    let result!: T;
    let failure: unknown;
    let failed = false;
    try {
      result = body(arena);
    } catch (error) {
      failed = true;
      failure = error;
    } finally {
      try {
        arena.dispose();
      } catch (cleanup) {
        failure = failed
          ? new EngineError("Wasm call and temporary cleanup failed", {
              cause: new AggregateError([failure, cleanup]),
            })
          : cleanup;
        failed = true;
      }
    }
    if (failed) throw failure;
    return result;
  }
}
