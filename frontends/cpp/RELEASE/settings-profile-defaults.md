RELEASE_TYPE: minor

This release fixes `hegel::Settings` overriding the engine's settings profile. A field you do not set now takes its value from the profile: `HEGEL_DEFAULT_PROFILE`, a `hegel.toml` profile, `HEGEL_MAIN`'s `--profile`, or a settings environment variable such as `HEGEL_SEED`, `HEGEL_PHASES`, `HEGEL_VERBOSITY` or `HEGEL_PRINT_BLOB`.

`verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness` and `print_blob` are now `std::optional`. `phases` is now the new `hegel::Phases` type, which is unset by default. 