RELEASE_TYPE: minor

This release makes the engine's settings resolution the single source of truth for every setting, exposes the settings and profiles that had no Java API, types the exceptions for runs the engine aborts, and fixes three native memory leaks.

Settings the test does not give are no longer sent to the engine. Before, every run sent `verbosity` (normal) and `reportMultipleFailures` (off), overriding whatever a `hegel.toml` profile or the environment said; now they come from the engine's profile like everything else. To support this:

- `Verbosity` gains a `DEFAULT` constant, which is the default for `Settings` and `@HegelTest(verbosity = ...)`.
- `@HegelTest(reportMultipleFailures = ...)` is now an `OptBoolean` rather than a `boolean`. Replace `reportMultipleFailures = true` with `reportMultipleFailures = OptBoolean.TRUE`.

The default example-database key of a `@HegelTest` is now its fully-qualified class name and method name (`com.example.CartTest.totalsAddUp`) rather than the bare method name, so same-named tests in different classes no longer share stored failures. Failures stored under the old key are not replayed; a `@HegelTest(name = ...)` keeps its explicit key.

New settings, each also an `@HegelTest` attribute:

- `stopOnFirstFailure(boolean)`: turned off, a run spends its whole test-case budget before reporting.
- `showStatistics(boolean)`: print the engine's statistics block at the end of the run.
- `unboundedChoices(boolean)`: lift the engine's per-test-case limit on choices, for very long test cases such as long-running concurrent state machines.
- `testLocation(TestLocation)`: where the test lives, which the engine reports each run's verdict under inside Antithesis. `@HegelTest` locates a test at its method, and `Hegel.test` and `Hegel.run` at their caller, so this is only needed to override that.

Settings profiles are now reachable from Java:

```java
Settings.registerProfile("nightly", Settings.fromProfile("ci").testCases(10_000));
Settings.setDefaultProfile("nightly");

@HegelTest(profile = "ci")
void property(TestCase tc) { ... }
```

`Settings.fromProfile(name)` starts from the named profile instead of the default one, `Settings.registerProfile` registers a profile process-wide, and `Settings.setDefaultProfile` changes which profile `new Settings()` starts from.

This release also fixes three native memory leaks. Every list a test drew left its engine-side collection allocated, every `Pool` and `ConcurrentPool` stayed allocated after its test case ended, and in a concurrent stateful run each worker thread left its engine error context behind when it exited. Long runs, and long concurrent state machines in particular, now hold steady memory.


An errored run now raises an exception typed by the engine's classification of the error rather than by its message. `HegelRunException` (a `HegelException`) is the new base type for a run the engine aborted, with subtypes `HealthCheckFailure` (which now says which check failed through `healthCheck()`), `FlakyTestException` (nondeterminism under `NondeterminismStrictness.ERROR`) and `UnsatisfiableException` (no valid test case was ever generated). Code that caught `HegelException` still catches all of them. `RunReport` gains `errorKind()` and `failedHealthCheck()`, and `healthCheckFailed()` no longer depends on the message's wording.

`Label.of` and `Label.combine` are now computed by the engine, and the labels of the standard generators (`Label.LIST`, `Label.ONE_OF`, `Label.OPTIONAL`, `Label.FILTER` and so on) now use the same names as the other Hegel frontends, so their values change. Labels are only identities, so this changes nothing but the values of those constants.

`dev.hegel.lowlevel.Libhegel` gains a method for each newly bound C function: `collectionFree`, `poolFree`, `releaseThreadContext`, `settingsNewForProfile`, `settingsRegisterProfile`, `setDefaultProfile`, `settingsGetVerbosity`, `settingsStopOnFirstFailure`, `settingsShowStatistics`, `settingsUnboundedChoices`, `settingsTestLocation`, `runResultErrorKind`, `labelFromName` and `labelCombine`. A third-party binding needs to implement them.
