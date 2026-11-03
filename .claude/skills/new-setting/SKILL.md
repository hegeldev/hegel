---
name: new-setting
description: "How to add a setting to hegel-rust. Use when the user asks to add a setting, a Settings builder method, a hegel.toml profile key, a HEGEL_* environment variable, or a hegel_settings_set_* C-ABI function. Covers every surface a setting must reach before it counts as done."
---

# Adding a Setting

A setting is not done until it is reachable every way the others are. Pick an existing setting (e.g. `derandomize`) and follow it through each of these:

1. **Engine builder** — a `Settings` builder method in `libhegel/src/settings.rs`.
2. **Profile key** — a `hegel.toml` key in `libhegel/src/config.rs`, with the same name and vocabulary as the builder method.
3. **String setter** — a `parse_setting` arm in `libhegel/src/config.rs` and the key added to `SETTING_KEYS` there, so `hegel_settings_set_from_string` (which every frontend's command line uses) accepts it. Reuse the per-setting parser the profile key uses.
4. **C ABI** — `hegel_settings_set_<name>` / `hegel_settings_get_<name>` in `libhegel/src/lib.rs`, then `just libhegel::header` to regenerate `libhegel/include/hegel.h` and `frontends/rust/src/ffi/sys/fns.rs`.
5. **Frontend** — `frontends/rust/src/ffi.rs` round-trips the value through the new C functions, and the `Settings` builder in `frontends/rust/src/runner.rs` exposes it.
6. **Environment variable** — `HEGEL_<SETTING>` applied in `Settings::with_env_overrides_from` (`libhegel/src/settings.rs`), using the same vocabulary as the profile key. An empty value is ignored; a malformed value is rejected with a message naming the variable. Never add a setting without its variable: libtest owns the command line, so the variable is the only way to change a setting for one run of a `#[hegel::test]` without editing it.
7. **Docs** — `frontends/rust/src/docs/settings.md` (the settings table and both environment-variable tables) and the `hegel_settings_new` doc comment in `libhegel/src/lib.rs`.
8. **Tests** — in `frontends/rust/tests/` (frontend) and `libhegel/tests/` (engine), never inline: builder, profile parsing, `hegel_settings_set_from_string` (accepted and rejected values), C-ABI get/set round trip, env-var override (empty ignored, malformed rejected). The `coverage` skill applies.
9. **Release notes** — a `RELEASE/<slug>.md` file per the `changelog` skill.
