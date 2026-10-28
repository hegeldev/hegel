# Hegel for OCaml

Hegel is a property-based testing library for OCaml based on
[Hypothesis](https://github.com/hypothesisworks/hypothesis), and runs the native
[libhegel](https://hegel.dev/reference/libhegel) backend in-process.

## Install Hegel

```bash
opam install hegel
```

The Hegel version in [opam](https://opam.ocaml.org/packages/hegel/) sometimes lags behind the version in GitHub. To pin the version in GitHub (the frontend lives in the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo, under `frontends/ocaml`):
```bash
opam pin add --subpath frontends/ocaml hegel "git+https://github.com/hegeldev/hegel.git"
```

Hegel calls the native `libhegel` shared library and locates it automatically
at runtime. It looks for the following in order:

1. `$HEGEL_LIBHEGEL_PATH`, an explicit path to the library (or a directory
   containing it)
2. a copy bundled into the installed package: every opam release ships the
   matching engine, so `opam install hegel` needs nothing else
3. the in-tree build of a [hegeldev/hegel](https://github.com/hegeldev/hegel)
   checkout: `target/release/libhegel_c.<ext>` (then `target/debug/`) under
   `$CARGO_TARGET_DIR`, or under the nearest directory above the current
   working directory that has one — what `cargo build --release -p hegeltest-c`
   produces.

A git pin ships no engine, so set `HEGEL_LIBHEGEL_PATH` to a library from the
matching [libhegel release](https://github.com/hegeldev/hegel/releases) (the
`libhegel/<version>` tags) or build one in a checkout.

Hegel for OCaml supports **Linux** (amd64/arm64) and **macOS** (Apple Silicon).
macOS amd64 (Intel) has no published `libhegel` artifact, so on that platform set
`HEGEL_LIBHEGEL_PATH` to a locally built `libhegel_c.dylib`.

## Quick start

Hegel works with whatever test framework your project already uses. The examples below use
[Alcotest](https://github.com/mirage/alcotest).

Add `hegel` and `alcotest` to your dune test stanza:

```
(test
 (name my_tests)
 (libraries hegel alcotest)
 (preprocess (pps ppx_hegel_test)))
```

Write a property test using `let%hegel_test`:

```ocaml
open Hegel

let%hegel_test commutative_addition tc =
  let a = draw tc (integers ~min_value:(-1000) ~max_value:1000 ()) in
  let b = draw tc (integers ~min_value:(-1000) ~max_value:1000 ()) in
  assert (a + b = b + a)
;;

let () =
  Alcotest.run
    "my_tests"
    [ "properties", [ Alcotest.test_case "commutative addition" `Quick commutative_addition ] ]
;;
```

Run `dune runtest`. Hegel generates up to 100 random input pairs and reports the
minimal counterexample if it finds one. When a test fails, Hegel prints a
report naming each value you drew from the failing case (`let a = … in`,
`let b = … in`, named after the `let` binding) and a copy-pasteable line to replay it. Alcotest
reports the test as failed and exits non-zero.

To override the default settings, attach a `[@@settings ...]` attribute:

```ocaml
let%hegel_test commutative_addition tc =
  let a = draw tc (integers ()) in
  let b = draw tc (integers ()) in
  assert (a + b = b + a)
[@@settings Settings.create ~test_cases:500 ()]
;;
```

For a full walkthrough, see the [docs](https://hegel.dev/ocaml).

## Development

The frontend is developed in the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo and built and tested against the engine in `libhegel/`:

```bash
just ocaml::setup   # install the opam dependencies (needs opam)
just ocaml::check   # lint + docs + tests with 100% coverage, against the in-tree libhegel
just ocaml::test    # tests only
```

See `AGENTS.md` for the architecture and `../../CONTRIBUTING.md` for how to contribute.
