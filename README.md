# Hegel

> [!IMPORTANT]
> Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel/issues/new). See https://hegel.dev/compatibility for details on what may still change.

[Hegel](https://hegel.dev) is property-based testing, based on [Hypothesis](https://github.com/hypothesisworks/hypothesis): a native engine that does the generating, shrinking and bookkeeping, and thin frontends that give it an idiomatic surface in each language.

| Component | Path | What it is |
| --- | --- | --- |
| libhegel | [`libhegel/`](libhegel/) | The engine, with a C ABI. Published as prebuilt libraries on [GitHub releases](https://github.com/hegeldev/hegel/releases) and as the `hegeltest-c` crate. |
| Hegel for Rust | [`frontends/rust/`](frontends/rust/) | The [`hegeltest`](https://docs.rs/hegeltest) crate. |

Each component is versioned and released independently; releases are tagged `<path>/<version>`, for example `libhegel/0.47.1` or `frontends/rust/0.50.1`. See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.
