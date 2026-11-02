---
name: go-ffi
description: "How the Go frontend (frontends/go) binds libhegel's C ABI through purego in internal/libhegel. Use when changing libhegel's C ABI (hegel.h), when the Go tests fail at library load with a symbol-resolution or version-mismatch error, or when adding, removing or reshaping an ABI function in the Go frontend."
---

# The Go frontend's bindings to libhegel

`frontends/go` calls libhegel, the engine in `libhegel/` of this repository,
through its C ABI (`libhegel/include/hegel.h`) via purego FFI, in
`frontends/go/internal/libhegel/`. There is no pinned release to align to: the
frontend is tested against the in-tree engine (`just go::build-libhegel`), so a
C ABI change and the binding change that follows it land in the same PR. Only
a release commit embeds libhegel binaries (the release automation commits
them); on main `internal/libhegel/libs/` holds empty placeholders.

The main `hegel` package's exported API must **not** change as a result of an
ABI change — only the internal binding layer — **except when libhegel removes
an API**. In that case, remove the corresponding API from the public Go
package too. Do not preserve it with a compatibility shim, emulation, or
another workaround after its upstream libhegel support is gone.

**Within that binding layer, the wrapper API *should* track the C API.** When a
C function gains a parameter, plumb it through the `internal/libhegel` wrapper
method (`Settings.RunStart`, …) rather than hardcoding a constant inside the
wrapper — the wrapper's exported signature is meant to mirror the C signature.
Any behavior-preserving default for a new *optional* parameter (a NULL callback,
a nullable pointer, a `0`-means-default flag) belongs in the **package-`hegel`
callers** (`runner.go`, …), which pass `0`/NULL — not baked into the wrapper.
This keeps the binding honest (a future caller *can* supply a real value) while
holding the `hegel` package's public API stable. Example: `hegel_output_callback_t`
added `(callback, user_data)` to `hegel_run_start`; the wrapper takes both and
the runner passes `0, 0`.

## The context-based ABI

Every fallible libhegel call follows one convention, and the whole wrapper is
shaped around it:

- The **first argument** is a `hegel_context_t` (an error-reporting context).
- The **return value** is an `Error` result code (`OK` is 0; failures are
  negative — `E_INVALID_HANDLE`, `E_INVALID_ARG`, …).
- Any **value the call produces** (a handle, a count, a bool, a byte buffer) is
  written through a **trailing out-parameter**, not returned.
- On a non-`OK` return, the human-readable message is read back from the context
  via `hegel_context_last_error`.

So `symbols` (the loaded library) is immutable and shared across goroutines,
while the per-call error state lives on a `*Context`. The wrapper threads a
`*Context` through every method and funnels every call through two helpers:

- `ctx.invoke(op, fn)` — runs `fn(ctxT)`, and if it returns non-`OK`, reads
  `hegel_context_last_error` and wraps it into a Go `error`.
- `allocate(ctx, op, new, free)` — `invoke` for handle constructors: maps a
  zero out-handle to a `nil` result (the engine's "no object" sentinel, e.g. a
  finished run) and registers a GC cleanup that calls `free`.

## 1. Build the in-tree engine and read its header

The header to bind is `libhegel/include/hegel.h` in the same checkout, and the
library to test against is the in-tree build:

```bash
just go::build-libhegel        # cargo build --release -p hegeltest-c
just go::libhegel-path         # where the build landed
```

**When you need the symbol table**, run `nm -D` against the in-tree build — it
is ground truth for which `hegel_*` symbols exist:

```bash
nm -D "$(just go::libhegel-path)" | grep '^.* T hegel_' | sort
```

Diff it against the registration table in `tryOpen` (`libhegel.go`). A symbol
the wrapper registers but the lib no longer exports makes `registerSymbols`
fail and **every** integration test dies at load.

## 2. Compare hegel.h against `internal/libhegel/libhegel.go`

Walk every C declaration and check it against the wrapper. Categorize each:

- **Removed symbol/API** → delete its `symbols` field, its registration-table
  entry, its `stub.go` closure, and any wrapper method. If it backs an exported
  API in the main `hegel` package, remove that public API and its callers too;
  do not add compatibility code that recreates or retains an API libhegel has
  removed.
- **Renamed/retyped symbol** (e.g. `hegel_run_result_passed`, a `bool`, became
  `hegel_run_result_status`, a `hegel_run_status_t` written through an out-param)
  → update the field signature, the registration name, the wrapper method, and
  the `stub.go` closure's signature/out-param handling.
- **New symbol** → add a `symbols` field, a registration entry, a wrapper method,
  a `stub.go` closure, and a test (see §4).
- **Changed signature** (a new arg, or a value that moved from return to
  out-param) → remember the ABI convention: ctx first, `Error` return,
  produced value through a trailing out-pointer. The `symbols` field type and the
  wrapper method change together (`stub.go` adapts on its own — see §3.3). A new
  arg must be **plumbed through the wrapper method's signature**, not hardcoded
  inside the wrapper. If it is an *optional* arg with a behavior-preserving
  default (a NULL callback, a nullable pointer, a `0`-means-default flag), leave
  the wrapper faithful and pass the default from the **package-`hegel` callers**
  (see the charter note at the top). Do not silently bury a default in the
  wrapper — that hides the new capability and the §7 audit cannot tell a
  deliberate default from an oversight.
- **Changed enum/constant values** (e.g. a new `HEGEL_LABEL_*`, or a reordered
  `hegel_status_t`) → update the Go `const` block. Binding-invented labels that
  aren't in upstream (`LABEL_COMPOSITE`, `LABEL_STATEFUL`) must be renumbered to
  sit *past* the last upstream value so they can't collide.
- **Enum-typed param widened to a fixed-width integer** (e.g.
  `hegel_settings_set_mode(…, hegel_mode_t mode)` became `(…, uint32_t mode)`;
  same for `set_backend`, `set_verbosity`, `mark_complete`'s `status`) → the Go
  enum's **base type must mirror the parameter's exact width and signedness
  literally**, even though it's ABI-transparent. Change `type Mode int32` to
  `type Mode uint32` so it matches the `uint32_t` param, not the (still-signed)
  C `enum` it descends from. This is a real ABI difference to encode even
  though purego marshals a signed and an unsigned 32-bit value into the argument
  register identically for the small non-negative values these enums hold — so
  the integration tests pass either way and a mismatch will **not** be caught by
  the test suite; only a literal read of the header catches it. A base-type
  change needs a `go generate ./internal/libhegel` (§3), but the generated
  `_string.go` is usually byte-identical (the index assertions and string tables
  don't depend on signedness). Note the asymmetry: a value written through an
  out-param typed as the enum pointer (`hegel_run_status_t *`, i.e. `RunStatus`)
  is *not* widened and keeps mirroring the enum (`int32`).

## 3. Editing the binding — the things that move together

These must stay consistent or the build/coverage breaks. In practice only the
first two are hand-edited for a signature change; `stub.go` is reflection-driven
and adapts on its own (item 3), and the wrapper method (item 4) changes only when
the call shape does:

1. **`libhegel.go` — `symbols` struct**: one `func`-typed field per C symbol.
   Mirror the C signature exactly: `func(ctxT, <args>, <*outParam>) Error` for a
   fallible call. The two exceptions are the C functions that return a value
   directly rather than through a result code — only `hegel_context_last_error`
   (`func(ctxT) string`) currently does this.
2. **`libhegel.go` — `registerSymbols` table** in `tryOpen`: `{"hegel_x", &syms.x}`.
3. **`stub.go` — reflection-driven, usually needs no edit**: `Stub(tb, returns
   ...any)` builds a `symbols` by reflection — it walks the struct and installs a
   `reflect.MakeFunc` closure on every `func`-typed field, then returns a
   `*Context`. Because the closures are synthesized from each field's type at
   runtime, **adding, removing, reordering, or retyping a `symbols` field needs no
   change to `stub.go`** — it adapts automatically. Each synthesized
   closure recognizes its out-params **structurally, not by name**: an `out[T]`
   argument by `reflect.Type` identity, or a pointer-to-`~uintptr` as a produced
   handle. Every non-pointer argument (a by-value scalar, a `string`, or the
   always-NULL `outputCallbackT` / `user_data` args) is treated as an input and
   consumes **no** scripted return value. Destructors (field name contains
   `Free`) are detected and auto-free their handle, scripting no return. Values
   are supplied by the caller via `returns ...any` and popped in strict call
   order by a single `retval()` helper. So the thing that moves in lockstep with a
   signature change is not `stub.go` but each test's `returns ...any` list when a
   symbol's **output** type or call order changes (see §4) — a new *input* arg
   that consumes no return value (like the NULL callback) needs no `returns`
   change either.
4. **`libhegel.go` — wrapper method**: a thin, typed Go method that threads a
   `*Context`. Handle constructors live on `Context` (e.g. `SettingsNew`) or on
   the parent handle (e.g. `Settings.RunStart`, `Run.NextTestCase`) and go
   through `allocate(ctx, op, new, free)`. Other fallible calls go through
   `ctx.invoke(op, fn)`. Reader methods (`Result.Status`, `Result.FailureCount`,
   `Failure.Origin`, …) use the reusable `out*` scratch fields on the wrapper
   struct (`TestCase`, `Result`, `Failure`) so the hot per-draw path doesn't
   allocate a fresh out-param on every call.

   A pointer to a string is not a valid Go wrapper API choice. Keep nullable
   pointers where required in the private `symbols` ABI signature, but expose
   strings idiomatically from wrapper methods. In order of preference:

   - Use `""` as the absent/remove sentinel when the empty string is not itself
     a distinct valid value. This applies to both inputs and outputs.
   - When `""` is a valid value and absence must remain distinguishable, use a
     `(string, bool)` pair: accept `value string, present bool` for an input, or
     return `value string, present bool, err error` for a fallible output. Keep
     the string first, following Go's value/comma-ok convention.

   Do not expose `*string` merely to mirror a nullable C `char *`; translate
   between the idiomatic Go representation and NULL at the wrapper boundary.

### Regenerate the stringer output

Enum types are listed in the `//go:generate` directive at the top of
`libhegel.go`. After adding an enum type or changing any enum constant value:

```bash
go generate ./internal/libhegel        # uses `go tool stringer`
```

Changed constant values break the compile-time `_ = x[CONST-N]` assertions in
`libhegel_string.go` until you regenerate, so this is not optional. Add new enum
types to the `-type=` list. The generated `_string.go` is coverage-excluded.

## 4. Coverage and tests

100% per-file coverage is enforced (see the `go-coverage` skill). New or changed
wrapper methods need coverage:

- Methods the runner exercises (result/status handling, generate, spans,
  mark_complete) are covered by the stub-driven tests in `runner_test.go` —
  update those `Stub(...)` return lists when a symbol's output type or call
  order changes (e.g. a `bool` "passed" value becoming a `RunStatus`).
- Methods **not** wired into the runner (pools, state machines, blob replay,
  primitive draws, the unwired settings setters) are covered by direct stub
  tests in `internal/libhegel/stub_test.go`. Build a `*Context` with
  `lib := Stub(...)`, construct the wrapper value in-package against its symbol
  table —
  `tc := &TestCase{pointer: &pointer[testCaseT]{syms: lib.syms, raw: 1}}` or
  `s := &Settings{syms: lib.syms, raw: 1}` — and call the method, passing `lib`
  as the `*Context`. Cover both the happy path and every error branch (a guard
  that rejects bad input, like `cStringArray`'s interior-NUL check, needs a test
  that trips it).
- Prefer covering a method to annotating it `// coverage-ignore`. The ratchet in
  `frontends/go/coverage-ratchet.json` auto-tightens when the annotation count
  drops, so removing now-covered ignores is free and good.

## 5. purego pitfalls

- A symbol that **returns** `const char *` directly → type the field
  `func(ctxT) string` (not `*byte`/`uintptr`). Only `hegel_context_last_error`
  does this today.
- A symbol that **writes** a string through a `const char **` out-param → type
  that argument `**byte` and read it back with `goString`, which copies the
  NUL-terminated buffer immediately (a NULL pointer — libhegel's "no string" —
  yields `""`). This is how `hegel_version`, `hegel_run_result_error`, and the
  failure getters return strings.
- Produced **handles and scalars** come back through trailing out-params
  (`*settingsT`, `*runT`, `*uint64`, `*bool`, `*int64`, `*Collection`,
  `*RunStatus`), never as the return value — the return is always the `Error`
  code.
- **Keeping Go memory alive across a call is automatic — do not thread a
  `keepalive` return value or sprinkle `runtime.KeepAlive`.** The GC keeps Go
  memory reachable for the duration of a call as long as it is passed as a
  *typed pointer* (`*byte`, `**byte`, `*Date`, …) rather than flattened to a
  `uintptr`. A typed pointer argument is a live root through the call, and the
  GC traces transitively from it (a `**byte` pins the pointer array *and* every
  buffer its elements point at). Memory is lost only if you drop to `uintptr` —
  then the GC can't see it and may free/move it mid-call. So a helper that
  produces a C-facing pointer should return just `(ptr, len)`; a separate
  `keepalive` slice is a code smell that means someone thought a typed pointer
  needs propping up. It doesn't.
- `const char *` **argument** → pass a Go `string`; purego manages the memory.
- A **length-delimited byte buffer** argument (`const uint8_t *` plus a `size_t`
  length, e.g. `include_characters`) → build it with `cString`, the inverse of
  `goString`: it aliases the Go string's bytes via `unsafe.StringData` (no
  `[]byte` copy) and returns `(*byte, len)`. A nil/empty string yields a NULL
  pointer. No keepalive — the returned `*byte` is what keeps the string
  reachable (see the bullet above).
- `const char *const *` (array of C strings) is **not** handled automatically.
  `cStringArrayArg` builds a `[]*byte` of NUL-terminated buffers and returns
  `(cStringArrayPtr, len, err)`. The typed `**byte` return keeps both the
  pointer array and its buffers reachable across the call — no keepalive. A Go
  string may contain an interior NUL but a C string can't, so `cStringArrayArg`
  rejects such input with an error rather than letting C silently truncate it. A
  nil slice yields a NULL pointer ("absent"); a non-nil empty slice yields a
  non-NULL, length-0 pointer ("the empty set"), which libhegel treats as
  distinct.
- A **struct passed by value** (e.g. `hegel_date_t` / `hegel_time_t` /
  `hegel_datetime_t` as a `hegel_generate_*` bound) needs **explicit padding
  fields**. purego packs struct fields into argument registers sequentially by
  field size and only flushes an eightbyte at an exact 64-bit boundary — it
  does *not* honor the struct's natural alignment padding. So a Go struct whose
  C counterpart has interior or trailing padding (like `hegel_time_t`, where
  `microsecond` sits at offset 4 after a 1-byte pad) must spell that padding out
  as real blank fields (`_ uint8`, `_ uint16`, …) at the right offsets, or the
  fields land in the wrong registers and the C side reads garbage (the classic
  symptom: a drawn bound like `23:59:59` arriving as `59:63:66`). Add the padding
  directly to the exported struct — the same explicit layout also matches C in
  memory, so one struct type serves both the by-value inputs and the pointer
  output, with no separate ABI/conversion type. Blank padding fields marshal
  fine (purego reads them as their zero value). Structs of two eightbytes or
  fewer pass in registers; a `//go:generate`-free way to sanity-check a new one
  is a throwaway real-lib draw asserting the bound round-trips. This affects only
  by-**value** struct arguments; a struct written through an out-pointer
  (`*Date`) uses the natural in-memory layout and needs no special handling
  beyond the shared padded definition.

## 6. Verify

```bash
just go::check    # lint + check-docs + the coverage suite against the in-tree build
```

Confirm the end-to-end smoke test actually ran against the real lib (not skipped):

```bash
just go::check-test-no-coverage -run TestLibhegelEndToEnd -v ./internal/libhegel/
```

## 7. Validation gate — independent completeness audit

The steps above are done by the same context that made the edits, so they share
its blind spots: a symbol you never noticed in the header is a symbol you also
won't notice is unbound. Close that gap with a **fresh-context audit** as the
final gate. Launch a separate agent (`subagent_type: "general-purpose"`) that
has *not* seen your edits and whose only job is to diff the header against the
binding and report anything missing.

Give the agent a self-contained prompt — it starts with no context, so spell
out the two files to compare and the exact output you want:

```
Audit the Go frontend's libhegel FFI binding for completeness against the C
header. Do NOT edit anything — this is a read-only verification.

1. The header is libhegel/include/hegel.h; the binding is
   frontends/go/internal/libhegel/libhegel.go (both in this checkout).
2. Extract every `hegel_*` function declared in the header.
3. Cross-check each against the binding: it must have (a) a field in the
   `symbols` struct, (b) an entry in the `registerSymbols` table in `tryOpen`,
   and (c) a wrapper method (or a documented reason it has none). Also flag any
   registered symbol that is NOT in the header (a stale binding that would break
   `registerSymbols` at load).
4. For every symbol, confirm the Go field signature matches the C prototype
   under the context-based ABI (ctx first, Error return, produced values via
   trailing out-params).
5. Check every integer parameter's width AND signedness against the C type,
   including enum-typed params. A C param typed `uint32_t` (even one that
   carries a semantically-enum value, e.g. `hegel_mode_t`/`hegel_status_t`
   values passed through a widened `uint32_t` slot) must be bound by a
   Go type whose base is `uint32`, not `int32` — flag a signed Go enum
   bound to an unsigned C param as SIGNATURE-MISMATCH. This mismatch is
   invisible to the runtime tests (purego marshals both identically for small
   values), so this literal-read check is the only thing that catches it.
6. For every parameter the change under review adds to the header (compare
   against `git diff origin/main -- libhegel/include/hegel.h`), check the
   corresponding `internal/libhegel` wrapper method: is the new arg forwarded
   through the wrapper's own signature, or hardcoded to a constant inside the
   wrapper? A new arg hardcoded inside the wrapper (rather than plumbed to the
   wrapper signature and defaulted at the package-`hegel` caller) is a
   BURIED-DEFAULT — flag it. This is a design-policy check, not an ABI failure.

Report a table of every header function with OK / MISSING / SIGNATURE-MISMATCH /
BURIED-DEFAULT, and a final verdict line. List discrepancies explicitly; do not
fix them.
```

The agent's report is the gate: if it comes back clean, the binding is
complete. If it flags a `MISSING` or `SIGNATURE-MISMATCH`, return to §2–§4,
fix it, and re-run this gate. This is a genuine independent check only because
the agent rederives the header→binding mapping from scratch — do not paste your
own diff or conclusions into its prompt.
