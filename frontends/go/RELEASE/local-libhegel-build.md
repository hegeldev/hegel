RELEASE_TYPE: patch

This patch moves the Go frontend into the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo, where it is now released and tested together with libhegel. The module path stays `hegel.dev/go/hegel`, and tagged releases still embed libhegel, so `go get` works as before. This release also updates the bundled libhegel engine to 0.50.0.

A build of an untagged version, such as one a `replace` directive points at a local checkout of the monorepo, embeds no libhegel. It now loads the libhegel built in the nearest `hegel` checkout instead: the first `hegel/target/` in the working directory or any of its parents, taking its `release/` build (what `cargo build --release -p hegeltest-c` leaves) over its `debug/` one. If that checkout has no build, the error says so rather than looking further up. `HEGEL_LIBHEGEL_PATH` still takes precedence.
