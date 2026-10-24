RELEASE_TYPE: minor

This release makes the engine the source of every setting a TypeScript test does not set itself, adds the settings that were missing, and changes what a failing or aborted run throws.

**Settings.** A run now starts from the settings the engine resolves (the default profile, `hegel.toml` and the `HEGEL_*` environment variables) and sets only the fields you give. Previously every run overwrote the test count, verbosity, derandomization and multiple-failure reporting with the frontend's own defaults, so `HEGEL_TEST_CASES` or a `hegel.toml` profile had no effect on them, and the frontend detected CI itself. CI and Antithesis detection is now the engine's: on a CI server the shipped `ci` profile applies (derandomized, database off, `too_slow` suppressed).

- Every field of `Settings` is optional.
- New settings: `profile`, `phases` (with the new `Phase` enum), `printBlob`, `stopOnFirstFailure`, `nondeterminismStrictness` (`NondeterminismStrictness`), `backend` (`Backend`), `showStatistics` and `unboundedChoices`.
- `seed` accepts a `bigint`, so the whole 64-bit range, and `null` clears a seed set by a profile.
- New `hegel.registerProfile(name, settings)` and `hegel.setDefaultProfile(name)`.
- `Database.unset` is replaced by `Database.default`, which resets the database to the engine's default location over whatever the profile says. Leave `database` out to keep the profile's choice.
- `suppressHealthCheck: []` now turns every health check back on, where before it changed nothing.

**Test identity.** A test's database key is now its file plus its name (its function's name, or its line for an anonymous function), derived from where `hegel.test` was called, instead of the source text of the test function, which changed with every edit to the test body. Saved failures from earlier versions are not replayed. The same location is passed to the engine, which reports the test's verdict to Antithesis when running inside it; the frontend no longer writes `sdk.jsonl` itself. The `Hegel` builder and the `TestLocation` type are now exported, with `.databaseKey(key)` and `.testLocation(location)` to override either:

```ts
new hegel.Hegel((tc) => {
  tc.draw(gs.integers());
})
  .settings({ testCases: 500 })
  .databaseKey("integers")
  .runSync();
```

**Errors.** A failing test now throws its own error, with its class, `cause` and an assertion library's expected and actual values intact, instead of a new `Error` whose message was the original message followed by the failure's origin. With `reportMultipleFailures`, several distinct failures are thrown together as an `AggregateError` rather than only the last one. A failure the engine could only confirm as nondeterministic prints its caveat, and reports the error seen while searching if replaying it does not fail again. A run the engine aborts throws a `RunError`: `FailedHealthCheckError` (with the failed `healthCheck`), `FlakyError`, `UnsatisfiableError`, or a plain `RunError`.
