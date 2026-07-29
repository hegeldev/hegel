---
title: Installation reference
---

:::note
You may have been directed to this page by an error during the installation of a Hegel library. If that is you, and any part of how we install Hegel or the tradeoffs we made is still confusing or misleading after reading this page, *please* [open an issue against hegeldev/website](https://github.com/hegeldev/website/issues/new) so we can improve it for everyone.
:::

Every Hegel library is a frontend over [`libhegel`](/reference/libhegel), Hegel's engine. `libhegel` is written in Rust, provides data generation, shrinking, and the example database, and is distributed as a native shared library, `libhegel.so` on Linux, `libhegel.dylib` on macOS, `hegel.dll` on Windows.

Your Hegel library loads that shared library and calls it in-process over a C ABI.

Installing a Hegel library therefore involves two things: the library itself, which you install with your language's normal package manager, and a copy of the engine for your platform.

## Getting the engine

Prebuilt, checksummed `libhegel` binaries are published as assets on each [hegel-rust release](https://github.com/hegeldev/hegel-rust/releases), named `libhegel-<goos>-<goarch>.<ext>` with a matching `.sha256` sidecar.

How a library gets hold of that binary depends on what is idiomatic for its ecosystem:

| Library | How it obtains the engine |
| --- | --- |
| [hegel-rust](https://github.com/hegeldev/hegel-rust) | Compiled from source as an ordinary Cargo dependency (`hegeltest-c`) and linked into your test binary. |
| [hegel-go](https://github.com/hegeldev/hegel-go) | Vendored in the module via Git LFS, `go:embed`ed into your binary at build time, and materialized at runtime under `~/.cache/hegel-go/libhegel/<version>/`. |
| [hegel-typescript](https://github.com/hegeldev/hegel-typescript) | One npm package per platform (`@hegeldev/hegel-linux-x64` and friends), listed in `optionalDependencies` so your package manager installs exactly the one matching your host. |
| [hegel-java](https://github.com/hegeldev/hegel-java) | Bundled in the jar as a classpath resource per OS/arch, unpacked at runtime into `~/.cache/hegel-java/libhegel/` (or `$XDG_CACHE_HOME`). |
| [hegel-cpp](https://github.com/hegeldev/hegel-cpp) | Downloaded for your platform during CMake configuration, verified against its published SHA-256, and linked. |
| [hegel-ocaml](https://github.com/hegeldev/hegel-ocaml) | Located at runtime: a sibling `hegel-rust` checkout if there is one, otherwise a checksum-verified download cached under `~/.cache/hegel-ocaml/libhegel/<version>/`. |

Some practical implications:

- Most libraries need no network access at all once the package is installed. The exceptions are hegel-cpp, which fetches it when you configure your build, and hegel-ocaml, which fetches it the first time you run a test.
- Where a library does download the engine, the download happens once and is then cached. If your builds or tests must run in a sandboxed environment without network access, pre-populate the cache or point the library at a local copy (see below). hegel-ocaml also accepts `HEGEL_LIBHEGEL_NO_DOWNLOAD=1` to disable the fallback outright.
- If you upgrade your Hegel library and it bumps its pinned engine version, the next build or test run may be slower while the new version is fetched.

## Using your own engine build

If you need control over which engine binary is used, every library gives you a way to point at one.

The common way is the `HEGEL_LIBHEGEL_PATH` environment variable, pointing at the library file. It takes priority over everything else the library would otherwise try.

- **hegel-go**, **hegel-typescript**, and **hegel-java** load it instead of their bundled copy. hegel-java then also falls back to the OS's standard shared-library search path (`LD_LIBRARY_PATH`, `DYLD_LIBRARY_PATH`) before using the bundled native.
- **hegel-ocaml** accepts either the library file or a directory containing it.

The two build-time exceptions:

- **hegel-cpp** takes a CMake option instead, since the engine is linked rather than loaded: `-DHEGEL_LIBHEGEL_LIBRARY=/path/to/libhegel_c.<ext>`. The version pin and platform mapping are then skipped entirely.
- **hegel-rust** needs no override. The engine is a source dependency, so a `[patch]` entry or path dependency on `hegeltest-c` is the way to substitute your own.

To build offline against a local engine with hegel-java, set `-Dhegel.natives.skip=true` so Maven skips the fetch step, then point `HEGEL_LIBHEGEL_PATH` at your build.

Note that if you supply your own engine, you are responsible for ensuring it is a version compatible with your Hegel library. We do our best to give informative errors where this is not the case, but there are a lot of possible combinations of this and ways things can go wrong, and only the most common have been tested for.

To build the engine yourself, clone [hegel-rust](https://github.com/hegeldev/hegel-rust) and build the `hegel-c` workspace member with `cargo build --release -p hegeltest-c`. The resulting shared library is in `target/release/`. The crate is also published to crates.io as `hegeltest-c`.

## Troubleshooting

This process should ideally be transparent to you. If it breaks without giving a very clear error message about what you need to do to fix it, that's a bug and we'd appreciate it if you reported it.

If a Hegel library fails to start, the two most common causes are that it could not find or load the engine for your platform, and that the engine it found is not the version the library expects. The error should say which. Any information you can give us about the environment that triggered the problem would be appreciated. If you believe the failure is because of a bug in Hegel, please report it [here](https://github.com/hegeldev/hegel-rust/issues).
