---
name: typescript-ffi
description: "How the TypeScript frontend (frontends/typescript) binds libhegel's C ABI: the koffi bindings for Node, Bun and Deno and the raw Wasm adapter for browsers. Use when changing libhegel's C ABI (hegel.h), when the TypeScript tests fail at library load with a missing symbol or an incompatible Wasm export, or when adding an ABI function to the TypeScript frontend."
---

# The TypeScript frontend's bindings to libhegel

`frontends/typescript` calls libhegel, the engine in `libhegel/` of this
repository, synchronously through its C ABI (`libhegel/include/hegel.h`):
on Node, Bun and Deno via the `koffi` FFI library, in browsers through the raw
exports of the engine's `wasm32-unknown-unknown` build. There is no pinned
release to align to: the frontend always builds against the in-tree engine
(`just typescript::build-libhegel`), so a C ABI change and the binding change
that follows it land in the same PR. The public `@hegeldev/hegel` API
(`test`, `testAsync`, `Settings`, the generators) must **not** change as a
result of an ABI change — only the internal layers.

The layers, and what an ABI change usually touches:

- **`src/engine.ts`** — the runtime-independent `Engine` interface both
  adapters implement: one method per ABI operation, branded handle types, the
  enums copied from `hegel.h`. A new or reshaped operation changes here first.
- **`src/libhegel.ts`** — the whole koffi surface. Three things move together:
  the koffi declarations in `bindLibrary` (which must mirror the C signatures
  exactly), the `Bindings` interface (the typed, slightly slimmed wrapper
  surface), and the `Libhegel` class methods (ergonomic wrappers that map
  result codes to exceptions via `Libhegel.check`).
- **`src/browser/abi.ts`** — the exact raw Wasm signature of every export the
  adapter calls (`I` = i32, `L` = i64, `D` = f64), checked at instantiation so
  a drifted module fails loudly, plus `voidOperations` for exports that return
  nothing. **`src/browser/arena.ts`** marshals `hegel_string_t` values and
  lists in and out of Wasm memory, and **`src/browser/engine.ts`** implements
  `Engine` over those. `tests/wasm-engine.test.ts` runs both engines side by
  side against the real builds.
- **`src/generate.ts`** — the client-side schema interpreter driving the
  per-primitive draw entry points. Touch it when a draw entry point changes
  shape or a new one replaces schema-level behaviour.
- **`src/runnerCore.ts` / `src/session.ts`** — the run lifecycle
  (`run_start` → `next_test_case` → `mark_complete` → `run_result`, plus
  failure replay via `hegel_test_case_from_blob`) and the process-global
  library handle with its `major.minor` version check. Touch only if the
  lifecycle protocol itself changes.
- **`src/wtf8.ts`** — decodes engine-returned string bytes. Touch only if
  string encoding changes.

## The ABI's conventions

Every fallible libhegel call follows one convention, and the binding is shaped
around it:

- The **first argument** is a `hegel_context_t*` (an error-reporting context).
- The **return value** is a `hegel_result_t` code (`HEGEL_OK` is 0; failures
  are negative — `-1` is `HEGEL_E_STOP_TEST`, `-2` is `HEGEL_E_ASSUME`).
- Any **value the call produces** (a handle, a count, a bool, a buffer) is
  written through a **trailing out-parameter**, never returned.
- On a non-OK return, the message is read back from the context via
  `hegel_context_last_error`, which returns a `hegel_string_t` by value.
- **Strings are `hegel_string_t`** (`{const char *data; size_t len}`), by
  value for a required string and `const hegel_string_t *` (NULL for none)
  for an optional one or an array; they are length-delimited, so they may
  contain NUL. Outputs are NUL-terminated with `len` excluding it. An
  engine-owned `hegel_owned_string_t` (`hegel_generate_string`) is freed with
  `hegel_owned_string_free`; a borrowed `const hegel_string_t *` output
  (`hegel_run_result_error`, `hegel_failure_reproduction_blob`) is read before
  its owner is freed and never freed itself.

`Libhegel.check(ctx, code, op)` (and `WasmEngine.check`) funnels every
fallible call: `HEGEL_E_STOP_TEST` → `StopTestError` (the runner unwinds and
calls `mark_complete` with `OVERRUN`), `HEGEL_E_ASSUME` → `AssumeError`
(`mark_complete` with `INVALID`), any other non-OK code → `LibhegelError` /
`EngineError` carrying the context diagnostic.

Calls that cannot fail for the inputs this client gives them (constructors,
frees, setters, result getters) pass a NULL context and `bindLibrary`
presents them as value-returning wrappers on `Bindings`. A new optional
parameter with a behaviour-preserving default (a NULL callback, say) may be
absorbed inside `bindLibrary`'s wrapper, but the absorption must be documented
in the `Bindings` interface docs so a reader can tell a deliberate default from
an oversight.

### Ownership

Neither adapter frees anything implicitly. Every handle the ABI writes back is
released with its matching free, in `finally` blocks so error paths release
too: the runner frees `Context`, `Settings` and `Run`, each test case, the run
result, each failure and each replay test case; the interpreter frees each
collection; engine-owned buffers (`hegel_generate_bytes`,
`hegel_generate_string`, `hegel_generate_timezone`, the zone name of
`hegel_generate_zoned_datetime`) are copied out immediately and then freed.
The exception: `hegel_string_generator_t`, `hegel_date_options_t`,
`hegel_timezone_spec_t`, `hegel_zoned_spec_t` and `hegel_span_spec_t` handles
are cached per schema object (`HandleCache` in `src/generate.ts`) and freed
with the runner.

## Making a change

1. **Diff the header.** `git diff main -- libhegel/include/hegel.h` (or the
   release tag the frontend last shipped against) lists every declaration that
   moved. Walk each function the frontend binds (grep `src/libhegel.ts` for
   its name) and each entry of `signatures` in `src/browser/abi.ts`.
2. **Native: update `bindLibrary`, `Bindings` and `Libhegel` together.**
   - Removed symbol → delete all three and re-route the callers.
   - Changed signature → the koffi declaration grows or shrinks with it.
     Prototype strings (`lib.func("int hegel_x(void* ctx, ...)")`) are the
     default; a function taking a **struct by value** (every `hegel_string_t`
     argument) uses the `fs(name, [types])` form with the anonymous
     `koffi.struct` type objects, `koffi.pointer(stringType)` for optional
     strings and string arrays (pass `null`, an object or an array of
     objects), and `koffi.out(koffi.pointer(t))` for struct out-parameters. A
     `_Out_ void**` that yields a `const hegel_string_t *` is decoded with
     `koffi.decode(ptr, stringType)`. A struct returned by value
     (`hegel_context_last_error`) is declared with `lib.func(name, stringType,
     [...])`.
   - New symbol → bind it **only if the frontend needs it**: the coverage gate
     forbids dead bindings.
   - Changed enum or constant values → the exported `const` objects in
     `src/engine.ts` and the `RESULT_*` codes in `src/libhegel.ts`. Watch for
     renumbering that keeps every name; only a side-by-side header diff
     catches it.
   - Changed struct layout → the `koffi.struct({...})` type objects and the
     matching `Native*` interfaces; field order and widths mirror the C struct.
     The Wasm adapter reads and writes the same structs by hand through
     `DataView`s (`Layout` objects and the `read*`/`write*` helpers at the
     bottom of `src/browser/engine.ts`), with the wasm32 offsets, so a layout
     change touches both. A 64-bit field (`hegel_timestamp_t.second`, every
     field of `hegel_span_t`) comes back from koffi as `number | bigint` and
     is normalised with `BigInt()` in the `Libhegel` method.
3. **Wasm: update `signatures`, then the marshalling.** By-value structs lower
   to a pointer argument; a struct **return** lowers to an sret pointer as
   the first parameter with a void result (list the export in
   `voidOperations`); an `int64_t`/`uint64_t` is `L` (pass a `bigint`), a
   `double` is `D`. `libhegel/scripts/test-wasm-abi.mjs` proves the lowering
   of the in-tree module when it is unclear. In `engine.ts`, strings go in
   through `arena.text` (NULL for `null`), `arena.bytesValue` (always a
   struct, `{NULL, 0}` for `null`) and `arena.stringList`, and come out
   through `memory.string(ptr)` (an 8-byte `hegel_string_t`) or
   `optionalString` (a pointer to one, or NULL).
4. **koffi specifics that bite.** Struct types stay anonymous
   (`koffi.struct({...})`, never named: koffi's named-type registry is
   process-global and survives vitest worker reloads, so a named registration
   throws "duplicate type name" on the second load). Out-parameters are
   single-element JS arrays (`[null]`, `[0]`), read as `out[0]` after the
   call. A raw byte pointer is bound `uint8_t*`, not `char*`, so koffi hands
   back the pointer for `koffi.decode(ptr, "uint8_t", len)`. Pass a `bigint`
   for 64-bit arguments; a 64-bit out-param may come back as
   `number | bigint`, so normalise at the wrapper. When the marshalling is
   unclear from the header, prove it with a throwaway script that
   `koffi.load`s the in-tree build (`node scripts/engine.mjs native`) and
   round-trips one value, then delete the script.
5. **Tests and coverage.** `just typescript::check` enforces 100% coverage,
   including function coverage. Integration coverage comes from the real
   engine: the generator tests exercise the draw paths, `tests/libhegel.test.ts`
   drives the wrapper directly including real error paths, and
   `tests/wasm-*.test.ts` do the same for the Wasm adapter (the parity test in
   `tests/wasm-engine.test.ts` draws fixed values through every struct-passing
   entry point in both engines and compares them). The few branches
   the real engine cannot be driven into (NULL returns, specific result
   codes, corrupt output) use an injected fake `Bindings`, fake Wasm exports
   or `tests/fakeEngine.ts`. Never mock the engine for anything the real
   library can do, and never add coverage-ignore comments.
6. **Verify.** `just typescript::check` (lint, docs, vitest with coverage,
   packaging), then `just typescript::check-browser` for the packed package
   in Chromium, and the smoke fixture against the built `dist/`:

   ```bash
   just typescript::build
   (cd frontends/typescript/tests/smoke && npm install --no-audit --no-fund)
   cd frontends/typescript && HEGEL_LIBHEGEL_PATH=$(node scripts/engine.mjs native) node tests/smoke/run-hegel-test.mjs
   ```

7. **Release notes.** The change ships: write `frontends/typescript/RELEASE.md`
   (see the `changelog` skill) alongside the `libhegel/RELEASE.md` for the
   ABI change, describing what a TypeScript user can observe, not the binding
   mechanics.

## Validation gate: an independent audit

The steps above are done by the context that made the edits, so they share
its blind spots. Close that gap with a fresh-context audit as the final gate:
launch a separate agent that has not seen your edits and whose only job is to
check the bindings against the header. Give it a self-contained prompt:

```
Audit the TypeScript frontend's bindings to libhegel's C ABI for correctness
against the header. Do NOT edit anything — this is a read-only verification.

1. Read libhegel/include/hegel.h and extract every `hegel_*` function it
   declares, with its exact parameter and return types.
2. Cross-check each against frontends/typescript/src/libhegel.ts. A function
   the frontend uses must have (a) a koffi declaration in bindLibrary whose
   parameter types match the C prototype literally — integer widths and
   signedness, hegel_string_t by value versus const hegel_string_t * —
   (b) a Bindings field, and (c) a Libhegel method. A header function with
   NO binding is fine only if nothing in src/ needs it. Flag any bound symbol
   that is NOT in the header.
3. Cross-check each entry of `signatures` in
   frontends/typescript/src/browser/abi.ts against the same prototype under
   the wasm32 lowering: i32 for pointers, bool, enums and by-value structs
   (which lower to a pointer), i64 for 64-bit integers, f64 for doubles; a
   struct return becomes a leading sret pointer and a void result.
4. For every parameter a bindLibrary wrapper absorbs (hardcodes rather than
   exposes on Bindings), check the Bindings interface docs explain the
   absorption. An undocumented absorbed parameter is a BURIED-DEFAULT.
5. Check the struct type objects (date/time/datetime/timestamp/offset/span/
   zoned datetime/timezone info/string) field order and widths against the C
   structs, in both the koffi type objects and the Wasm adapter's `DataView`
   layouts.

Report a table of every header function with OK / MISSING /
SIGNATURE-MISMATCH / BURIED-DEFAULT / UNBOUND-UNUSED for each adapter, and a
final verdict line. List discrepancies explicitly; do not fix them.
```

If it flags a `MISSING` or `SIGNATURE-MISMATCH`, fix it and re-run the gate.
Do not paste your own diff or conclusions into its prompt: the audit is only
independent because the agent rederives the header-to-binding mapping itself.
