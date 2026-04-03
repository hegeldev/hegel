---
title: Installation reference
---

:::note
You may have been directed to this page by an error during the installation of a Hegel library. If that is you, and any part of how we install Hegel or the tradeoffs we made is still confusing or misleading after reading this page, *please* [open an issue against hegeldev/website](https://github.com/hegeldev/website/issues/new) so we can improve it for everyone.
:::

The [hegel-core](https://github.com/hegeldev/hegel-core) server uses [Hypothesis](https://github.com/hypothesisworks/hypothesis) as the underlying library providing data generation, shrinking, and so on. Every Hegel library, regardless of language, therefore has an implicit dependency on Python.

:::note
We recognize this is less than ideal for a number of reasons and will make some people unhappy. Sorry! We decided it was the most pragmatic option.

Our current long-term plan to eliminate this dependency on Python is to rewrite Hypothesis in rust and provide per-language bindings. However, we aren't promising this will happen or committing to any timelines.
:::

At runtime, the first time a Hegel test is run in a test suite, each Hegel library spawns the `hegel-core` server as a subprocess.

Each Hegel library uses the following steps to run `hegel-core`:

- If the `HEGEL_SERVER_COMMAND` environment variable is set, use that command directly.
- Otherwise, the library uses [`uv tool run`](https://docs.astral.sh/uv/reference/cli/#uv-tool-run) to run `hegel-core` with a specific version that the library has been tested with.
- The first time this is run, it will install a virtualenv in `~/.cache/uv` (or `$XDG_CACHE_HOME/uv` if set), after which that will be reused until the needed version changes. `uv` will be found as follows:
  - If `uv` is already on the PATH, it uses that.
  - If `uv` is not on the PATH, the library automatically downloads a private copy of `uv` to `~/.cache/hegel/uv` (or `$XDG_CACHE_HOME/hegel/uv` if set). This copy is not added to your PATH.
  - The library then uses `uv tool run` to run `hegel-core==$VERSION`, where `$VERSION` is determined by the version of the Hegel library you have installed, as each Hegel library pins to an exact `hegel-core` version in its source[^1].

Some practical implications of this:
- The download of `uv` (if needed) and the first run of `hegel-core` happen at runtime. If your tests must run in a sandboxed environment without network access, consider using `HEGEL_SERVER_COMMAND`.
- If you upgrade your Hegel library, and the Hegel library happened to bump its `hegel-core` version, your first test run afterwards may be slow as `uv` fetches the new version.

## Installing hegel-core manually

If you require greater control over how and when `hegel-core` is installed, use the `HEGEL_SERVER_COMMAND` environment variable.

Where the `hegel-core` binary gets placed depends on how you install `hegel-core`. For example, when using `pip`, the binary is placed into `bin/hegel`, where the `bin/` directory is located inside the corresponding Python environment. You might then set `HEGEL_SERVER_COMMAND=/path/to/.../bin/hegel`.

For knowledgeable Python users, it may be useful to know that Hegel defines its entrypoint as:

```python
[project.scripts]
hegel = "hegel.__main__:main"
```

Note that if you set `HEGEL_SERVER_COMMAND`, you are responsible for ensuring that your Hegel library version is compatible with that `hegel-core` version. We do our best to give informative errors where this is not the case, but there are a lot of possible combinations of this and ways things can go wrong, and only the most common have been tested for.

## Troubleshooting

This process should ideally be transparent to you. If it breaks without giving a very clear error message about what you need to do to fix it, that's a bug and we'd appreciate it if you reported it.

The most useful source of information is that `hegel-core` server's `stderr` is piped to `.hegel/server.log` and should contain any errors that occurred during installation, but any information you can give us about the environment that triggered the problems would be appreciated.

[^1]: Note that there is no correspondence between `hegel-core` version numbers and Hegel library version numbers.
