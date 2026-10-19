RELEASE_TYPE: patch

This patch points the `libhegel` download fallback at libhegel's new home: since 0.47.2, libhegel is released from the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo as GitHub releases tagged `libhegel/<version>`. Earlier versions, including the currently pinned one, are still fetched from hegel-rust's `libhegel-v<version>` releases, so nothing changes for existing installs.
