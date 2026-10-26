use super::*;
use crate::runner::{Backend, Database, Settings, Verbosity};

fn s(strs: &[&str]) -> Vec<String> {
    std::iter::once("prog")
        .chain(strs.iter().copied())
        .map(String::from)
        .collect()
}

fn apply(args: &[&str]) -> Settings {
    try_apply_cli_args(Settings::new, s(args)).unwrap_or_else(|e| match e {
        CliError::Help(_) => panic!("unexpected help"),
        CliError::Parse(msg) => panic!("parse error: {msg}"),
    })
}

#[test]
fn test_no_args_returns_default() {
    let defaults = Settings::new();
    let parsed = apply(&[]);
    assert_eq!(parsed.test_cases, defaults.test_cases);
    assert_eq!(parsed.verbosity, defaults.verbosity);
    assert_eq!(parsed.seed, defaults.seed);
}

#[test]
fn test_seed_override() {
    let parsed = apply(&["--seed", "42"]);
    assert_eq!(parsed.seed, Some(42));
}

#[test]
fn test_seed_none() {
    let parsed = apply(&["--seed", "none"]);
    assert_eq!(parsed.seed, None);
}

#[test]
fn test_verbosity_override() {
    let parsed = apply(&["--verbosity", "quiet"]);
    assert_eq!(parsed.verbosity, Verbosity::Quiet);

    let parsed = apply(&["--verbosity", "verbose"]);
    assert_eq!(parsed.verbosity, Verbosity::Verbose);

    let parsed = apply(&["--verbosity", "debug"]);
    assert_eq!(parsed.verbosity, Verbosity::Debug);

    let parsed = apply(&["--verbosity", "normal"]);
    assert_eq!(parsed.verbosity, Verbosity::Normal);
}

#[test]
fn test_derandomize_override() {
    let parsed = apply(&["--derandomize", "true"]);
    assert!(parsed.derandomize);
    let parsed = apply(&["--derandomize", "false"]);
    assert!(!parsed.derandomize);
}

#[test]
fn test_database_path() {
    let parsed = apply(&["--database", "/tmp/example"]);
    assert_eq!(parsed.database, Database::Path("/tmp/example".to_string()));
}

#[test]
fn test_database_disabled() {
    let parsed = apply(&["--database", "disabled"]);
    assert_eq!(parsed.database, Database::Disabled);
}

#[test]
fn test_suppress_health_check_single() {
    let parsed = apply(&["--suppress-health-check", "too_slow"]);
    assert_eq!(
        parsed.suppress_health_check,
        vec![crate::runner::HealthCheck::TooSlow]
    );
}

#[test]
fn test_suppress_health_check_multiple() {
    let parsed = apply(&["--suppress-health-check", "too_slow,filter_too_much"]);
    assert_eq!(
        parsed.suppress_health_check,
        vec![
            crate::runner::HealthCheck::FilterTooMuch,
            crate::runner::HealthCheck::TooSlow
        ]
    );
}

#[test]
fn test_suppress_health_check_all() {
    let parsed = apply(&["--suppress-health-check", "all"]);
    assert_eq!(parsed.suppress_health_check.len(), 4);
}

#[test]
fn test_multiple_flags() {
    let parsed = apply(&[
        "--derandomize",
        "true",
        "--seed",
        "9",
        "--verbosity",
        "quiet",
    ]);
    assert!(parsed.derandomize);
    assert_eq!(parsed.seed, Some(9));
    assert_eq!(parsed.verbosity, Verbosity::Quiet);
}

#[test]
fn test_unknown_arg_error() {
    for args in [&["--nope", "1"][..], &["--nope"][..], &["--nope=1"][..]] {
        let err = try_apply_cli_args(Settings::new, s(args)).unwrap_err();
        match err {
            CliError::Parse(msg) => assert_eq!(msg, "Unknown argument: --nope"),
            _ => panic!("wrong error kind"),
        }
    }
}

#[test]
fn test_missing_value_for_a_flag_that_takes_an_empty_value() {
    let err = try_apply_cli_args(Settings::new, s(&["--phases"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert_eq!(msg, "--phases requires a value"),
        _ => panic!("wrong error kind"),
    }
}

#[test]
fn test_missing_value_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--seed"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert!(msg.contains("requires a value")),
        _ => panic!("wrong error kind"),
    }
}

#[test]
fn test_help_returns_help_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--help"])).unwrap_err();
    match err {
        CliError::Help(msg) => assert!(msg.contains("Usage:")),
        _ => panic!("wrong error kind"),
    }
}

#[test]
fn test_short_help_returns_help_error() {
    let err = try_apply_cli_args(Settings::new, s(&["-h"])).unwrap_err();
    matches!(err, CliError::Help(_));
}

#[test]
fn test_default_preserved_when_not_overridden() {
    let parsed = try_apply_cli_args(|| Settings::new().test_cases(42), s(&[])).unwrap();
    assert_eq!(parsed.test_cases, 42);
}

#[test]
fn test_explicit_override_wins_over_default() {
    let parsed =
        try_apply_cli_args(|| Settings::new().seed(Some(42)), s(&["--seed", "10"])).unwrap();
    assert_eq!(parsed.seed, Some(10));
}

#[test]
fn test_invalid_verbosity_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--verbosity", "loud"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert!(msg.contains("quiet|normal|verbose|debug")),
        _ => panic!("wrong error kind"),
    }
}

#[test]
fn test_invalid_bool_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--derandomize", "maybe"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert!(msg.contains("true|false")),
        _ => panic!("wrong error kind"),
    }
}

#[test]
fn test_invalid_health_check_error() {
    let err =
        try_apply_cli_args(Settings::new, s(&["--suppress-health-check", "bad_name"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert!(msg.contains("does not recognise")),
        _ => panic!("wrong error kind"),
    }
}

#[test]
fn test_bool_aliases() {
    let parsed = apply(&["--derandomize", "1"]);
    assert!(parsed.derandomize);
    let parsed = apply(&["--derandomize", "yes"]);
    assert!(parsed.derandomize);
    let parsed = apply(&["--derandomize", "0"]);
    assert!(!parsed.derandomize);
    let parsed = apply(&["--derandomize", "no"]);
    assert!(!parsed.derandomize);
}

#[test]
fn test_invalid_seed_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--seed", "abc"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert_eq!(
            msg, "--seed expects an integer or \"none\", got \"abc\"",
        ),
        _ => panic!("wrong error kind"),
    }
}

#[test]
fn test_apply_cli_args_success() {
    match apply_cli_args(Settings::new, s(&["--seed", "13"])) {
        CliOutcome::Success(settings) => assert_eq!(settings.seed, Some(13)),
        other => panic!("expected Success, got {other:?}"),
    }
}

#[test]
fn test_apply_cli_args_help() {
    match apply_cli_args(Settings::new, s(&["--help"])) {
        CliOutcome::Help(msg) => assert!(msg.contains("Usage:")),
        other => panic!("expected Help, got {other:?}"),
    }
}

#[test]
fn test_removed_flag_is_unknown_argument() {
    let err = try_apply_cli_args(Settings::new, s(&["--single-test-case"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert_eq!(msg, "Unknown argument: --single-test-case"),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn test_test_cases_flag_is_rejected() {
    let err = try_apply_cli_args(Settings::new, s(&["--test-cases", "3"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert!(msg.contains("exactly one test case"), "{msg}"),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn test_underscore_flag_is_unknown_argument() {
    for (args, flag) in [
        (&["--test_cases", "3"][..], "--test_cases"),
        (&["--test_cases=3"][..], "--test_cases"),
        (&["--seed_", "3"][..], "--seed_"),
        (&["--database_key", "k"][..], "--database_key"),
    ] {
        let err = try_apply_cli_args(Settings::new, s(args)).unwrap_err();
        match err {
            CliError::Parse(msg) => assert_eq!(msg, format!("Unknown argument: {flag}")),
            other => panic!("expected parse error, got {other:?}"),
        }
    }
}

#[test]
fn test_positional_argument_is_unknown() {
    for arg in ["positional", "--"] {
        let err = try_apply_cli_args(Settings::new, s(&[arg])).unwrap_err();
        match err {
            CliError::Parse(msg) => assert_eq!(msg, format!("Unknown argument: {arg}")),
            other => panic!("expected parse error, got {other:?}"),
        }
    }
}

#[test]
fn test_equals_form() {
    let parsed = apply(&["--seed=7", "--verbosity=debug"]);
    assert_eq!(parsed.seed, Some(7));
    assert_eq!(parsed.verbosity, Verbosity::Debug);
}

#[test]
fn test_every_engine_setting_is_a_flag() {
    let parsed = apply(&[
        "--phases",
        "generate,shrink",
        "--report-multiple-failures",
        "true",
        "--stop-on-first-failure",
        "false",
        "--show-statistics",
        "yes",
        "--print-blob",
        "no",
        "--nondeterminism-strictness",
        "error",
        "--unbounded-choices",
        "1",
        "--database-key",
        "my-key",
    ]);
    assert_eq!(
        parsed.phases,
        vec![crate::runner::Phase::Generate, crate::runner::Phase::Shrink]
    );
    assert!(parsed.report_multiple_failures);
    assert!(!parsed.stop_on_first_failure);
    assert!(parsed.show_statistics);
    assert!(!parsed.print_blob);
    assert_eq!(
        parsed.nondeterminism_strictness,
        crate::runner::NondeterminismStrictness::Error
    );
    assert!(parsed.unbounded_choices);
    assert_eq!(parsed.database_key.as_deref(), Some("my-key"));
}

#[test]
fn test_compiled_in_database_key_is_kept() {
    let parsed = try_apply_cli_args(
        || Settings::new().database_key("compiled"),
        s(&["--seed", "1"]),
    )
    .unwrap();
    assert_eq!(parsed.database_key.as_deref(), Some("compiled"));
}

#[test]
fn test_empty_phases() {
    let parsed = apply(&["--phases="]);
    assert!(parsed.phases.is_empty());
}

#[test]
fn test_backend_urandom_flag() {
    let parsed = apply(&["--backend", "urandom"]);
    assert_eq!(parsed.backend, Backend::Urandom);
}

#[test]
fn test_backend_default_flag() {
    let parsed = apply(&["--backend", "default"]);
    assert_eq!(parsed.backend, Backend::Default);
}

#[test]
fn test_backend_defaults_to_the_prng() {
    let parsed = apply(&[]);
    assert_eq!(parsed.backend, Backend::Default);
}

#[test]
fn test_invalid_backend_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--backend", "wat"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert!(msg.contains("default|urandom")),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn test_backend_missing_value_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--backend"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert!(msg.contains("requires a value")),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn test_apply_cli_args_parse_error() {
    match apply_cli_args(Settings::new, s(&["--not-a-flag"])) {
        CliOutcome::ParseError(msg) => {
            assert!(msg.contains("Unknown argument"));
            assert!(msg.contains("Usage:"));
        }
        other => panic!("expected ParseError, got {other:?}"),
    }
}

#[test]
fn test_database_default() {
    let parsed = try_apply_cli_args(
        || Settings::new().database(None),
        s(&["--database", "default"]),
    )
    .unwrap();
    assert_eq!(parsed.database, Database::Default);
}

/// `--profile` values that parse set the default profile for the whole
/// process, so only the errors caught before that happens are testable
/// in-process; the flag's behavior is covered through fixture binaries in
/// `tests/test_profiles.rs`.
#[test]
fn test_profile_flag_missing_value_error() {
    let err = try_apply_cli_args(Settings::new, s(&["--profile"])).unwrap_err();
    match err {
        CliError::Parse(msg) => assert_eq!(msg, "--profile requires a value"),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn test_profile_flag_repeated_error() {
    let err = try_apply_cli_args(
        Settings::new,
        s(&["--profile", "ci", "--profile", "development"]),
    )
    .unwrap_err();
    match err {
        CliError::Parse(msg) => assert_eq!(msg, "--profile may be given at most once"),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn test_extract_profile_takes_both_forms() {
    let flags = |strs: &[&str]| s(strs).split_off(1);
    let mut args = flags(&["--seed", "1", "--profile=ci", "--derandomize", "true"]);
    assert_eq!(extract_profile(&mut args).unwrap(), Some("ci".to_string()));
    assert_eq!(args, flags(&["--seed", "1", "--derandomize", "true"]));

    let mut args = flags(&["--profile", "ci", "--seed", "1"]);
    assert_eq!(extract_profile(&mut args).unwrap(), Some("ci".to_string()));
    assert_eq!(args, flags(&["--seed", "1"]));

    let mut args = flags(&["--seed", "1"]);
    assert_eq!(extract_profile(&mut args).unwrap(), None);
    assert_eq!(args, flags(&["--seed", "1"]));
}

#[test]
fn test_profile_flag_repeated_in_mixed_forms_error() {
    let err = try_apply_cli_args(
        Settings::new,
        s(&["--profile=ci", "--profile", "development"]),
    )
    .unwrap_err();
    match err {
        CliError::Parse(msg) => assert_eq!(msg, "--profile may be given at most once"),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn test_usage_documents_the_profile_flag() {
    match apply_cli_args(Settings::new, s(&["--help"])) {
        CliOutcome::Help(msg) => assert!(msg.contains("--profile <NAME>")),
        other => panic!("expected Help, got {other:?}"),
    }
}


fn parse_error(args: &[&str]) -> String {
    match try_apply_cli_args(Settings::new, s(args)) {
        Err(CliError::Parse(msg)) => msg,
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn test_a_flag_is_not_taken_as_the_previous_flags_value() {
    assert_eq!(
        parse_error(&["--seed", "--verbosity", "quiet"]),
        "--seed requires a value"
    );
}

#[test]
fn test_empty_database_key_is_rejected() {
    assert_eq!(
        parse_error(&["--database-key="]),
        "--database-key expects a non-empty key"
    );
    assert_eq!(
        parse_error(&["--database-key", ""]),
        "--database-key expects a non-empty key"
    );
}

#[test]
fn test_value_errors_name_the_setting_by_its_flag() {
    assert_eq!(
        parse_error(&["--nondeterminism-strictness", "x"]),
        "--nondeterminism-strictness expects one of quiet|warn|error, got \"x\""
    );
    assert_eq!(
        parse_error(&["--suppress-health-check=nope"]),
        "--suppress-health-check does not recognise \"nope\". Known names: all, \
         filter_too_much, too_slow, test_cases_too_large, large_initial_test_case"
    );
}
