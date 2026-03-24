---
title: Installation reference
---

:::note
You may have been directed to this page by an error during the installation of a Hegel library. If that is you, and any part of how we install Hegel or the tradeoffs we made is still confusing or misleading after reading this page, *please* [open an issue against hegeldev/website](https://github.com/hegeldev/website/issues/new) so we can improve it for everyone.
:::

<!-- (TODO lots of stuff to say here, including venvs, HEGEL_SERVER_COMMAND, version pinning, etc) -->

The [hegel-core](https://github.com/hegeldev/hegel-core) server uses [Hypothesis](https://github.com/hypothesisworks/hypothesis) as the underlying library providing data generation, shrinking, and so on. Every Hegel library, regardless of language, therefore has an implicit dependency on Python.

:::note
We recognize this is less than ideal for a number of reasons and will make some people unhappy. Sorry! We decided it was the most pragmatic option.

Our current long-term plan to eliminate this dependency on Python is to rewrite Hypothesis in rust and provide per-language bindings. However, we aren't promising this will happen or committing to any timelines.
:::

At runtime, the first time a Hegel test is run in a test suite, each Hegel library spawns the `hegel-core` server as a subprocess. It does so by invoking the executable entrypoint provided by the `hegel-core` Python package.

Each Hegel library uses the following steps to resolve the `hegel-core` path:

- If the `HEGEL_SERVER_COMMAND` environment variable is set, use that path.
- Otherwise, the library expects [`uv`](https://docs.astral.sh/uv/) to be on the PATH.
  - If `uv` is not on the PATH, the library errors with a message about possible next steps, including how to install `uv` and directing you to this page.
  - If `uv` is on the PATH, the library uses `uv` to install `hegel-core==$VERSION` into a virtual environment located in `.hegel/venv`. `$VERSION` is determined by the version of the Hegel library you have installed, as each Hegel library pins to an exact `hegel-core` version in its source[^1]. It also writes `.hegel/venv/hegel-version`, a text file containing `$VERSION` as ASCII text.
  - If `.hegel/venv` already exists, the Hegel library checks `.hegel/venv/hegel-version`. If that version is different than its pinned `$VERSION`, it re-creates `.hegel/venv` with `hegel-core==$VERSION`.

Some practical implications of this:
- The installation through the `uv` path happens at runtime. If your tests must run in a sandboxed environment without network access, consider using `HEGEL_SERVER_COMMAND`.
- If you upgrade your Hegel library, and the Hegel library happened to bump its `hegel-core` version, your first test run afterwards will be slow as it upgrades the local `hegel-core` library.

Hegel libraries log the installation of the venv and `hegel-core` to `.hegel/venv/install.log`, which might help you narrow down any issues.

## Installing hegel-core manually

If you require greater control over how and when `hegel-core` is installed, use the `HEGEL_SERVER_COMMAND` environment variable.

Where the `hegel-core` binary gets placed depends on how you install `hegel-core`. For example, when using `pip`, the binary is placed into `bin/hegel`, where the `bin/` directory is located inside the corresponding Python environment. You might then set `HEGEL_SERVER_COMMAND=/path/to/.../bin/hegel`.

For knowledgeable Python users, it may be useful to know that Hegel defines its entrypoint as:

```python
[project.scripts]
hegel = "hegel.__main__:main"
```

Note that if you set `HEGEL_SERVER_COMMAND`, you are responsible for ensuring that your Hegel library version is compatible with that `hegel-core` version.

## Troubleshooting

If you run into installation issues, for example a version mismatch on disk, you can delete `.hegel/venv` to force a fresh install.

:::note
Manually removing this directory shouldn't ever be necessary. If you run into an issue and need to do this, *please* open an issue on the appropriate Hegel library repository so we can fix it for everyone.
:::


[^1]: Note that there is no correspondence between `hegel-core` version numbers and Hegel library version numbers.
