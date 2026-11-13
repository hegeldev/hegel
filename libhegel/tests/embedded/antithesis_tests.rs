use super::*;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use std::sync::{Arc, Mutex};

#[test]
fn existing_output_dir_is_accepted() {
    let dir = tempfile::TempDir::new().unwrap();
    check_antithesis_output_dir(dir.path().to_str().unwrap()).unwrap();
}

#[test]
fn missing_output_dir_is_a_usage_error() {
    let err =
        check_antithesis_output_dir("/no/such/antithesis/output/dir/for/hegel/tests").unwrap_err();
    assert!(matches!(err, crate::backend::RunError::UsageError(_)));
    let msg = err.to_string();
    assert!(msg.contains("ANTITHESIS_OUTPUT_DIR"), "got: {msg}");
}

/// A fake environment holding only `ANTITHESIS_OUTPUT_DIR`.
fn env_with_output_dir(dir: &str) -> impl Fn(&str) -> Option<String> {
    let dir = dir.to_string();
    move |key| (key == "ANTITHESIS_OUTPUT_DIR").then(|| dir.clone())
}

#[test]
fn not_in_antithesis_when_the_variable_is_unset() {
    check_environment_from(|_| None).unwrap();
    assert!(!antithesis_env_var_set_from(|_| None));
    assert!(antithesis_output_dir_from(|_| None).is_none());
}

#[test]
fn in_antithesis_when_the_variable_names_an_existing_directory() {
    let dir = tempfile::TempDir::new().unwrap();
    let env = env_with_output_dir(dir.path().to_str().unwrap());
    check_environment_from(&env).unwrap();
    assert_eq!(antithesis_env_var_set_from(&env), !cfg!(windows));
    assert_eq!(antithesis_output_dir_from(&env).is_some(), !cfg!(windows));
}

#[test]
fn other_variables_do_not_count() {
    let env = |key: &str| (key == "ANTITHESIS_OUTPUT").then(|| "/tmp".to_string());
    check_environment_from(env).unwrap();
    assert!(!antithesis_env_var_set_from(env));
}

#[cfg(not(windows))]
#[test]
fn a_missing_directory_is_a_usage_error_via_the_environment() {
    let env = env_with_output_dir("/no/such/antithesis/output/dir/for/hegel/tests");
    let err = check_environment_from(env).unwrap_err();
    assert!(matches!(err, crate::backend::RunError::UsageError(_)));
}

fn location() -> TestLocation {
    TestLocation {
        function: "my_property".to_string(),
        class: "my_crate::tests".to_string(),
        file: "tests/my_tests.rs".to_string(),
        begin_line: 42,
    }
}

/// The JSON object Antithesis's SDKs write for one assertion event.
fn expected_event(location: &TestLocation, hit: bool, condition: bool) -> serde_json::Value {
    let id = format!(
        "{}::{} passes properties",
        location.class, location.function
    );
    serde_json::json!({
        "antithesis_assert": {
            "hit": hit,
            "must_hit": true,
            "assert_type": "always",
            "display_type": "Always",
            "condition": condition,
            "id": id,
            "message": id,
            "location": {
                "class": location.class,
                "function": location.function,
                "file": location.file,
                "begin_line": location.begin_line,
                "begin_column": 0,
            },
        }
    })
}

fn parse_lines(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn assertion_lines_declare_then_evaluate_the_assertion() {
    for passed in [true, false] {
        let text = assertion_lines(&location(), passed);
        assert!(text.ends_with('\n'));
        assert_eq!(
            parse_lines(&text),
            [
                expected_event(&location(), false, false),
                expected_event(&location(), true, passed),
            ]
        );
    }
}

#[test]
fn assertion_lines_escape_the_location_strings() {
    let awkward = TestLocation {
        function: "quotes \" and \\ backslashes".to_string(),
        class: "tabs\tnewlines\nreturns\r".to_string(),
        file: "control \u{1} chars \u{1f} and unicode é 日本 \u{1F600}".to_string(),
        begin_line: u32::MAX,
    };
    assert_eq!(
        parse_lines(&assertion_lines(&awkward, true)),
        [
            expected_event(&awkward, false, false),
            expected_event(&awkward, true, true),
        ]
    );
}

#[test]
fn json_string_round_trips_every_ascii_character() {
    let all: String = (0u8..128).map(char::from).collect();
    let decoded: String = serde_json::from_str(&json_string(&all)).unwrap();
    assert_eq!(decoded, all);
}

/// An output whose lines are collected into the returned buffer.
fn capturing_output() -> (Output, Arc<Mutex<Vec<String>>>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    let output = Output::callback(move |line: &str| sink.lock().unwrap().push(line.to_string()));
    (output, lines)
}

#[test]
fn nothing_is_reported_outside_antithesis() {
    let (output, lines) = capturing_output();
    report_with(|_| None, &location(), false, &output);
    assert!(lines.lock().unwrap().is_empty());
}

#[cfg(not(windows))]
#[test]
fn reports_append_to_the_sdk_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let env = env_with_output_dir(dir.path().to_str().unwrap());
    let (output, lines) = capturing_output();
    report_with(&env, &location(), true, &output);
    report_with(&env, &location(), false, &output);
    assert!(lines.lock().unwrap().is_empty());
    let text = std::fs::read_to_string(dir.path().join("sdk.jsonl")).unwrap();
    assert_eq!(
        parse_lines(&text),
        [
            expected_event(&location(), false, false),
            expected_event(&location(), true, true),
            expected_event(&location(), false, false),
            expected_event(&location(), true, false),
        ]
    );
}

#[cfg(not(windows))]
#[test]
fn an_unwritable_sdk_file_is_announced_on_the_output() {
    let env = env_with_output_dir("/no/such/antithesis/output/dir/for/hegel/tests");
    let (output, lines) = capturing_output();
    report_with(env, &location(), true, &output);
    let lines = lines.lock().unwrap();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("my_crate::tests::my_property passes properties")
            && lines[0].contains("/no/such/antithesis/output/dir/for/hegel/tests/sdk.jsonl"),
        "{}",
        lines[0]
    );
}

#[test]
fn no_fuzz_source_is_looked_up_outside_antithesis() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("libvoidstar.so");
    assert!(fuzz_source_from(|_| None, path.to_str().unwrap()).is_none());
}

#[cfg(not(windows))]
#[test]
fn no_fuzz_source_inside_antithesis_without_the_library() {
    let dir = tempfile::TempDir::new().unwrap();
    let env = env_with_output_dir(dir.path().to_str().unwrap());
    let path = dir.path().join("libvoidstar.so");
    assert!(fuzz_source_from(env, path.to_str().unwrap()).is_none());
}

#[test]
fn a_reporter_outside_antithesis_is_silent() {
    let (output, lines) = capturing_output();
    Reporter::new(location(), output).report(false);
    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn pausing_does_nothing_outside_antithesis() {
    if crate::sys::env_var(FAULT_CONTROL_VAR).is_none() {
        pause_faults().unwrap();
        unpause_faults().unwrap();
    }
}

/// A fake environment holding only `ANTITHESIS_FAULT_CONTROL`.
#[cfg(unix)]
fn env_with_fault_control(program: &str) -> impl Fn(&str) -> Option<String> {
    let program = program.to_string();
    move |key| (key == "ANTITHESIS_FAULT_CONTROL").then(|| program.clone())
}

/// A runner that must not be called.
#[cfg(unix)]
fn never_run(program: &str, args: &[&str]) -> Result<i32, crate::sys::Error> {
    panic!("ran {program} {args:?}");
}

/// The calls a [`recording_run`] runner has seen: the program and its
/// arguments.
#[cfg(unix)]
type CallLog = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// A runner recording each call into the returned log and answering with
/// `result`.
#[cfg(unix)]
fn recording_run(
    result: Result<i32, crate::sys::Error>,
) -> (
    impl Fn(&str, &[&str]) -> Result<i32, crate::sys::Error>,
    CallLog,
) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&calls);
    let run = move |program: &str, args: &[&str]| {
        let args = args.iter().map(|a| a.to_string()).collect();
        log.lock().unwrap().push((program.to_string(), args));
        result
    };
    (run, calls)
}

/// An empty file standing in for the fault-control program, in `dir`.
#[cfg(unix)]
fn fake_program(dir: &tempfile::TempDir) -> String {
    let program = dir.path().join("fault-control");
    std::fs::write(&program, b"").unwrap();
    program.to_str().unwrap().to_string()
}

#[cfg(unix)]
#[test]
fn fault_control_is_skipped_when_the_variable_is_unset() {
    fault_control_from(|_| None, never_run, PAUSE_FLAG).unwrap();
    fault_control_from(|_| None, never_run, UNPAUSE_FLAG).unwrap();
}

#[cfg(unix)]
#[test]
fn fault_control_is_skipped_when_the_program_does_not_exist() {
    let env = env_with_fault_control("/no/such/antithesis/fault/control/for/hegel/tests");
    fault_control_from(&env, never_run, PAUSE_FLAG).unwrap();
    fault_control_from(&env, never_run, UNPAUSE_FLAG).unwrap();
}

#[cfg(unix)]
#[test]
fn fault_control_runs_the_program_with_the_pause_or_unpause_flag() {
    let dir = tempfile::TempDir::new().unwrap();
    let program = fake_program(&dir);
    let env = env_with_fault_control(&program);
    let (run, calls) = recording_run(Ok(0));
    fault_control_from(&env, &run, PAUSE_FLAG).unwrap();
    fault_control_from(&env, &run, UNPAUSE_FLAG).unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        [
            (program.clone(), vec!["--pause".to_string()]),
            (program.clone(), vec!["--unpause".to_string()]),
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_non_zero_exit_from_the_program_is_an_error_naming_it() {
    let dir = tempfile::TempDir::new().unwrap();
    let program = fake_program(&dir);
    let (run, _) = recording_run(Ok(3));
    let message =
        fault_control_from(env_with_fault_control(&program), run, PAUSE_FLAG).unwrap_err();
    assert_eq!(
        message,
        format!("ANTITHESIS_FAULT_CONTROL={program} --pause exited with status 3")
    );
}

#[cfg(unix)]
#[test]
fn a_program_that_cannot_run_is_an_error_naming_it() {
    let dir = tempfile::TempDir::new().unwrap();
    let program = fake_program(&dir);
    let (run, _) = recording_run(Err(crate::sys::Error));
    let message =
        fault_control_from(env_with_fault_control(&program), run, UNPAUSE_FLAG).unwrap_err();
    assert_eq!(
        message,
        format!(
            "ANTITHESIS_FAULT_CONTROL={program} --unpause could not be run, or did not exit normally"
        )
    );
}

/// A child shell writes the script: a file this process wrote could still
/// be open for writing in a sibling test's child between its fork and
/// exec, and executing it would then fail with `ETXTBSY`.
#[cfg(unix)]
#[test]
fn fault_control_really_runs_the_program() {
    let dir = tempfile::TempDir::new().unwrap();
    let program = dir.path().join("fault-control");
    let log = dir.path().join("log");
    let script = format!("#!/bin/sh\nprintf '%s\\n' \"$1\" >> {}", log.display());
    let write = "printf '%s\\n' \"$2\" > \"$1\" && chmod 755 \"$1\"";
    let args = ["-c", write, "sh", program.to_str().unwrap(), &script];
    assert_eq!(crate::sys::process::run("/bin/sh", &args).unwrap(), 0);
    let env = env_with_fault_control(program.to_str().unwrap());
    fault_control_from(&env, crate::sys::process::run, PAUSE_FLAG).unwrap();
    fault_control_from(&env, crate::sys::process::run, UNPAUSE_FLAG).unwrap();
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "--pause\n--unpause\n"
    );
}
