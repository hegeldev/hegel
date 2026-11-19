//! Compile-diagnostic UI tests: every case in `tests/ui/` is a program that
//! must FAIL to compile, with diagnostics matching its checked-in `.stderr`
//! golden file. These pin the compile-time error messages of the hegel
//! macros (and a couple of deliberate type-level properties).
//!
//! To (re)generate the goldens after intentionally changing a diagnostic:
//! `TRYBUILD=overwrite cargo test --test test_ui`.
//!
//! The `tests/ui-e0283/` case is checked by hand (see [`e0283_diagnostic`])
//! rather than through trybuild: its diagnostic enumerates 8 of the crate's
//! `PrintableGenerator` implementors, and both the entries shown and their
//! count vary with the enabled feature set and — in ways that resist
//! prediction from the version number alone — the exact toolchain. The
//! hand-rolled comparison normalizes that block away and pins everything
//! else exactly.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// rustc changed the E0283 ambiguity note from ``cannot satisfy `_: Debug` ``
/// to ``the type must implement `Debug` `` somewhere after the MSRV
/// toolchain, so the case whose golden contains that note keeps one golden
/// per wording (same source, same assertion). Probe the active toolchain's
/// actual wording with a dependency-free snippet rather than maintaining a
/// version table.
fn e0283_note_uses_must_implement_wording() -> bool {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.rs");
    std::fs::write(
        &probe,
        "fn foo<T: std::fmt::Debug>() -> T { unimplemented!() }\n\
         fn main() { let _ = foo(); }\n",
    )
    .unwrap();
    let output = Command::new(rustc_binary())
        .args(["--edition", "2021", "--crate-name", "probe"])
        .arg(&probe)
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "the E0283 probe unexpectedly compiled"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("the type must implement") {
        return true;
    }
    if stderr.contains("cannot satisfy") {
        return false;
    }
    // A third wording: fail loudly so a matching golden gets added instead
    // of the mismatch surfacing as an opaque diff.
    panic!("unrecognized E0283 note wording; add a golden for it:\n{stderr}");
}

/// rustc also changed how it renders the "the trait `X` is not implemented
/// for `Y`" help when `Y` is a local type: the MSRV toolchain prints it as an
/// inline `= help:` note where newer toolchains print a `help:` block with a
/// source pointer at `Y`'s definition (and spell out trait paths more
/// fully). Probed like [`e0283_note_uses_must_implement_wording`].
fn trait_help_has_source_pointer() -> bool {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.rs");
    std::fs::write(
        &probe,
        "#[diagnostic::on_unimplemented(message = \"probe\", label = \"probe\")]\n\
         trait Marker {}\n\
         struct Plain;\n\
         fn need<T: Marker>() {}\n\
         fn main() { need::<Plain>(); }\n",
    )
    .unwrap();
    let output = Command::new(rustc_binary())
        .args(["--edition", "2021", "--crate-name", "probe"])
        .arg(&probe)
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "the trait-help probe unexpectedly compiled"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("= help: the trait `Marker` is not implemented") {
        return false;
    }
    if stderr.contains("help: the trait `Marker` is not implemented") {
        return true;
    }
    panic!("unrecognized trait-help wording; add a golden for it:\n{stderr}");
}

fn rustc_binary() -> std::ffi::OsString {
    std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into())
}

/// The Cargo workspace root (the nearest ancestor holding `Cargo.lock`), or
/// this crate's directory when it is not in a workspace. Cargo records the
/// crate's source paths in the hegel rlib relative to the workspace root, so
/// rustc must run from there for a diagnostic pointing into `src/` to find
/// the source line it quotes.
fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .find(|dir| dir.join("Cargo.lock").exists())
        .map(Path::to_path_buf)
        .unwrap_or(manifest_dir)
}

/// This crate's directory relative to [`workspace_root`] (`frontends/rust`),
/// the prefix a workspace-relative source pointer carries beyond the
/// crate-relative form the goldens use; empty when the crate is the root.
fn manifest_dir_in_workspace() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .strip_prefix(workspace_root())
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .into_owned()
}

/// The features this test binary was compiled with, which the nested cargo
/// build in [`hegel_build`] requests to get the same hegel rlib.
const FEATURES: &[(&str, bool)] = &[
    ("rand", cfg!(feature = "rand")),
    ("chrono", cfg!(feature = "chrono")),
    ("chrono-tz", cfg!(feature = "chrono-tz")),
    ("jiff", cfg!(feature = "jiff")),
    ("time", cfg!(feature = "time")),
    ("serde_json", cfg!(feature = "serde_json")),
    (
        "serde_json_raw_value",
        cfg!(feature = "serde_json_raw_value"),
    ),
    ("static-engine", cfg!(feature = "static-engine")),
];

#[test]
fn features_list_matches_manifest() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let mut in_features = false;
    let mut declared = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_features = line == "[features]";
        } else if in_features && !line.starts_with('#') {
            if let Some((name, _)) = line.split_once('=') {
                if name.trim() != "default" {
                    declared.push(name.trim().to_owned());
                }
            }
        }
    }
    let listed: Vec<String> = FEATURES
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    assert_eq!(listed, declared);
}

/// The hegel rlib of the build this test binary belongs to, and the
/// directories holding the dependency crates rustc needs to load alongside
/// it. Rlibs from other builds (other feature sets, older sources) sit in the
/// same directories, so cargo is asked instead: a nested `cargo test
/// --no-run` of this target with the same features, profile, target and
/// target directory is a no-op that reports every artifact the build uses.
fn hegel_build() -> (PathBuf, Vec<PathBuf>) {
    let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).parent().unwrap();
    let exe = std::env::current_exe().unwrap();
    let exe_dir = exe.parent().unwrap();
    let profile_dir = if exe_dir.file_name().is_some_and(|n| n == "deps") {
        exe_dir.parent().unwrap()
    } else {
        exe_dir
            .ancestors()
            .nth(4)
            .filter(|_| exe_dir.file_name().is_some_and(|n| n == "out"))
            .unwrap_or_else(|| panic!("unrecognized cargo target layout at {}", exe.display()))
    };
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| env!("CARGO").into());
    let mut command = Command::new(cargo);
    command
        .args(["test", "--no-run", "-p", "hegeltest", "--test", "test_ui"])
        .args(["--message-format=json-render-diagnostics", "--target-dir"])
        .arg(target_dir);
    match profile_dir.file_name().unwrap().to_str().unwrap() {
        "debug" => {}
        "release" => {
            command.arg("--release");
        }
        profile => {
            command.args(["--profile", profile]);
        }
    }
    let triple_dir = profile_dir.parent().unwrap();
    if triple_dir != target_dir {
        command.arg("--target").arg(triple_dir.file_name().unwrap());
    }
    let features: Vec<&str> = FEATURES
        .iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(name, _)| *name)
        .collect();
    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }
    let output = command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .unwrap();
    assert!(output.status.success(), "{command:?} failed");
    let mut rlib = None;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let message: serde_json::Value = serde_json::from_str(line).unwrap();
        if message["reason"] != "compiler-artifact" {
            continue;
        }
        let is_hegel_lib = message["target"]["name"] == "hegel"
            && message["target"]["kind"]
                .as_array()
                .unwrap()
                .iter()
                .any(|kind| kind == "lib");
        for filename in message["filenames"].as_array().unwrap() {
            let path = PathBuf::from(filename.as_str().unwrap());
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !["rlib", "rmeta", "dylib", "so", "dll"].contains(&ext) {
                continue;
            }
            if is_hegel_lib && ext == "rlib" {
                rlib = Some(path.clone());
            }
            let dir = path.parent().unwrap().to_path_buf();
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    let rlib = rlib.unwrap_or_else(|| panic!("{command:?} reported no hegel rlib"));
    (rlib, dirs)
}

/// Normalize the raw rustc stderr for the E0283 case down to its stable
/// content:
///
/// - the `PrintableGenerator` implementors list keeps only its `= help:`
///   header — the entries shown and the "and N others" count vary by
///   feature set and toolchain;
/// - gutter line numbers become `LL` and gutter indentation collapses, so
///   the golden doesn't churn when `src/test_case.rs` (whose `draw` the
///   diagnostic quotes) shifts;
/// - `--> ` pointers into crate sources drop their line:column for the same
///   reason (the case file's own pointer, whose position we control, keeps
///   its position);
/// - rustc's trailing notes about the full type name written to a temp file
///   (a random path), the `--verbose` hint, the "aborting due to" line, and
///   the `--explain` hint carry no information about hegel and are dropped,
///   as are the "consider manually implementing" help for derive-introduced
///   bounds and its "to learn more" link, whose wording is still evolving
///   across toolchains;
/// - a type too long for a `required for` note is elided as `, ...>` by
///   stable but `, _>` by nightly; the nightly form is rewritten to
///   stable's, and the `unsatisfied requirement introduced here` label of
///   a conditional-impl help, which nightly elides whole to `_: Trait<_>`,
///   drops its requirement (the `required for` notes spell it out);
/// - a `required by a bound in` note keeps only the source line its `^^^`
///   label points at: newer toolchains also print the enclosing item's
///   header (`impl TestCase {`) above it, with a `...` line for the gap.
fn normalize_e0283_stderr(raw: &str) -> String {
    let mut out = Vec::new();
    let mut in_impl_list = false;
    let mut in_bound_note = false;
    let mut bound_source = None;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if in_bound_note {
            if trimmed == "..." {
                continue;
            }
            if trimmed.starts_with('|') && trimmed.contains('^') {
                out.extend(bound_source.take());
                out.push(format!(" {trimmed}"));
                in_bound_note = false;
                continue;
            }
        }
        if trimmed.starts_with("note: required by a bound in") {
            in_bound_note = true;
        }
        if in_impl_list {
            // List entries vary by toolchain: backticked "`X` implements
            // `Y`" lines, bare type names, and the "and N others" tail.
            // Everything until the next diagnostic marker is part of the
            // list.
            let starts_marker = trimmed.starts_with('=')
                || trimmed.starts_with('|')
                || trimmed.starts_with("--> ")
                || trimmed.starts_with("note")
                || trimmed.starts_with("help")
                || trimmed.starts_with("error")
                || trimmed.chars().next().is_some_and(|c| c.is_ascii_digit());
            if !starts_marker {
                continue;
            }
            in_impl_list = false;
        }
        if trimmed.starts_with("= help: the following types implement trait")
            || trimmed.starts_with("= help: the following other types implement trait")
        {
            in_impl_list = true;
            out.push(format!(" {trimmed}"));
            continue;
        }
        if trimmed.starts_with("= note: the full name for the type has been written")
            || trimmed.starts_with("= note: consider using `--verbose`")
            || trimmed.starts_with("= help: consider manually implementing")
            || trimmed.starts_with("= note: to learn more, visit")
            || trimmed.starts_with("error: aborting due to")
            || trimmed.starts_with("For more information about this error")
        {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("--> ") {
            // Some toolchains print crate-source pointers as absolute paths,
            // and Windows uses backslashes; reduce both to the same
            // manifest-relative forward-slash form.
            let rest = rest.replace('\\', "/");
            let manifest = env!("CARGO_MANIFEST_DIR").replace('\\', "/");
            let rest = rest
                .strip_prefix(&manifest)
                .map(|stripped| stripped.trim_start_matches('/'))
                .unwrap_or(&rest);
            let in_workspace = manifest_dir_in_workspace().replace('\\', "/");
            let rest = rest
                .strip_prefix(&in_workspace)
                .map(|stripped| stripped.trim_start_matches('/'))
                .unwrap_or(rest);
            let location = if rest.starts_with("tests/") {
                rest.to_string()
            } else {
                rest.rsplitn(3, ':').last().unwrap().to_string()
            };
            out.push(format!(" --> {location}"));
            continue;
        }
        if trimmed.starts_with("= note: required for ") {
            out.push(format!(" {}", trimmed.replace(", _>", ", ...>")));
            continue;
        }
        if let Some(index) = trimmed.find("unsatisfied requirement introduced here: `") {
            out.push(format!(
                " {}unsatisfied requirement introduced here",
                &trimmed[..index]
            ));
            continue;
        }
        if trimmed.starts_with('|') || trimmed.starts_with('=') {
            out.push(format!(" {trimmed}"));
            continue;
        }
        let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && trimmed[digits..].trim_start().starts_with('|') {
            let rest = trimmed[digits..].trim_start();
            if in_bound_note {
                bound_source = Some(format!("LL {rest}"));
            } else {
                out.push(format!("LL {rest}"));
            }
            continue;
        }
        out.push(line.to_string());
    }
    while out.last().is_some_and(|line| line.is_empty()) {
        out.pop();
    }
    out.join("\n") + "\n"
}

/// Compile `case` against the freshly built hegel rlib and return its
/// normalized stderr (see [`normalize_e0283_stderr`]). The case must fail to
/// compile. The diagnostic width is pinned to rustc's non-terminal default:
/// it decides which types in the `required for` notes are elided to `...`,
/// and would otherwise follow the width of whatever terminal ran the test.
fn compile_failing_case(case: &str) -> String {
    let (rlib, search_dirs) = hegel_build();
    let out_dir = tempfile::tempdir().unwrap();
    let mut command = Command::new(rustc_binary());
    command
        .args([
            "--edition",
            "2021",
            "--emit=metadata",
            "--color=never",
            "--diagnostic-width=140",
        ])
        .arg("--extern")
        .arg({
            let mut arg = std::ffi::OsString::from("hegel=");
            arg.push(&rlib);
            arg
        });
    let rmeta = rlib.with_extension("rmeta");
    if rmeta.exists() {
        command.arg("--extern").arg({
            let mut arg = std::ffi::OsString::from("hegel=");
            arg.push(&rmeta);
            arg
        });
    }
    for dir in &search_dirs {
        command.arg("-L").arg({
            let mut arg = std::ffi::OsString::from("dependency=");
            arg.push(dir);
            arg
        });
    }
    let output = command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join(case))
        .arg("-o")
        .arg(out_dir.path().join("case.rmeta"))
        .current_dir(workspace_root())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "{case} unexpectedly compiled against {}",
        rlib.display()
    );
    normalize_e0283_stderr(&String::from_utf8_lossy(&output.stderr))
}

/// Compare a hand-checked case's normalized diagnostic against its golden,
/// or rewrite the golden under `TRYBUILD=overwrite`. Goldens are compared
/// with `\n` endings — a Windows checkout under `core.autocrlf` hands them
/// back with `\r\n`.
fn check_against_golden(actual: &str, golden: &str) {
    if std::env::var_os("TRYBUILD").is_some_and(|v| v == "overwrite") {
        std::fs::write(golden, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(golden)
        .unwrap_or_else(|_| panic!("missing golden {golden}; regenerate with TRYBUILD=overwrite"))
        .replace("\r\n", "\n");
    assert_eq!(
        actual, expected,
        "normalized diagnostic does not match {golden}; \
         if the new output is intended, regenerate with TRYBUILD=overwrite"
    );
}

/// The `tests/ui-e0283/` case, checked by hand: its diagnostic enumerates
/// implementors and splits by the active toolchain's E0283 wording (see the
/// module docs). Regenerate with `TRYBUILD=overwrite`, once on a `cannot
/// satisfy` toolchain (MSRV or current stable) and once on a `must
/// implement` one (nightly).
#[test]
fn e0283_diagnostic() {
    let actual = compile_failing_case("tests/ui-e0283/default_cant_infer_through_draw.rs");
    let golden = if e0283_note_uses_must_implement_wording() {
        "tests/ui-e0283/expected-current.stderr"
    } else {
        "tests/ui-e0283/expected-msrv.stderr"
    };
    check_against_golden(&actual, golden);
}

/// A `one_of!` over non-printable components passed to `tc.draw`. Checked by
/// hand for the same reason as the E0283 case — the diagnostic enumerates
/// `Debug` implementors, which vary with the toolchain — and golden-split by
/// the trait-help wording: the MSRV toolchain explains the missing `Debug`
/// with inline notes, newer ones with a `help:` block pointing at the
/// conditional `PrintableGenerator` impl. Regenerate with
/// `TRYBUILD=overwrite`, once on the MSRV toolchain and once on a current
/// one.
#[test]
fn one_of_non_printable_draw_diagnostic() {
    let actual = compile_failing_case("tests/ui-printability/one_of_non_printable_draw.rs");
    let golden = if trait_help_has_source_pointer() {
        "tests/ui-printability/one_of_non_printable_draw-current.stderr"
    } else {
        "tests/ui-printability/one_of_non_printable_draw-msrv.stderr"
    };
    check_against_golden(&actual, golden);
}

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
