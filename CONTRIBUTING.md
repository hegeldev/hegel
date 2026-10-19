# Contributing

First: It's great that you're interested in contributing to Hegel! Thanks!

> [!NOTE]
> This guide will get fleshed out over time. In short, we welcome contributions, and will happily walk you through any complications you encounter during a contribution—just ask!

This repository holds the Hegel engine (`libhegel/`) and the language frontends built on it (`frontends/<language>/`). Hegel uses [`just`](https://github.com/casey/just) for development commands:

```bash
just test
just format
# full pre-PR check: lint + every component's tests
just check
# one component
just libhegel::check
just rust::check
just python::check   # needs uv, and `just python::setup` once
```

Of course, you can also run the underlying `cargo` commands if you prefer not to install `just`. The Python frontend needs [`uv`](https://docs.astral.sh/uv/) as well.

Every pull request that changes a component's source needs a `RELEASE.md` in that component's directory; see `frontends/rust/RELEASE-sample.md` or `frontends/python/RELEASE-sample.md`.
