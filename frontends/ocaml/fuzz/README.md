# ocaml_diff: differential testing of OCaml compilers

`ocaml_diff` generates random OCaml programs and compiles each one with many
compilers and flag sets. It reports a program if the backends disagree on its
output, if some backends compile it and others don't, or if any backend
crashes. It is built on Hegel, so every failure is shrunk to a small program
before it is reported.

```
dune exec fuzz/ocaml_diff.exe -- [--sample] [--ox] [TEST_CASES]
```

- `TEST_CASES`: how many programs to try. Defaults to 200, or 5 with
  `--sample`.
- `--sample`: print the generated programs instead of compiling them, with a
  `(* ---- *)` line between programs.
- `--ox`: also generate OxCaml-only features, and test only OxCaml (flambda2)
  switches.

## Files

- `ocaml_diff_gen.ml`: the program generator. Its only export is
  `program ?ox ?effect_syntax tc`, which returns a program's source.
- `ocaml_diff.ml`: the driver. It finds the backends, compiles and runs each
  program, compares the results and filters out known bugs.
- `run.sh` and `watch.sh`: scripts for long unattended runs (see below).

## Long runs

```
OCAML_DIFF_SWITCHES=ocaml-trunkfix,ocaml-trunkfix-flambda fuzz/run.sh <dir> <hours> <workers>
fuzz/watch.sh <dir>
```

`run.sh` builds the fuzzer and copies the binary to `<dir>/fuzz.exe`, so you
can keep editing and rebuilding without disturbing the run. It then starts
`<workers>` fuzzers, each asking for a million programs. A fuzzer stops at its
first failure, and the worker then starts a fresh one. Worker `w`'s `i`-th
fuzzer logs to `<dir>/w<w>-<i>.log`, so every log except the last one of each
worker holds one failure, with its shrunk program. Programs that hit known
bugs go to `<dir>/known/`. The run stops after `<hours>`, or once
`<dir>/stop` exists. Environment variables such as `OCAML_DIFF_SWITCHES` pass
through to the fuzzers.

To update a running fuzzer, replace `<dir>/fuzz.exe`: each worker picks up the
new binary when it next restarts. Replace it with `mv`, not `cp`, so that a
binary that is still running is never overwritten.

`watch.sh` waits until some log contains `Exception:` (a failure), then prints
that log's name and exits. It also stops the run, by creating `<dir>/stop`,
in two cases:
- more than 40 logs appear, which means workers are failing straight away
  (for example, because the repo has moved);
- the disk containing `<dir>` has less than 10GB free.

It exits on its own when the run ends. Running it with `run_in_background` in
Claude Code gives a notification when something happens.

## The generated programs

Every program is well typed, terminates, and prints output that the language
fully determines. So if two correct compilers ran it, they would print the same
thing. The generator keeps these rules:

- **Determinism.** Argument evaluation order is unspecified, and it differs
  between `ocamlc` and `ocamlopt`. So any subexpression with an effect is
  let-bound before it is used (performs, atomic updates, mutation inside a
  loop). All NaNs print as `nan`, because constant folding doesn't keep NaN's
  sign the same everywhere. Results of `compare` go through `sgn`, because only
  their sign is specified.
- **Totality.** Division, `mod` and `String.sub` go through the prelude's
  `sdiv`, `smod`, `i64div`, `i64rem` and `ssub`, which can't fail. Loop bounds
  are masked (`land 7`). Recursive functions count down a masked counter.
  Programs don't rely on bounds checks, which is why `-unsafe` backends are
  allowed.
- **Exceptions.** The only exception a program raises on purpose is `Exit`,
  and only inside a `try` or an `exception Exit` match arm. Any other
  uncaught exception, or an `Exit` that escapes, counts as a crash.

A program has a prelude (the helpers above plus a small GADT evaluator), up to
four type declarations, possibly some effect declarations and global atomics,
and then one to six top-level bindings. Each binding is printed with a
printer generated for its type:

```ocaml
let v3 : (int * string) list = <expression>
let () = print_endline (<printer for (int * string) list> v3)
```

### Types

`int`, `bool`, `float`, `char`, `int64`, `string`, lists, options, pairs,
functions, and the program's declared records and variants. A declared type
may only use earlier ones. Records have mutable and immutable fields. Variants
are sometimes polymorphic variants, and one may extend another. With `--ox`
there is also `float#`, which can only be a binding, a function argument or
result, or a record field.

### Expressions

`expr ctx env ty depth` builds an expression of type `ty`. `env` tracks the
variables in scope, any OxCaml stack-allocated locals, whether the expression
is inside a `try`, and which effects a surrounding handler handles. When depth
runs out it returns a variable or a literal; a literal is sometimes wrapped in
`Sys.opaque_identity` so the compiler can't fold it away. Otherwise it picks
one of these:

- **Generic forms** that work at any type: `if`, `match` with nested
  patterns, guards and or-patterns, `let`/`let rec`, local functions, partial
  application with labelled and optional arguments, and `try ... with Exit`.
  Some get inlining attributes (`[@inline never]`, `[@inlined]`, and with
  `--ox` `[@unrolled]` and `[@loop]`).
- **Type-specific operations**, chosen more often so that values get computed
  rather than just passed around: arithmetic and comparisons, `Int64`,
  `Char`, `String`, `Bytes` and `Buffer` operations, list functions, record
  field updates inside loops, and so on.
- **Targeted shapes**, each aimed at one part of the compiler:
  - `pressure`: many ints and floats live across an opaque call, so some must
    be spilled to the stack.
  - `many_args`: a recursive function with more parameters than arm64 passes
    in registers.
  - `gc`: fresh allocations stay live across `Gc.minor`, `Gc.full_major` or
    `Gc.compact`, then get used.
  - `local_exn`: a local exception carries a value out of a function.
  - `lazy_value`: a `lazy` that is forced where its value is used.
  - `gadt`: a term of the prelude's GADT.
  - `atomic_op`, `local_atomic`: `Atomic` updates in a loop.
  - `str_load`: string reads at offsets inside the string's padding.
  - Effects: deep handlers (`handler`), shallow handlers that resume the
    fiber again and again (`shallow`), and OCaml 5.3's
    `match ... with effect` syntax (`effect_match`). Handlers may continue,
    discontinue with `Exit`, or drop the continuation.
  - With `--ox`: `let local_`/`stack_`/`exclave_` bindings and `float#`
    arithmetic.

Literals favour edge values such as `min_int`, `max_int`, `-0.`, subnormals
and infinities.

### Using what is bound

A function whose parameter is never used tests less than it seems to: the
compiler just drops the argument. This is the problem studied by Frank,
Quiring and Lampropoulos in "Generating Well-Typed Terms That Are Not
'Useless'" (POPL 2024). Their fix delays choosing types; ours is a cheaper,
local version. `ctx.used` records each variable the program has referred to
so far, and three choices favour the ones that haven't been used yet:

- when a variable of the right type is emitted;
- when `from_env` picks the type a subexpression eliminates (a list to fold,
  an option to match, and so on);
- when an unused first-order variable has a different type from the
  expression being built, which sometimes becomes
  `if x = <literal> then ... else ...`.

Over 200 sampled programs, this cut the share of unused function parameters
from 37% to about 27%, with programs about the same size.

### Swarm testing

Each program turns on a random subset of features: `atomics`, `deep`,
`effect_syntax`, `effects`, `exits`, `exns`, `gadt`, `gc`, `lazy`, `locals`,
`loops`, `manyargs`, `opaque`, `partial`, `poly`, `records`, `shallow` and
`strings`. A feature that is off is never generated. A feature that is on has
its forms repeated five times (`boost`) among the choices, so it actually
shows up. The program's first line lists its features
(`(* features: ... *)`). Because the feature flags are Hegel draws, shrinking
turns off the ones a failure doesn't need.

### Shrinking

Every choice the generator makes is a Hegel draw. Each `expr` call runs inside
its own labelled span, so Hegel can recognise subexpressions and shrink them
(for example, replacing one with a smaller one). A failure is reported as a
small program built by the same generator, so it is still well typed and
deterministic.

## Backends

`discover` lists the opam switches. `OCAML_DIFF_SWITCHES=a,b` limits it to
the switches named. Each switch contributes:

- **Without `--ox`** (OxCaml switches are skipped): `ocamlc`, `ocamlopt` and
  `ocamlopt -linscan`. A non-flambda switch adds
  `ocamlopt -unsafe -inline 1000`. A flambda switch adds `-O3`, `-Oclassic`,
  `-O3 -linscan`, and an aggressive `-O3` (`-unbox-closures -inline 1000
  -rounds 4 -inline-max-depth 3 -remove-unused-arguments -unsafe`).
- **With `--ox`** (flambda2 switches only): `ocamlc`, and `ocamlopt` at
  `-Oclassic`, the default level and `-O3`.

`OCAML_DIFF_EXTRA="name=compiler flags;..."` adds more backends. Effect
syntax is only generated when every backend is OCaml 5.3 or later.

Compilers run with `-w -a`. Every backend compiles at the same time, each in
its own temporary directory, with `OCAML*`, `CAML_LD_LIBRARY_PATH` and
`OPAM_SWITCH_PREFIX` removed from the environment. Compiling times out after
120 s and running after 10 s. A program that times out on any backend is
discarded; it is not counted as a failure.

## Deciding the result

For each program, `check` does the following, in order:

1. If no backend compiled it, it fails. The message is a compiler crash if
   any compiler printed `Fatal error`, and a generator bug otherwise.
2. If any run timed out, the program is discarded.
3. If the result matches a known bug (see below), the program is discarded and
   counted.
4. If any backend was killed by a signal, or exited with an uncaught
   exception other than `Exit`, it fails with "backend crashed". This applies
   even if every backend crashed the same way.
5. If the backends' results differ, it fails with "backends disagree". A
   result is either a compile error, or an exit status plus stdout.

A failure message groups the backends that gave the same result, and shows
each group's exit status and output.

After the first failure, `focus` keeps only the first backend and the first
one that disagreed with it. So each shrinking step compiles two programs
instead of a dozen. Hegel then prints the shrunk program (through `note`) and
a `failure_blobs` line that reproduces it.

### Known bugs

`known_bug` recognises bugs that are already found, so a long run can keep
looking for new ones. Each check looks at which backends disagree with the
majority result (the "odd" backends), and confirms the bug by behaviour rather
than by the text of the source:

- **ocaml/ocaml#15154:** `ocamlopt` without flambda fails to compile some
  programs, with `Selection.size_expr` in the error.
- **Unreported effect-handler bug:** after a nested `perform`, native code
  leaves a shallow fiber's effect handler pointing into the minor heap. A
  program matches when every odd backend died with SIGBUS or SIGSEGV, and every
  one of them, rebuilt with `-runtime-variant d`, fails
  `Assertion failed: !Is_young(v)`.
- **ocaml/ocaml#15189:** trunk's `-linscan` miscompiles native `Atomic`
  operations. A program matches when the odd backends are all trunk `-linscan`
  ones and they agree with the majority once `Atomic` is replaced by ordinary
  functions (`atomic_shim`). To keep this bug from hiding others, `-linscan`
  backends always get the shim when the program uses `Atomic`.

With `OCAML_DIFF_SAVE=dir`, each program that matched a known bug is written
to `dir/known<pid>-<n>.ml`, so the filter can be checked.

When the run ends, it prints how many programs it ran, how many timed out, and
how many matched known bugs.
