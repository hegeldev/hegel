//! The Antithesis fault-control hook driven through the C ABI with
//! `ANTITHESIS_FAULT_CONTROL` really set: `hegel_pause_faults` and
//! `hegel_unpause_faults` run the program it names with `--pause` and
//! `--unpause`, and report its failures.
//!
//! The variable is process-wide, so it lives in its own test binary, is set
//! once to one script, and the tests take turns, each choosing how the
//! script behaves through a sidecar file next to it and truncating its log.

#![cfg(unix)]

mod common;

use common::{last_error, make_settings_no_db, next_case, ok, start};
use hegel_c::hegel_result_t::{HEGEL_E_INVALID_ARG, HEGEL_OK};
use hegel_c::{
    hegel_context_free, hegel_context_new, hegel_mark_complete, hegel_pause_faults, hegel_run_free,
    hegel_settings_free, hegel_status_t, hegel_test_case_free, hegel_unpause_faults,
};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

static DIR: OnceLock<TempDir> = OnceLock::new();
static TURN: Mutex<()> = Mutex::new(());

/// The script `ANTITHESIS_FAULT_CONTROL` names: it appends its argument to
/// `log`, then exits with the status read from `status`.
const SCRIPT: &str = "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$(dirname \"$0\")/log\"\nexit \"$(cat \"$(dirname \"$0\")/status\")\"\n";

/// Take the turn: install the script and point `ANTITHESIS_FAULT_CONTROL`
/// at it (once), make it exit with `status`, and empty its log.
fn take_turn(status: i32) -> (MutexGuard<'static, ()>, PathBuf) {
    let guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let dir = DIR.get_or_init(|| {
        let dir = TempDir::new().unwrap();
        let program = dir.path().join("fault-control");
        std::fs::write(&program, SCRIPT).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        unsafe { std::env::set_var("ANTITHESIS_FAULT_CONTROL", &program) };
        dir
    });
    std::fs::write(dir.path().join("status"), status.to_string()).unwrap();
    let log = dir.path().join("log");
    std::fs::write(&log, "").unwrap();
    (guard, log)
}

fn log_lines(log: &Path) -> String {
    std::fs::read_to_string(log).unwrap()
}

/// Run `body` against one live test case, then finish the case and the run.
unsafe fn with_test_case(
    body: impl FnOnce(*mut hegel_c::HegelContext, *mut hegel_c::HegelTestCase),
) {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let run = start(ctx, s);
        let tc = next_case(ctx, run);
        body(ctx, tc);
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
        ));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn pausing_and_unpausing_run_the_program_with_each_flag() {
    let (_turn, log) = take_turn(0);
    unsafe {
        with_test_case(|ctx, tc| {
            assert_eq!(hegel_pause_faults(ctx, tc), HEGEL_OK);
            assert_eq!(log_lines(&log), "--pause\n");
            assert_eq!(hegel_unpause_faults(ctx, tc), HEGEL_OK);
            assert_eq!(log_lines(&log), "--pause\n--unpause\n");
        });
    }
}

#[test]
fn a_failing_program_is_reported_from_either_call() {
    let (_turn, log) = take_turn(3);
    unsafe {
        with_test_case(|ctx, tc| {
            assert_eq!(hegel_pause_faults(ctx, tc), HEGEL_E_INVALID_ARG);
            let message = last_error(ctx);
            assert!(
                message.starts_with("hegel_pause_faults: ANTITHESIS_FAULT_CONTROL="),
                "{message}"
            );
            assert!(
                message.ends_with("/fault-control --pause exited with status 3"),
                "{message}"
            );
            assert_eq!(hegel_unpause_faults(ctx, tc), HEGEL_E_INVALID_ARG);
            let message = last_error(ctx);
            assert!(
                message.ends_with("/fault-control --unpause exited with status 3"),
                "{message}"
            );
            assert_eq!(log_lines(&log), "--pause\n--unpause\n");
        });
    }
}
