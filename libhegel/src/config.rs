//! `hegel.toml` discovery and parsing.
//!
//! The config file defines and modifies settings profiles
//! ([`crate::profiles`]). It is a TOML document with an optional top-level
//! `default = "<profile>"` entry naming the default profile, and
//! `[profiles.<name>]` tables whose entries are the settings keys. The
//! vocabulary is strict: an unknown key, a value of the wrong type, a
//! misplaced `default`, or any other top-level table is a hard error
//! carrying a line number, because silent misconfiguration in a file that
//! changes test behaviour is worse than strictness.
//!
//! The file is discovered by checking the current directory and then each
//! ancestor up to the filesystem root, first hit wins — the same shape as
//! cargo's config discovery, so a `hegel.toml` at either the package or the
//! workspace root is found from wherever the test process runs. Setting
//! `HEGEL_CONFIG` to a path bypasses discovery entirely, for environments
//! that relocate the test process outside the source tree.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::profiles::{BASE, DEFAULT, ProfileDelta, ProfileError, is_valid_name};
use crate::settings::{Backend, Database, HealthCheck, NondeterminismStrictness, Phase, Verbosity};

/// The config file's name, looked for in the current directory and every
/// ancestor.
pub(crate) const FILE_NAME: &str = "hegel.toml";

/// When set and non-empty, the path of the config file to load, replacing
/// discovery.
pub(crate) const CONFIG_VAR: &str = "HEGEL_CONFIG";

/// Parsed contents of a `hegel.toml`: the default-profile entry, the
/// profile deltas it defines in file order, and the path it was loaded from
/// (`None` for a config that was parsed rather than loaded, or the empty
/// default).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ConfigFile {
    pub(crate) default: Option<String>,
    pub(crate) profiles: Vec<(String, ProfileDelta)>,
    pub(crate) path: Option<String>,
}

/// A parse failure at a 1-based line of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParseError {
    pub(crate) line: usize,
    pub(crate) message: String,
}

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
}

/// The parsed document, for turning the byte offsets of the spans the TOML
/// parser records into line numbers.
struct Source<'t> {
    text: &'t str,
}

impl Source<'_> {
    fn line_at(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .matches('\n')
            .count()
            + 1
    }

    fn line_of<T>(&self, spanned: &Spanned<T>) -> usize {
        self.line_at(spanned.span().start)
    }

    fn err_at<T>(&self, spanned: &Spanned<T>, message: impl Into<String>) -> ParseError {
        err(self.line_of(spanned), message)
    }
}

/// Parse the text of a `hegel.toml`.
pub(crate) fn parse(text: &str) -> Result<ConfigFile, ParseError> {
    let src = Source { text };
    let table = DeTable::parse(text).map_err(|e| {
        err(
            src.line_at(e.span().map_or(0, |span| span.start)),
            e.message(),
        )
    })?;
    let mut out = ConfigFile::default();
    for (key, value) in in_file_order(table.get_ref()) {
        match key.get_ref().as_ref() {
            "default" => {
                let name = expect_string(&src, value, "default")?;
                if name == DEFAULT {
                    return Err(src.err_at(
                        value,
                        format!("`default` cannot name the {DEFAULT:?} alias it resolves"),
                    ));
                }
                if !is_valid_name(name) {
                    return Err(invalid_name_err(name, src.line_of(value)));
                }
                out.default = Some(name.to_string());
            }
            "profiles" => {
                let DeValue::Table(profiles) = value.get_ref() else {
                    return Err(type_error(
                        &src,
                        value,
                        "profiles",
                        "[profiles.<name>] tables",
                    ));
                };
                for (name, delta) in in_file_order(profiles) {
                    out.profiles.push(parse_profile(&src, name, delta)?);
                }
            }
            other => {
                return Err(src.err_at(
                    key,
                    format!(
                        "unknown top-level key `{other}`: only `default` and \
                         [profiles.<name>] tables are allowed"
                    ),
                ));
            }
        }
    }
    Ok(out)
}

/// A table's entries sorted by where they appear in the file, since the
/// parsed table orders them by key.
fn in_file_order<'a, 'i>(
    table: &'a DeTable<'i>,
) -> Vec<(&'a Spanned<DeString<'i>>, &'a Spanned<DeValue<'i>>)> {
    let mut entries: Vec<_> = table.iter().collect();
    entries.sort_by_key(|(key, _)| key.span().start);
    entries
}

/// Parse one `[profiles.<name>]` table into a named delta, validating the
/// name.
fn parse_profile(
    src: &Source<'_>,
    name: &Spanned<DeString<'_>>,
    table: &Spanned<DeValue<'_>>,
) -> Result<(String, ProfileDelta), ParseError> {
    let line = src.line_of(name);
    let name = name.get_ref().as_ref();
    if name == BASE {
        return Err(err(
            line,
            format!(
                "{BASE:?} is the reserved base profile and cannot be modified; \
                 customize [profiles.development] instead"
            ),
        ));
    }
    if name == DEFAULT {
        return Err(err(
            line,
            format!(
                "{DEFAULT:?} is an alias for the default profile and cannot be defined; \
                 choose it with `default = \"<profile>\"` instead"
            ),
        ));
    }
    if !is_valid_name(name) {
        return Err(invalid_name_err(name, line));
    }
    let DeValue::Table(entries) = table.get_ref() else {
        return Err(type_error(
            src,
            table,
            &format!("profiles.{name}"),
            "a table of settings",
        ));
    };
    let mut delta = ProfileDelta::default();
    for (key, value) in in_file_order(entries) {
        assign(src, &mut delta, key, value)?;
    }
    Ok((name.to_string(), delta))
}

fn invalid_name_err(name: &str, line_no: usize) -> ParseError {
    err(
        line_no,
        format!("invalid profile name {name:?}: use only ASCII letters, digits, '-' and '_'"),
    )
}

fn kind(value: &DeValue<'_>) -> &'static str {
    match value {
        DeValue::String(_) => "a string",
        DeValue::Integer(_) => "an integer",
        DeValue::Float(_) => "a float",
        DeValue::Boolean(_) => "a boolean",
        DeValue::Datetime(_) => "a datetime",
        DeValue::Array(_) => "an array",
        DeValue::Table(_) => "a table",
    }
}

fn type_error(
    src: &Source<'_>,
    value: &Spanned<DeValue<'_>>,
    key: &str,
    expected: &str,
) -> ParseError {
    src.err_at(
        value,
        format!("`{key}` expects {expected}, got {}", kind(value.get_ref())),
    )
}

fn expect_string<'a>(
    src: &Source<'_>,
    value: &'a Spanned<DeValue<'_>>,
    key: &str,
) -> Result<&'a str, ParseError> {
    match value.get_ref() {
        DeValue::String(s) => Ok(s.as_ref()),
        _ => Err(type_error(src, value, key, "a string")),
    }
}

fn expect_bool(
    src: &Source<'_>,
    value: &Spanned<DeValue<'_>>,
    key: &str,
) -> Result<bool, ParseError> {
    match value.get_ref() {
        DeValue::Boolean(b) => Ok(*b),
        _ => Err(type_error(src, value, key, "a boolean")),
    }
}

fn expect_array<'a>(
    src: &Source<'_>,
    value: &'a Spanned<DeValue<'_>>,
    key: &str,
) -> Result<Vec<&'a str>, ParseError> {
    let DeValue::Array(items) = value.get_ref() else {
        return Err(type_error(src, value, key, "an array of strings"));
    };
    items
        .iter()
        .map(|item| match item.get_ref() {
            DeValue::String(s) => Ok(s.as_ref()),
            other => Err(src.err_at(
                item,
                format!("elements of `{key}` must be strings, got {}", kind(other)),
            )),
        })
        .collect()
}

fn expect_int(
    src: &Source<'_>,
    value: &Spanned<DeValue<'_>>,
    key: &str,
    min: i128,
    max: i128,
) -> Result<i128, ParseError> {
    let DeValue::Integer(n) = value.get_ref() else {
        return Err(type_error(src, value, key, "an integer"));
    };
    match i128::from_str_radix(n.as_str(), n.radix()) {
        Ok(v) if (min..=max).contains(&v) => Ok(v),
        _ => Err(src.err_at(
            value,
            format!("`{key}` must be between {min} and {max}, got {n}"),
        )),
    }
}

/// Set the field `key` names on `delta`, validating the value's type and
/// vocabulary.
fn assign(
    src: &Source<'_>,
    delta: &mut ProfileDelta,
    key: &Spanned<DeString<'_>>,
    value: &Spanned<DeValue<'_>>,
) -> Result<(), ParseError> {
    let line_no = src.line_of(value);
    let key = key.get_ref().as_ref();
    match key {
        "default" => {
            return Err(err(
                line_no,
                "`default` is a top-level key, not a profile setting: \
                 move it above the first [profiles.<name>] header",
            ));
        }
        "extends" => {
            let name = expect_string(src, value, key)?;
            if !is_valid_name(name) {
                return Err(invalid_name_err(name, line_no));
            }
            delta.extends = Some(name.to_string());
        }
        "test_cases" => {
            delta.test_cases = Some(expect_int(src, value, key, 1, u64::MAX as i128)? as u64);
        }
        "seed" => {
            delta.seed = Some(match value.get_ref() {
                DeValue::String(s) if s == "none" => None,
                DeValue::String(s) => {
                    return Err(err(
                        line_no,
                        format!("`seed` expects an integer or \"none\", got {s:?}"),
                    ));
                }
                _ => Some(expect_int(src, value, key, 0, u64::MAX as i128)? as u64),
            });
        }
        "derandomize" => delta.derandomize = Some(expect_bool(src, value, key)?),
        "report_multiple_failures" => {
            delta.report_multiple_failures = Some(expect_bool(src, value, key)?);
        }
        "stop_on_first_failure" => {
            delta.stop_on_first_failure = Some(expect_bool(src, value, key)?);
        }
        "show_statistics" => delta.show_statistics = Some(expect_bool(src, value, key)?),
        "unbounded_choices" => delta.unbounded_choices = Some(expect_bool(src, value, key)?),
        "print_blob" => delta.print_blob = Some(expect_bool(src, value, key)?),
        "verbosity" => {
            let s = expect_string(src, value, key)?;
            delta.verbosity = Some(parse_verbosity(s).map_err(|m| err(line_no, m))?);
        }
        "backend" => {
            let s = expect_string(src, value, key)?;
            delta.backend = Some(parse_backend(s).map_err(|m| err(line_no, m))?);
        }
        "nondeterminism_strictness" => {
            let s = expect_string(src, value, key)?;
            delta.nondeterminism_strictness =
                Some(parse_strictness(s).map_err(|m| err(line_no, m))?);
        }
        "database" => {
            let s = expect_string(src, value, key)?;
            delta.database = Some(parse_database(s).map_err(|m| err(line_no, m))?);
        }
        "suppress_health_check" => {
            let items = expect_array(src, value, key)?;
            delta.suppress_health_check =
                Some(parse_health_checks(&items).map_err(|m| err(line_no, m))?);
        }
        "phases" => {
            let items = expect_array(src, value, key)?;
            delta.phases = Some(parse_phases(&items).map_err(|m| err(line_no, m))?);
        }
        other => return Err(err(line_no, format!("unknown key `{other}`"))),
    }
    Ok(())
}

/// The `verbosity` vocabulary: `quiet`, `normal`, `verbose` or `debug`.
fn parse_verbosity(s: &str) -> Result<Verbosity, String> {
    match s {
        "quiet" => Ok(Verbosity::Quiet),
        "normal" => Ok(Verbosity::Normal),
        "verbose" => Ok(Verbosity::Verbose),
        "debug" => Ok(Verbosity::Debug),
        other => Err(format!(
            "`verbosity` expects one of quiet|normal|verbose|debug, got {other:?}"
        )),
    }
}

/// The `backend` vocabulary: `default` or `urandom`.
fn parse_backend(s: &str) -> Result<Backend, String> {
    match s {
        "default" => Ok(Backend::Default),
        "urandom" => Ok(Backend::Urandom),
        other => Err(format!(
            "`backend` expects one of default|urandom, got {other:?}"
        )),
    }
}

/// The `nondeterminism_strictness` vocabulary: `quiet`, `warn` or `error`.
fn parse_strictness(s: &str) -> Result<NondeterminismStrictness, String> {
    NondeterminismStrictness::parse(s).ok_or_else(|| {
        format!("`nondeterminism_strictness` expects one of quiet|warn|error, got {s:?}")
    })
}

/// The `database` vocabulary: `disabled`, `default` (the engine's default
/// location), or any other non-empty string as the database's path.
fn parse_database(s: &str) -> Result<Database, String> {
    match s {
        "" => Err("`database` expects a path, \"disabled\", or \"default\", got \"\"".to_string()),
        "disabled" => Ok(Database::Disabled),
        "default" => Ok(Database::Unset),
        path => Ok(Database::Path(path.to_string())),
    }
}

/// The health-check name vocabulary, shared with the frontends'
/// command-line flags: the four snake_case check names, or `"all"` as the
/// only element.
fn parse_health_checks(items: &[&str]) -> Result<Vec<HealthCheck>, String> {
    if items.contains(&"all") {
        if items.len() != 1 {
            return Err("\"all\" must be the only element of `suppress_health_check`".to_string());
        }
        return Ok(alloc::vec![
            HealthCheck::FilterTooMuch,
            HealthCheck::TooSlow,
            HealthCheck::TestCasesTooLarge,
            HealthCheck::LargeInitialTestCase,
        ]);
    }
    items
        .iter()
        .map(|item| match *item {
            "filter_too_much" => Ok(HealthCheck::FilterTooMuch),
            "too_slow" => Ok(HealthCheck::TooSlow),
            "test_cases_too_large" => Ok(HealthCheck::TestCasesTooLarge),
            "large_initial_test_case" => Ok(HealthCheck::LargeInitialTestCase),
            other => Err(format!(
                "`suppress_health_check` does not recognise {other:?}. \
                 Known names: all, filter_too_much, too_slow, \
                 test_cases_too_large, large_initial_test_case"
            )),
        })
        .collect()
}

/// The phase name vocabulary.
fn parse_phases(items: &[&str]) -> Result<Vec<Phase>, String> {
    items
        .iter()
        .map(|item| match *item {
            "explicit" => Ok(Phase::Explicit),
            "reuse" => Ok(Phase::Reuse),
            "generate" => Ok(Phase::Generate),
            "target" => Ok(Phase::Target),
            "shrink" => Ok(Phase::Shrink),
            other => Err(format!(
                "`phases` does not recognise {other:?}. \
                 Known names: explicit, reuse, generate, target, shrink"
            )),
        })
        .collect()
}

/// The keys [`parse_setting`] accepts: every `hegel.toml` profile setting,
/// plus `unbounded_choices` and `database_key`.
const SETTING_KEYS: &str = "test_cases, seed, derandomize, database, verbosity, \
     phases, suppress_health_check, report_multiple_failures, stop_on_first_failure, \
     show_statistics, print_blob, backend, nondeterminism_strictness, unbounded_choices, \
     database_key";

/// One setting parsed by [`parse_setting`].
pub(crate) enum ParsedSetting {
    /// A setting a `hegel.toml` profile can hold.
    Profile(ProfileDelta),
    /// The database key, which scopes the examples a run stores and
    /// replays. It is per-test identity, so no profile holds it.
    DatabaseKey(String),
}

/// Parse one setting given as text, as a command line passes it, into a
/// delta that sets just that setting. `key` is the setting's `hegel.toml`
/// name, and `value` uses that key's vocabulary in `hegel.toml`, with these
/// spellings for the values that are not strings there: an integer in
/// decimal; a boolean as `true`, `1`, `yes`, `false`, `0` or `no`, as the
/// `HEGEL_*` variables take it; and a list as its elements separated by
/// commas, with the empty string for the empty list. Of the keys
/// `hegel.toml` has none for, `unbounded_choices` takes a boolean and
/// `database_key` takes the key itself.
pub(crate) fn parse_setting(key: &str, value: &str) -> Result<ParsedSetting, String> {
    let integer = |min: u64| match value.parse::<u64>() {
        Ok(n) if n >= min => Ok(n),
        _ => Err(format!(
            "`{key}` expects an integer between {min} and {}, got {value:?}",
            u64::MAX
        )),
    };
    let boolean = || {
        crate::settings::parse_bool(value)
            .ok_or_else(|| format!("`{key}` expects one of true|false|1|0|yes|no, got {value:?}"))
    };
    let list = || -> Vec<&str> {
        if value.is_empty() {
            Vec::new()
        } else {
            value.split(',').map(str::trim).collect()
        }
    };
    let mut delta = ProfileDelta::default();
    match key {
        "test_cases" => delta.test_cases = Some(integer(1)?),
        "seed" => {
            delta.seed =
                Some(if value == "none" {
                    None
                } else {
                    Some(value.parse::<u64>().map_err(|_| {
                        format!("`seed` expects an integer or \"none\", got {value:?}")
                    })?)
                });
        }
        "derandomize" => delta.derandomize = Some(boolean()?),
        "report_multiple_failures" => delta.report_multiple_failures = Some(boolean()?),
        "stop_on_first_failure" => delta.stop_on_first_failure = Some(boolean()?),
        "show_statistics" => delta.show_statistics = Some(boolean()?),
        "print_blob" => delta.print_blob = Some(boolean()?),
        "unbounded_choices" => delta.unbounded_choices = Some(boolean()?),
        "verbosity" => delta.verbosity = Some(parse_verbosity(value)?),
        "backend" => delta.backend = Some(parse_backend(value)?),
        "nondeterminism_strictness" => {
            delta.nondeterminism_strictness = Some(parse_strictness(value)?);
        }
        "database" => delta.database = Some(parse_database(value)?),
        "suppress_health_check" => {
            delta.suppress_health_check = Some(parse_health_checks(&list())?);
        }
        "phases" => delta.phases = Some(parse_phases(&list())?),
        "database_key" => return Ok(ParsedSetting::DatabaseKey(value.to_string())),
        other => {
            return Err(format!(
                "unknown setting `{other}`. Known settings: {SETTING_KEYS}"
            ));
        }
    }
    Ok(ParsedSetting::Profile(delta))
}

fn is_separator(byte: u8) -> bool {
    byte == b'/' || (cfg!(windows) && byte == b'\\')
}

/// The parent directory of `dir`, or `None` at a filesystem root or for a
/// single relative component. Windows drive roots keep their separator
/// (`C:\foo` → `C:\`), since a bare `C:` is drive-relative rather than the
/// root.
fn parent(dir: &str) -> Option<&str> {
    parent_on(dir, cfg!(windows))
}

fn parent_on(dir: &str, windows: bool) -> Option<&str> {
    let bytes = dir.as_bytes();
    let mut end = bytes.len();
    while end > 0 && is_separator(bytes[end - 1]) {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let last_sep = dir[..end].bytes().rposition(is_separator)?;
    if last_sep == 0 {
        return Some(&dir[..1]);
    }
    let candidate = &dir[..last_sep];
    if windows && candidate.ends_with(':') {
        return Some(&dir[..last_sep + 1]);
    }
    Some(candidate)
}

fn join(dir: &str, name: &str) -> String {
    let mut path = String::with_capacity(dir.len() + name.len() + 1);
    path.push_str(dir);
    if !dir.as_bytes().last().copied().is_some_and(is_separator) {
        path.push('/');
    }
    path.push_str(name);
    path
}

/// Locate `hegel.toml`: the current directory first, then each ancestor up
/// to the filesystem root, first hit wins. Returns the path and raw bytes.
/// With no current directory, only the relative `hegel.toml` is checked.
/// A file that exists but cannot be read is skipped, per the sys philosophy
/// of degrading silently on OS failure.
pub(crate) fn discover(
    cwd: Option<String>,
    exists: impl Fn(&str) -> bool,
    read: impl Fn(&str) -> Option<Vec<u8>>,
) -> Option<(String, Vec<u8>)> {
    let check = |path: String| {
        if exists(&path) {
            read(&path).map(|bytes| (path, bytes))
        } else {
            None
        }
    };
    let Some(mut dir) = cwd else {
        return check(FILE_NAME.to_string());
    };
    loop {
        if let Some(found) = check(join(&dir, FILE_NAME)) {
            return Some(found);
        }
        match parent(&dir) {
            Some(p) if p != dir => dir = p.to_string(),
            _ => return None,
        }
    }
}

/// Discover and parse `hegel.toml`. No file found is `Ok` with an empty
/// config. A set, non-empty `HEGEL_CONFIG` names the file directly instead
/// of discovering one, and a file it names that cannot be read is a hard
/// error: the variable exists to guarantee a config is loaded, so failing
/// to load it must be loud.
///
/// The result is loaded once and cached for the life of the process, so
/// every settings resolution sees the same config even if the file changes
/// mid-run.
///
/// Under Miri the result is always an empty config: Miri has no shim for
/// the `stat` call the discovery makes.
pub(crate) fn load() -> Result<ConfigFile, ProfileError> {
    #[cfg(miri)]
    return Ok(ConfigFile::default());
    #[cfg(not(miri))]
    {
        static LOADED: crate::sys::sync::Lazy<Result<ConfigFile, ProfileError>> =
            crate::sys::sync::Lazy::new(load_from_sys);
        LOADED.clone()
    }
}

/// [`load_from`] against the real environment, working directory, and
/// filesystem.
#[cfg(all(not(miri), not(target_family = "wasm")))]
fn load_from_sys() -> Result<ConfigFile, ProfileError> {
    load_from(
        crate::sys::env_var(CONFIG_VAR),
        crate::sys::cwd(),
        crate::sys::fs::exists,
        |path| crate::sys::fs::read(path).ok(),
    )
}

/// [`load_from`] on WebAssembly, which has no working directory or
/// filesystem: discovery finds nothing, and a `HEGEL_CONFIG` the host does
/// supply names a file that cannot be read.
#[cfg(all(not(miri), target_family = "wasm"))]
fn load_from_sys() -> Result<ConfigFile, ProfileError> {
    load_from(crate::sys::env_var(CONFIG_VAR), None, |_| false, |_| None)
}

/// [`load`] with the environment, directory, and filesystem reads injected.
pub(crate) fn load_from(
    config_var: Option<String>,
    cwd: Option<String>,
    exists: impl Fn(&str) -> bool,
    read: impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<ConfigFile, ProfileError> {
    if let Some(path) = config_var.filter(|p| !p.is_empty()) {
        let Some(bytes) = read(&path) else {
            return Err(ProfileError::Config {
                path,
                line: 0,
                message: format!("cannot read the file named by {CONFIG_VAR}"),
            });
        };
        return parse_bytes(path, bytes);
    }
    let Some((path, bytes)) = discover(cwd, exists, read) else {
        return Ok(ConfigFile::default());
    };
    parse_bytes(path, bytes)
}

fn parse_bytes(path: String, bytes: Vec<u8>) -> Result<ConfigFile, ProfileError> {
    let Ok(text) = String::from_utf8(bytes) else {
        return Err(ProfileError::Config {
            path,
            line: 0,
            message: "file is not valid UTF-8".to_string(),
        });
    };
    match parse(&text) {
        Ok(mut config) => {
            config.path = Some(path);
            Ok(config)
        }
        Err(e) => Err(ProfileError::Config {
            path,
            line: e.line,
            message: e.message,
        }),
    }
}

#[cfg(test)]
#[path = "../tests/embedded/config_tests.rs"]
mod tests;
