//! The Antithesis fault-control hook driven through the frontend with
//! `ANTITHESIS_FAULT_CONTROL` really set: a program that fails makes the
//! invariant's run a usage error, whether the pause or the unpause failed.
//!
//! The variable is process-wide, so it lives in its own test binary, is set
//! once to one script, and the tests take turns, each choosing which flag
//! the script rejects through a sidecar file next to it.

#![cfg(unix)]

mod common;

use common::utils::expect_panic;
use hegel::stateful::machine;
use hegel::{Hegel, Settings, TestCase};
use std::os::unix::fs::PermissionsExt;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

static DIR: OnceLock<TempDir> = OnceLock::new();
static TURN: Mutex<()> = Mutex::new(());

/// The script `ANTITHESIS_FAULT_CONTROL` names: it exits with status 3 when
/// its argument is the flag written to `rejected`, and with 0 otherwise.
const SCRIPT: &str =
    "#!/bin/sh\n[ \"$1\" = \"$(cat \"$(dirname \"$0\")/rejected\")\" ] && exit 3\nexit 0\n";

/// Take the turn: install the script and point `ANTITHESIS_FAULT_CONTROL`
/// at it (once), and make it reject `flag`.
fn rejecting(flag: &str) -> MutexGuard<'static, ()> {
    let guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let dir = DIR.get_or_init(|| {
        let dir = TempDir::new().unwrap();
        let program = dir.path().join("fault-control");
        std::fs::write(&program, SCRIPT).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        unsafe { std::env::set_var("ANTITHESIS_FAULT_CONTROL", &program) };
        dir
    });
    std::fs::write(dir.path().join("rejected"), flag).unwrap();
    guard
}

struct Paused;

#[hegel::state_machine]
impl Paused {
    #[rule]
    fn step(&mut self, _tc: TestCase) {}

    #[invariant(pause_faults = true)]
    fn holds(&self, _tc: TestCase) {}
}

fn run_paused_machine() {
    Hegel::new(|tc: TestCase| machine(Paused).run(tc))
        .settings(
            Settings::new()
                .test_cases(5)
                .database(None)
                .derandomize(true),
        )
        .run();
}

#[test]
fn a_failing_pause_is_a_usage_error() {
    let _turn = rejecting("--pause");
    expect_panic(
        run_paused_machine,
        "ANTITHESIS_FAULT_CONTROL=.*/fault-control --pause exited with status 3",
    );
}

#[test]
fn a_failing_unpause_is_a_usage_error() {
    let _turn = rejecting("--unpause");
    expect_panic(
        run_paused_machine,
        "ANTITHESIS_FAULT_CONTROL=.*/fault-control --unpause exited with status 3",
    );
}
