RELEASE_TYPE: patch

This patch moves Hegel for OCaml into the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo, next to the engine it drives, where it is built and tested against the in-tree libhegel. The opam packages are used exactly as before, and the bundled engine stays at libhegel 0.48.0.

The library no longer downloads libhegel. Where it used to fall back to a checksum-verified download from the libhegel GitHub release, it now looks for `$HEGEL_LIBHEGEL_PATH`, then the engine bundled into the installed package, then the in-tree build of a hegeldev/hegel checkout (`target/release/libhegel_c.<ext>`, or `target/debug/`, under `$CARGO_TARGET_DIR` or the nearest directory above the working directory that has one), and fails with a message saying so when none is found. `opam install hegel` is unaffected, since every release bundles its engine; a git pin of the package now needs `HEGEL_LIBHEGEL_PATH` pointing at a libhegel library, and the `HEGEL_LIBHEGEL_NO_DOWNLOAD` variable no longer does anything. To pin the development version, pin the monorepo's `frontends/ocaml` subpath:

```bash
opam pin add --subpath frontends/ocaml hegel "git+https://github.com/hegeldev/hegel.git"
```
