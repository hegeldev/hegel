---
title: Getting started
---

Welcome to Hegel! This page will orient first-time Hegel users and direct you to next steps.

<!-- consolidate the best of property-based testing in every language. -->

Hegel is several things. It is an engine, `libhegel`, which implements the core of property-based testing once and exposes it through a C ABI. It is also a family of libraries, one for each language, built on that engine.

When we say "Hegel", we might mean any or all of these things. Your interest in Hegel will therefore determine which you might want to read further about:

- Want to *use* Hegel? Then you want to use one of our client libraries. [hegel-rust](https://github.com/hegeldev/hegel-rust), [hegel-go](https://github.com/hegeldev/hegel-go), [hegel-cpp](https://github.com/hegeldev/hegel-cpp), [hegel-typescript](https://github.com/hegeldev/hegel-typescript), [hegel-java](https://github.com/hegeldev/hegel-java), and [hegel-ocaml](https://github.com/hegeldev/hegel-ocaml) are fully-fledged property-based testing libraries, built on Hegel.
- Want to learn how it fits together or implement your own? See [How Hegel works](/explanation/how-hegel-works), or jump straight to [`hegel.h`](https://github.com/hegeldev/hegel-rust/blob/main/hegel-c/include/hegel.h) for the C ABI that every library binds to.
