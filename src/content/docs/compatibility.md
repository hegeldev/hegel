---
title: Compatibility
---

:::caution
TL;DR: Hegel is in beta. We aim for but do not guarantee stability. Check back in a few months for a stable release!
:::

Hegel is in beta. While we take minimizing breakage seriously and do not expect to make sweeping API changes, we do expect to make some amount of breaking changes as we refine the API.

In particular, if we think a breaking change will result in a better property-based testing API, we will make it. Of course, we will not do so without fully communicating the change and migration path.

If the possibility of breaking changes bothers you, please check back in a few months when we expect to have released a stable 1.0!

## Versioning

While Hegel is in beta, we have adopted the following version scheme for [hegel-core](https://github.com/hegeldev/hegel-core) and all Hegel libraries:

- Minor `0.N.0` releases for changes that might be breaking.
- Patch `0.0.N` releases for any other change.

This is effectively SemVer shifted right by one decimal place.

Our first stable release will be `1.0.0`.

## Platform support

Hegel fully supports both macOS and Linux. Hegel does not currently support Windows, due to the use of Unix sockets in [hegel-core](https://github.com/hegeldev/hegel-core). Support for windows is planned; you can follow [hegel-core#61](https://github.com/hegeldev/hegel-core/issues/61) for updates.
