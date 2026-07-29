---
title: Compatibility
---

:::caution
TL;DR: Hegel is in beta. We aim for but do not guarantee stability. Check back in a few months for a stable release!
:::

Hegel is in beta. While we take minimizing breakage seriously and do not expect to make sweeping API changes, we do expect to make some amount of breaking changes as we refine the API.

In particular, if we think a breaking change will result in a better property-based testing API, we will make it. Of course, we will not do so without fully communicating the change and migration path.

If the possibility of breaking changes bothers you, please check back in a few months when we expect to have released a stable 1.0!

## Versioning

While Hegel is in beta, we have adopted the following version scheme for [libhegel](https://github.com/hegeldev/hegel-rust/tree/main/hegel-c) and all Hegel libraries:

- Minor `0.N.0` releases for changes that might be breaking.
- Patch `0.0.N` releases for any other change.

This is effectively SemVer shifted right by one decimal place.

Our first stable release will be `1.0.0`.

## Platform support

Every Hegel library runs the [libhegel](https://github.com/hegeldev/hegel-rust/tree/main/hegel-c) engine in-process.

Prebuilt `libhegel` binaries are published for `linux/amd64`, `linux/arm64`, `darwin/arm64`, `windows/amd64`, and `windows/arm64`. Intel macOS (`darwin/amd64`) is **not** published. Build the engine from source if you need it.

- **Linux** (amd64 and arm64) and **macOS on Apple Silicon** are fully supported across the Hegel libraries.
- **Windows** (amd64 and arm64) is supported by [hegel-go](https://github.com/hegeldev/hegel-go), [hegel-typescript](https://github.com/hegeldev/hegel-typescript), and [hegel-cpp](https://github.com/hegeldev/hegel-cpp). [hegel-rust](https://github.com/hegeldev/hegel-rust) supports it too, though that support is still somewhat experimental. [hegel-java](https://github.com/hegeldev/hegel-java) and [hegel-ocaml](https://github.com/hegeldev/hegel-ocaml) currently work on Linux and macOS only.
- **Intel macOS** has no published engine binary. Libraries that let you point at your own build (via `HEGEL_LIBHEGEL_PATH`, or `-DHEGEL_LIBHEGEL_LIBRARY` for hegel-cpp) will work against a locally built `libhegel.dylib`.
