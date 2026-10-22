# Hegel

> [!IMPORTANT]
> Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel/issues/new). See https://hegel.dev/compatibility for details on what may still change.

[Hegel](https://hegel.dev) is property-based testing, based on [Hypothesis](https://github.com/hypothesisworks/hypothesis): a native engine that does the generating, shrinking and bookkeeping, and thin frontends that give it an idiomatic surface in each language.

This is the Hegel monorepo that includes the engine and (currently only some of) our officially supported frontends.
Specifically, right now it contains:

| Component | Path | What it is |
| --- | --- | --- |
| libhegel | [`libhegel/`](libhegel/) | The engine, with a C ABI. Published as prebuilt libraries on [GitHub releases](https://github.com/hegeldev/hegel/releases) and as the `hegeltest-c` crate. |
| Hegel for Rust | [`frontends/rust/`](frontends/rust/) | The [`hegeltest`](https://docs.rs/hegeltest) crate. |
| Hegel for Python | [`frontends/python/`](frontends/python/) | The [`hegeltest`](https://pypi.org/project/hegeltest/) package, imported as `hegel`. |
| Hegel for TypeScript | [`frontends/typescript/`](frontends/typescript/) | The [`@hegeldev/hegel`](https://www.npmjs.com/package/@hegeldev/hegel) package, for Node, Bun, Deno and browsers. |
| Hegel for C++ | [`frontends/cpp/`](frontends/cpp/) | The `hegel` CMake library, fetched from this repository with `FetchContent`. |
| Hegel for Java | [`frontends/java/`](frontends/java/) | The [`dev.hegel:hegel`](https://central.sonatype.com/artifact/dev.hegel/hegel) (Java 22+) and [`dev.hegel:hegel-jna`](https://central.sonatype.com/artifact/dev.hegel/hegel-jna) (Java 17+) artifacts on Maven Central. |

Each component is versioned and released independently; releases are tagged `<path>/<version>`, for example `libhegel/0.47.1`, `frontends/rust/0.50.1`, `frontends/python/0.1.0`, `frontends/typescript/0.4.10`, `frontends/cpp/0.13.1` or `frontends/java/0.10.2`.

See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.
