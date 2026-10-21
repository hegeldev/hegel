RELEASE_TYPE: patch

This patch moves Hegel for TypeScript into the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo, next to the engine it drives. The package is used exactly as before, but it is now built and tested against the in-tree libhegel, so this release also updates the bundled engine from 0.45.4 to 0.48.0. The engine changes a TypeScript user can see:

- Strings passed to the engine may now contain NUL characters: a database key, the reason given to a rejected draw, or a `gs.fromRegex()` pattern no longer throws for `"\0"`.
- `gs.fromRegex()` now fails when it is built, with an error naming the pattern, if no string can match it (for example `(?!)`), instead of rejecting every draw until the run fails a health check. Patterns may use `\z` as a spelling of `\Z`.
- Integer shrinking no longer stalls short of the minimum on some sparse sets of failing values, generation over small finite spaces no longer over-produces the simplest value, and a failure found while shrinking another is now shrunk before it is reported. Seeded runs may generate different values from this version on.
- Every distinct failure a run finds is persisted to its database, and a run that found several reports only the smallest unless `reportMultipleFailures` is on.
- A float draw whose bounds leave only an infinity while infinities are disallowed is rejected as an invalid argument instead of failing as an internal error.
