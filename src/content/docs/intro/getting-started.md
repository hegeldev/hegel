---
title: Getting started
---

Welcome to Hegel! This page will orient first-time Hegel users and direct you to next steps.

Hegel exists because of a problem: writing a good property-based testing library is hard. Writing a good property-based testing library in *every language*? Even more so.

Many of the hardest parts of writing a PBT library can in principle be reused. Bug-finding distributions, automatic shrinking, and flexible generator primitives could all be written once and used in every language. This would lower the barrier to entry and make world-class property-based testing available to all languages in a way which simply wasn't feasible before.

<!-- consolidate the best of property-based testing in every language. -->

Hegel is the realization of this idea.

Hegel is several things. It is a protocol for communicating between a PBT server (written once) and a PBT library (written for each language). It is also an implementation of this server and libraries in a number of languages.

When we say "Hegel", we might mean any or all of these things. Your interest in Hegel will therefore determine which you might want to read further about:

- Want to *use* Hegel? [hegel-rust](https://github.com/hegeldev/hegel-rust) is a fully-fledged property-based testing library, built on Hegel. Libraries for more languages coming soon.
- Want to learn more about the protocol? See [How Hegel works](/explanation/how-hegel-works), or jump straight to the [hegel-core](https://github.com/hegeldev/hegel-core) server.
