---
title: Getting started
---

Welcome to Hegel! This page will orient first-time Hegel users and direct you to next steps.

Hegel exists because of a problem: writing a good property-based testing library is hard. Writing a good property-based testing library in *every language*? Even more so.

Many of the hardest parts of writing a PBT library can in principle be reused. Bug-finding distributions, automatic shrinking, and flexible generator primitives could all be written once and used in every language. This would lower the barrier to entry and make world-class property-based testing available to all languages in a way which simply wasn't feasible before.

<!-- consolidate the best of property-based testing in every language. -->

Hegel is the realization of this idea.

Hegel is several things. It is `libhegel`, a property-based testing engine written in Rust and shipped as a native library. It is also a family of libraries, one per language, that run that engine in-process to get test data.

When we say "Hegel", we might mean any or all of these things. Your interest in Hegel will therefore determine which you might want to read further about:

- Want to *use* Hegel? [hegel-rust](https://github.com/hegeldev/hegel-rust), [hegel-go](https://github.com/hegeldev/hegel-go), [hegel-cpp](https://github.com/hegeldev/hegel-cpp), [hegel-typescript](https://github.com/hegeldev/hegel-typescript), [hegel-java](https://github.com/hegeldev/hegel-java), and [hegel-ocaml](https://github.com/hegeldev/hegel-ocaml) are fully-fledged property-based testing libraries, built on Hegel.
- Want to learn how it fits together? See [How Hegel works](/explanation/how-hegel-works), or jump straight to the [libhegel reference](/reference/libhegel) for the C ABI that every library binds to.
