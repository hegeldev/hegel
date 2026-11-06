RELEASE_TYPE: minor

This release links Hegel's engine (`hegeltest-c`) into your tests as an ordinary Rust dependency again, instead of building it into a `libhegel_c` shared library from the build script and loading it at runtime. `hegeltest` no longer has a build script, so it no longer runs a nested `cargo build`, which deadlocked when cargo's `build.build-dir` was set to a shared directory and ignored offline, vendored and locked builds. Binaries built with hegeltest, such as a deployed `#[hegel::main]` fuzzer, are self-contained again and no longer need the library shipped next to them. The `HEGEL_C_LIB_DIR` environment variable no longer has any effect on hegeltest.

The engine's dependencies now appear in your cargo graph. They are few and built with default features disabled.

The `static-engine` feature is now a no-op, kept so that existing `Cargo.toml` files continue to build. You can remove it.
