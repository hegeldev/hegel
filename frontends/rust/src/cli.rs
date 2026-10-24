//! Command line argument parsing for standalone Hegel binaries produced by
//! `#[hegel::main]`.
//!
//! The parser starts from a caller-provided [`Settings`] constructor (so
//! that `#[hegel::main(seed = 42)]` produces a binary whose `--seed` flag
//! defaults to 42) and applies CLI overrides on top of its value. The
//! constructor runs after `--profile` has set the process's default
//! profile, so compiled-in settings that derive from [`Settings::new`]
//! build on the profile the flag names, exactly as they would under
//! `HEGEL_DEFAULT_PROFILE`.
//!
//! Every other flag is a setting: `--some-setting VALUE` (or
//! `--some-setting=VALUE`) sets the `hegel.toml` key `some_setting`, with
//! the engine parsing the value through `hegel_settings_set_from_string`,
//! so the flags reach every setting the engine has, in the same vocabulary
//! as `hegel.toml` and the `HEGEL_*` variables.

use crate::runner::Settings;
/// Result of applying CLI overrides. The macro wrapper in `#[hegel::main]`
/// dispatches on this to print messages and exit the process; keeping the
/// I/O and process-exit out of this function makes it directly testable.
#[derive(Debug)]
pub enum CliOutcome {
    /// Settings parsed successfully.
    Success(Settings),
    /// `--help` / `-h` was passed. Caller should print the message to stdout
    /// and exit with code 0.
    Help(String),
    /// Unknown argument or malformed value. Caller should print the message
    /// to stderr and exit with a nonzero code.
    ParseError(String),
}

/// Apply CLI overrides to the settings `default_settings` constructs.
///
/// `args` should include the program name at index 0 (i.e., pass
/// `std::env::args()` directly).
///
/// This is called from the entry point produced by `#[hegel::main]`; it is
/// exported here so that other main wrappers can construct Settings from
/// the same CLI surface.
pub fn apply_cli_args<I>(default_settings: impl FnOnce() -> Settings, args: I) -> CliOutcome
where
    I: IntoIterator<Item = String>,
{
    match try_apply_cli_args(default_settings, args) {
        Ok(s) => CliOutcome::Success(s),
        Err(CliError::Help(msg)) => CliOutcome::Help(msg),
        Err(CliError::Parse(msg)) => CliOutcome::ParseError(format!("{}\n\n{}", msg, usage())),
    }
}

#[derive(Debug)]
enum CliError {
    Help(String),
    Parse(String),
}

fn try_apply_cli_args<I>(
    default_settings: impl FnOnce() -> Settings,
    args: I,
) -> Result<Settings, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let _program = iter.next();
    let mut args: Vec<String> = iter.collect();

    if let Some(name) = extract_profile(&mut args)? {
        set_default_profile(&name)?;
    }
    let settings = default_settings();

    let mut overrides = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--help" || arg == "-h" {
            return Err(CliError::Help(usage()));
        }
        let Some(flag) = arg.strip_prefix("--").filter(|f| !f.is_empty()) else {
            return Err(CliError::Parse(format!("Unknown argument: {arg}")));
        };
        let (name, value) = match flag.split_once('=') {
            Some((name, value)) => (name, value.to_string()),
            None => (flag, next_value(&args, &mut i, arg)?),
        };
        if name == "test-cases" {
            return Err(CliError::Parse(
                "--test-cases cannot be set: a #[hegel::main] binary runs exactly one test case"
                    .to_string(),
            ));
        }
        overrides.push((name.replace('-', "_"), value));
        i += 1;
    }
    crate::ffi::settings_with_strings(&settings, &overrides).map_err(|(index, message)| {
        let message = message
            .strip_prefix("hegel_settings_set_from_string: ")
            .unwrap_or(&message);
        let flag = format!("--{}", overrides[index].0.replace('_', "-"));
        if message.starts_with("unknown setting") {
            CliError::Parse(format!("Unknown argument: {flag}"))
        } else {
            CliError::Parse(format!("{flag}: {message}"))
        }
    })
}

fn next_value(args: &[String], i: &mut usize, name: &str) -> Result<String, CliError> {
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| CliError::Parse(format!("{name} requires a value")))
}

/// Extract `--profile <NAME>` from `args` before the flag loop runs, so
/// the flag takes effect before the compiled-in settings are constructed
/// whatever its position.
fn extract_profile(args: &mut Vec<String>) -> Result<Option<String>, CliError> {
    let Some(pos) = args.iter().position(|a| a == "--profile") else {
        return Ok(None);
    };
    args.remove(pos);
    if pos >= args.len() {
        return Err(CliError::Parse("--profile requires a value".to_string()));
    }
    let name = args.remove(pos);
    if args.iter().any(|a| a == "--profile") {
        return Err(CliError::Parse(
            "--profile may be given at most once".to_string(),
        ));
    }
    Ok(Some(name))
}

/// Set `name` as the process's default profile and check it resolves,
/// undoing the setting when it does not so a bad flag value cannot linger.
fn set_default_profile(name: &str) -> Result<(), CliError> {
    crate::ffi::set_default_profile(Some(name)).map_err(CliError::Parse)?;
    if let Err(message) = crate::ffi::settings_from_profile(None) {
        crate::ffi::set_default_profile(None).unwrap();
        return Err(CliError::Parse(message));
    }
    Ok(())
}

fn usage() -> String {
    "Usage: <program> [options]

Hegel property-based testing binary. Runs exactly one test case.

Options:
  --profile <NAME>                       Set the default settings profile, like
                                         the HEGEL_DEFAULT_PROFILE environment
                                         variable; other flags apply on top
  --seed <N|none>                        Seed for randomisation ('none' for unset)
  --derandomize <BOOL>                   Use a deterministic derived seed
  --database <PATH|disabled|default>     Database path for failing-example storage
  --database-key <KEY>                   Key the failing examples are stored under
  --verbosity <LEVEL>                    quiet | normal | verbose | debug
  --phases <NAMES>                       Comma-separated phases to run: explicit,
                                         reuse, generate, target, shrink
  --suppress-health-check <NAMES>        Comma-separated health check names
                                         (filter_too_much, too_slow,
                                         test_cases_too_large,
                                         large_initial_test_case), or 'all'
  --report-multiple-failures <BOOL>      Report every distinct failure found
  --stop-on-first-failure <BOOL>         End generation once a failure is found
  --show-statistics <BOOL>               Print the run's statistics
  --print-blob <BOOL>                    Print the failure's reproduce blob
  --backend <default|urandom>            Randomness source (urandom: the Antithesis
                                         fuzzer, else /dev/urandom)
  --nondeterminism-strictness <LEVEL>    quiet | warn | error
  --unbounded-choices <BOOL>             Remove the limit on choices per test case
  -h, --help                             Show this help and exit

Every flag also takes the form --flag=VALUE. A BOOL is true, false, 1, 0,
yes or no. Values use the vocabulary of the matching hegel.toml key.
"
    .to_string()
}

#[cfg(test)]
#[path = "../tests/embedded/cli_tests.rs"]
mod tests;
