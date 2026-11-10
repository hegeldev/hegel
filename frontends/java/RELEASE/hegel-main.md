RELEASE_TYPE: minor

This release adds `Hegel.main`, for writing a test as a standalone program. A program runs one test case and takes its settings from the command line. `--help` lists the options.

```java
public static void main(String[] args) {
    Hegel.main(args, tc -> {
        int x = tc.draw(integers());
        assertEquals(x + 0, x);
    });
}
```

The process exits with status 0 if the test case passes, 1 if it fails or the run ends in an error, and 2 if the command line is invalid.

In `dev.hegel:hegel-lowlevel`, `Libhegel` gains `settingsFromString` (`hegel_settings_set_from_string`), `setDefaultProfile` (`hegel_set_default_profile`), `settingsGetVerbosity` (`hegel_settings_get_verbosity`) and `settingsGetSuppressHealthCheck` (`hegel_settings_get_suppress_health_check`).
