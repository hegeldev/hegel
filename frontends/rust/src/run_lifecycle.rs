//! Shared per-test-case execution lifecycle.
//!
//! Provides the panic hook, the `run_test_case` wrapper that catches a single
//! test body's panic and converts it into a [`TestCaseResult`], and the
//! `drive` function that starts a libhegel run, pulls each test case the
//! engine schedules, and surfaces the run-level result.
//!
//! The engine lives behind libhegel's C ABI; `drive` owns everything around
//! it — installing the panic hook, wrapping each test body with
//! `catch_unwind` plus `mark_complete`, handing the engine each failure's
//! summary and rendered diagnostic, printing each reported failure from the
//! report the engine kept of it, and the closing re-raise of the failing
//! test's own panic. The engine prints each test case's output itself.

use std::backtrace::{Backtrace, BacktraceStatus};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Once};

use crate::backend::{Failure, TestCaseResult};
use crate::control::{
    AssumeFailed, InternalError, InvalidArgument, LoopDone, StopTest, currently_in_test_context,
    hegel_internal_error, hegel_internal_unwrap, with_test_context,
};
use crate::explicit_test_case::ExplicitTestCase;
use crate::ffi::{CTestCase, FailureReport, RunErrorKind, RunHandle, SettingsHandle};
use crate::runner::{Settings, TestLocation, Verbosity};
use crate::test_case::{RunOutput, TestCase};

static PANIC_HOOK_INIT: Once = Once::new();

thread_local! {
    /// `(thread_name, thread_id, location, backtrace)` captured by the panic
    /// hook for the most recent panic raised inside a test context. The
    /// hook overwrites this on every panic; callers consume it with
    /// [`take_panic_info`] right after `catch_unwind` returns.
    static LAST_PANIC_INFO: RefCell<Option<(String, String, String, Backtrace)>> =
        const { RefCell::new(None) };

    /// Whether the panic hook should pay to capture a backtrace for the next
    /// panic on this thread. Set by [`run_test_case`] to whether the engine
    /// wants the test case's detail — i.e. only when the resulting full
    /// diagnostic will actually be shown.
    /// Capturing (and, under
    /// `RUST_BACKTRACE`, symbolizing) a backtrace for every discarded shrink
    /// probe is the dominant cost of failing-heavy property runs, and is far
    /// worse on Windows.
    static CAPTURE_BACKTRACE: Cell<bool> = const { Cell::new(false) };
}

/// The `(thread_name, thread_id, location, backtrace)` tuple the panic hook
/// captures.
pub(crate) type PanicInfo = (String, String, String, Backtrace);

pub(crate) fn take_panic_info() -> Option<PanicInfo> {
    LAST_PANIC_INFO.with(|info| info.borrow_mut().take())
}

/// Install `info` into this thread's panic-info slot, as if the panic hook
/// had captured it here. Used by `stateful::Machine::run_concurrent` to re-install a
/// worker thread's capture on the main thread before `resume_unwind`ing the
/// ferried payload — the re-raise skips the panic hook, so without this the
/// lifecycle would fall back to [`unknown_panic_info`] and every concurrent
/// failure would share the origin `"Panic at <unknown>"`.
pub(crate) fn install_panic_info(info: PanicInfo) {
    LAST_PANIC_INFO.with(|slot| *slot.borrow_mut() = Some(info));
}

/// Whether the panic hook captures backtraces on this thread right now.
/// `run_test_case` decides this per case; `stateful::Machine::run_concurrent` reads
/// it on the main thread to mirror the setting onto its worker threads.
pub(crate) fn backtrace_capture_enabled() -> bool {
    CAPTURE_BACKTRACE.get()
}

/// Set whether the panic hook captures backtraces on this thread. Called by
/// `stateful::Machine::run_concurrent` on each worker thread (see
/// [`backtrace_capture_enabled`]).
pub(crate) fn set_backtrace_capture(enabled: bool) {
    CAPTURE_BACKTRACE.set(enabled);
}

/// Install the cross-backend panic hook on first call.
///
/// Idempotent across all backends: the hook captures the location for any
/// panic raised inside a test context (so [`run_test_case`] can read it after
/// `catch_unwind`), and forwards everything else to the previous hook
/// unchanged. A backtrace is captured only when [`CAPTURE_BACKTRACE`] is
/// set. Control-flow unwinds (a rejected assumption, out-of-data, ...)
/// never reach any hook at all — they are raised via
/// [`crate::control::raise_control`]'s `resume_unwind`, which skips hooks
/// by construction.
pub(crate) fn init_panic_hook() {
    PANIC_HOOK_INIT.call_once(|| {
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !currently_in_test_context() {
                prev_hook(info);
                return;
            }

            let thread = std::thread::current();
            let thread_name = thread.name().unwrap_or("<unnamed>").to_string();
            let thread_id = format!("{:?}", thread.id())
                .trim_start_matches("ThreadId(")
                .trim_end_matches(')')
                .to_string();
            let location = panic_location(info.location());
            let backtrace = if CAPTURE_BACKTRACE.get() {
                Backtrace::capture()
            } else {
                Backtrace::disabled()
            };

            LAST_PANIC_INFO
                .with(|l| *l.borrow_mut() = Some((thread_name, thread_id, location, backtrace)));
        }));
    });
}

/// Where a panic was raised, as `file:line:column`.
fn panic_location(location: Option<&panic::Location<'_>>) -> String {
    match location {
        Some(loc) => format!("{}:{}:{}", loc.file(), loc.line(), loc.column()),
        None => "<unknown>".to_string(),
    }
}

/// Format a backtrace, optionally filtering to the "short" format that the
/// default Rust panic handler uses.
fn format_backtrace(bt: &Backtrace, full: bool) -> String {
    let backtrace_str = format!("{}", bt);
    if full {
        return backtrace_str;
    }
    filter_short_backtrace(&backtrace_str)
}

/// Trim a `Backtrace`-as-string down to the "short" view (between
/// `__rust_end_short_backtrace` and `__rust_begin_short_backtrace`) and
/// renumber the surviving frames. Split out from [`format_backtrace`] so
/// the string-shape branches (frames with a `digit:` prefix, frames with
/// a digit but no colon, frames that don't start with a digit) can be
/// covered directly.
fn filter_short_backtrace(backtrace_str: &str) -> String {
    let lines: Vec<&str> = backtrace_str.lines().collect();
    let mut start_idx = 0;
    let mut end_idx = lines.len();

    for (i, line) in lines.iter().enumerate() {
        if line.contains("__rust_end_short_backtrace") {
            for (j, next_line) in lines.iter().enumerate().skip(i + 1) {
                if next_line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
                {
                    start_idx = j;
                    break;
                }
            }
        }
        if line.contains("__rust_begin_short_backtrace") {
            for (j, prev_line) in lines
                .iter()
                .enumerate()
                .take(i + 1)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                if prev_line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
                {
                    end_idx = j;
                    break;
                }
            }
            break;
        }
    }

    if start_idx > end_idx {
        start_idx = 0;
        end_idx = lines.len();
    }
    let filtered: Vec<&str> = lines[start_idx..end_idx].to_vec();
    let mut new_frame_num = 0usize;
    let mut result = Vec::new();
    for line in filtered {
        let trimmed = line.trim_start();
        if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            if let Some(colon_pos) = trimmed.find(':') {
                let rest = &trimmed[colon_pos..];
                result.push(format!("{:>4}{}", new_frame_num, rest));
                new_frame_num += 1;
            } else {
                result.push(line.to_string());
            }
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

/// Placeholder thread/location/backtrace tuple used when the panic hook
/// captured nothing for a caught panic. This *is* reached in production:
/// a panic payload ferried from another thread and re-raised on the test
/// thread with `resume_unwind` skips the hook, so the lifecycle finds
/// nothing here. (A plain `join().unwrap()` instead panics afresh
/// through the hook, pinning the origin to the join site and degrading
/// the message; a never-joined thread's panic produces no failure at
/// all.) One consequence is that every ferried failure shares the origin
/// `"Panic at <unknown>"`, merging distinct threaded bugs into one
/// counterexample; fixing that needs cross-thread capture, which is
/// deferred until there is structured concurrency support to hang it on.
pub(crate) fn unknown_panic_info() -> (String, String, String, Backtrace) {
    (
        "<unknown>".to_string(),
        "?".to_string(),
        "<unknown>".to_string(),
        Backtrace::disabled(),
    )
}

/// Extract a string message from a panic payload. Pre-N1 this was duplicated
/// in `src/native/runner.rs:11`; that copy now re-exports this one.
///
/// `pub` so that the libhegel C bindings (`libhegel/`) can use it from their
/// own `catch_unwind` wrapper around `run_native`. Not part of the
/// supported public surface — `#[doc(hidden)]`.
#[doc(hidden)]
pub fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    }
}

/// Run the user's test body once against the supplied libhegel test case,
/// catching any panic and translating it to a [`TestCaseResult`].
///
/// The body's `TestCase` shares `c_tc` through an `Arc`, and the outcome is
/// reported on the same shared handle after the body finishes — so no
/// per-case libhegel clone is needed (a real clone is created only when the
/// body itself calls `tc.clone()`), while a `TestCase` that escapes the body
/// (moved to a thread that is never joined) keeps the handle alive rather
/// than dangling; its later draws fail cleanly because the case has finished.
///
/// Reports the outcome back to the engine via [`report_outcome`] (which calls
/// `hegel_mark_complete`): that is the channel for per-test-case results, and
/// the engine reads it back over the C ABI.
/// On the `Interesting` path the panic site is captured as a
/// `file:line:col` string and stored on the [`Failure`] so per-origin
/// shrinking can key on it.
///
/// What is printed is the engine's decision, read once at case start. When
/// it wants the case's output ([`CTestCase::wants_output`]) — every case of
/// a verbose run, and the executions it stamps for capture because a
/// failure report can be built from them — draws and notes are printed and
/// a failing case hands it a one-line summary (the panic line and message).
/// When it also wants the case's detail ([`CTestCase::wants_detail`]) — a
/// debug run, and the stamped executions — a backtrace is captured and the
/// full diagnostic (panic location, message, backtrace) rendered and handed
/// over too. Both reach the engine with the outcome (`hegel_mark_complete`),
/// and it prints them after the case's output, wherever that goes.
///
/// Also returns the caught panic payload for an `Interesting` result, so
/// the caller can re-raise the test's *own* panic as the run's closing
/// unwind instead of synthesizing one, and whether the engine wanted the
/// case's output, which says whether that payload matches the failure's
/// report.
pub(crate) fn run_test_case(
    c_tc: CTestCase,
    test_fn: &mut dyn FnMut(TestCase),
) -> (TestCaseResult, Option<Box<dyn std::any::Any + Send>>, bool) {
    let wants_output = c_tc.wants_output();
    let wants_detail = c_tc.wants_detail();
    CAPTURE_BACKTRACE.with(|c| c.set(wants_detail));
    // Drop any capture left over from a previous test case on this thread
    // (e.g. a body that caught its own panic and then passed): a later panic
    // that skips the hook via resume_unwind must not inherit it.
    take_panic_info();

    let c_tc = Arc::new(c_tc);
    let tc = TestCase::new(Arc::clone(&c_tc), wants_output);
    let result = with_test_context(|| catch_unwind(AssertUnwindSafe(|| test_fn(tc))));

    let mut summary = None;
    let mut detail = None;
    let (tc_result, payload) = match result {
        Ok(()) => (TestCaseResult::Valid, None),
        Err(e) if e.downcast_ref::<AssumeFailed>().is_some() => (TestCaseResult::Invalid, None),
        Err(e) if e.downcast_ref::<StopTest>().is_some() => (TestCaseResult::Overrun, None),
        Err(e) if e.downcast_ref::<LoopDone>().is_some() => (TestCaseResult::Valid, None),
        Err(e) => {
            let e = match e.downcast::<InvalidArgument>() {
                Ok(invalid) => panic!("{}", invalid.0),
                Err(e) => e,
            };
            let e = match e.downcast::<InternalError>() {
                Ok(internal) => panic!("{}", internal.0),
                Err(e) => e,
            };
            let (thread_name, thread_id, location, backtrace) =
                take_panic_info().unwrap_or_else(unknown_panic_info);
            let msg = panic_message(&e);
            if wants_output {
                summary = Some(render_summary(&thread_name, &thread_id, &location, &msg));
            }
            if wants_detail {
                detail = Some(render_diagnostic(
                    &thread_name,
                    &thread_id,
                    &location,
                    &msg,
                    &backtrace,
                ));
            }
            let failure = TestCaseResult::Interesting(Failure {
                origin: format!("Panic at {}", location),
            });
            (failure, Some(e))
        }
    };

    report_outcome(&c_tc, &tc_result, summary.as_deref(), detail.as_deref());

    (tc_result, payload, wants_output)
}

/// A failure in one line: the panic line Rust prints followed by the
/// message, a message spanning several lines cut at its first line and
/// followed by ` …`.
fn render_summary(thread_name: &str, thread_id: &str, location: &str, msg: &str) -> String {
    let first = msg.lines().next().unwrap_or("");
    let cut = if msg.trim_end().contains('\n') {
        " …"
    } else {
        ""
    };
    format!("thread '{thread_name}' ({thread_id}) panicked at {location}: {first}{cut}")
        .trim_end()
        .to_string()
}

/// A failing explicit test case: its origin, for keeping one failure per
/// origin and dropping the generated failures that share it, and its report
/// material.
pub(crate) struct ExplicitFailure {
    pub(crate) origin: String,
    report: Vec<String>,
    payload: Box<dyn std::any::Any + Send>,
}

impl ExplicitFailure {
    /// The failure as the report prints it: an explicit test case has no
    /// caveat and no blob to reproduce it with.
    pub(crate) fn into_reported(self) -> ReportedFailure {
        ReportedFailure {
            origin: self.origin,
            report: self.report,
            payload: self.payload,
            caveat: None,
            reproducer: None,
        }
    }
}

/// Run the user's test body once against an explicit test case, catching
/// its panic, as [`run_test_case`] does for a generated one.
///
/// A body that passes must have drawn every value the case was given, or
/// the test is misused and panics saying which values were not drawn. A
/// usage error the body hits ([`InvalidArgument`]: a rejected explicit test
/// case, a draw with no value) and an [`InternalError`] panic at once with
/// their message, as they do in a generated case. Any other panic is a
/// failure, returned with the case's draw and note lines and the rendered
/// diagnostic (neither under quiet) for the caller to report. A verbose run
/// prints the case's lines and the failure's summary as it finishes, and a
/// debug run its full diagnostic, as the engine does for a generated case.
pub(crate) fn run_explicit_case<F: FnOnce(&ExplicitTestCase)>(
    case: &ExplicitTestCase,
    body: F,
    verbosity: Verbosity,
    output: &RunOutput,
) -> Option<ExplicitFailure> {
    init_panic_hook();
    let verbose = matches!(verbosity, Verbosity::Verbose | Verbosity::Debug);
    let quiet = verbosity == Verbosity::Quiet;
    CAPTURE_BACKTRACE.with(|c| c.set(!quiet));
    take_panic_info();

    let result = with_test_context(|| catch_unwind(AssertUnwindSafe(|| body(case))));
    let lines = if quiet { Vec::new() } else { case.lines() };

    let payload = match result {
        Ok(()) => None,
        Err(e) if e.downcast_ref::<LoopDone>().is_some() => None,
        Err(e) => Some(e),
    };
    let Some(payload) = payload else {
        if verbose {
            for line in &lines {
                output.line(line);
            }
        }
        let unused = case.unused_values();
        if !unused.is_empty() {
            panic!(
                "Explicit test case: the following values were provided \
                 but never drawn: {:?}",
                unused
            );
        }
        return None;
    };
    let payload = match payload.downcast::<InvalidArgument>() {
        Ok(invalid) => panic!("{}", invalid.0),
        Err(e) => e,
    };
    let payload = match payload.downcast::<InternalError>() {
        Ok(internal) => panic!("{}", internal.0),
        Err(e) => e,
    };
    let (thread_name, thread_id, location, backtrace) =
        take_panic_info().unwrap_or_else(unknown_panic_info);
    let msg = panic_message(&payload);
    let diagnostic = if quiet {
        None
    } else {
        Some(render_diagnostic(
            &thread_name,
            &thread_id,
            &location,
            &msg,
            &backtrace,
        ))
    };
    if verbose {
        for line in &lines {
            output.line(line);
        }
        match (verbosity, &diagnostic) {
            (Verbosity::Debug, Some(diagnostic)) => output.block(diagnostic),
            _ => output.line(&render_summary(&thread_name, &thread_id, &location, &msg)),
        }
    }
    let mut report = lines;
    report.extend(
        diagnostic
            .iter()
            .flat_map(|d| d.lines().map(str::to_string)),
    );
    Some(ExplicitFailure {
        origin: format!("Panic at {}", location),
        report,
        payload,
    })
}

/// Report a test case's outcome to the engine over the C ABI.
///
/// The status crosses the boundary, and for an interesting (failing) case
/// its origin, `summary` and `detail` (each of the last two present exactly
/// when the engine wants it); the reproduce blob is recovered from the run
/// result afterward. This reports on the handle the body drew on (shared
/// with it through the `Arc`); `hegel_mark_complete` waits for any
/// in-flight operation on the handle, so a still-running leaked thread
/// cannot make this report fail. A call the engine rejects as incomplete
/// is an internal error.
fn report_outcome(
    handle: &CTestCase,
    result: &TestCaseResult,
    summary: Option<&str>,
    detail: Option<&str>,
) {
    use crate::ffi::sys::hegel_status_t as Status;
    let (status, failure) = match result {
        TestCaseResult::Valid => (Status::HEGEL_STATUS_VALID, None),
        TestCaseResult::Invalid => (Status::HEGEL_STATUS_INVALID, None),
        TestCaseResult::Overrun => (Status::HEGEL_STATUS_OVERRUN, None),
        TestCaseResult::Interesting(failure) => (
            Status::HEGEL_STATUS_INTERESTING,
            Some(FailureReport {
                origin: &failure.origin,
                summary,
                detail,
            }),
        ),
    };
    handle
        .mark_complete(status, failure.as_ref())
        .unwrap_or_else(|rc| match rc {
            crate::ffi::sys::hegel_result_t::HEGEL_E_INVALID_ARG => hegel_internal_error!(
                "libhegel rejected a test case's outcome: {}",
                crate::ffi::last_error_string()
            ),
            rc => crate::test_case::raise_for_rc(rc),
        });
}

/// Render a failure's diagnostic block. Mirrors the default Rust
/// panic-handler output so each failure in the report looks like a
/// stand-alone test failure.
fn render_diagnostic(
    thread_name: &str,
    thread_id: &str,
    location: &str,
    msg: &str,
    backtrace: &Backtrace,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "thread '{}' ({}) panicked at {}:\n",
        thread_name, thread_id, location
    ));
    out.push_str(msg);
    out.push('\n');
    // nocov start
    if backtrace.status() == BacktraceStatus::Captured {
        let is_full = std::env::var("RUST_BACKTRACE")
            .map(|v| v == "full")
            .unwrap_or(false);
        let formatted = format_backtrace(backtrace, is_full);
        out.push_str(&format!("stack backtrace:\n{}\n", formatted));
        if !is_full {
            out.push_str(
                "note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.\n",
            );
        }
    }
    // nocov end
    out
}

/// The copy-pasteable reproduce failure text to append after a failure's diagnostic,
/// or `None` when nothing should be printed.
///
/// `Some` only when [`Settings::print_blob`](crate::Settings::print_blob) is
/// enabled *and* the failure carries a reproduce blob. A blobless failure —
/// an unconfirmed nondeterministic failure reported caveat-only, a blob
/// replay (the caller already holds the blob) — prints nothing.
///
/// `attribute` is the attribute macro the run came from, which the
/// `#[hegel::reproduce_failure]` attribute goes below; a run built with the
/// [`Hegel`](crate::Hegel) builder directly is told to call
/// [`Hegel::reproduce_failure`](crate::Hegel::reproduce_failure) instead.
fn reproducer_line(
    settings: &Settings,
    reproduce_blob: Option<&str>,
    attribute: Option<&str>,
) -> Option<String> {
    if !settings.print_blob {
        return None;
    }
    let blob = reproduce_blob?;
    Some(match attribute {
        Some(attribute) => format!(
            "\nTo reproduce this failure, add the attribute below \
             {attribute}:\n    #[hegel::reproduce_failure(\"{blob}\")]"
        ),
        None => format!(
            "\nTo reproduce this failure, call this on the Hegel builder \
             before run():\n    .reproduce_failure(\"{blob}\")"
        ),
    })
}

/// The diagnostic of a failure the engine kept no diagnostic of
/// (`hegel_failure_diagnostic` is NULL): it reported the failure from a case it
/// never stamped for capture, as it does for a run it does not record (a
/// single test case inside Antithesis with no database), whose failure is
/// never replayed. It holds what such a case keeps, the panic's site (its
/// origin) and its message, and no backtrace.
fn bare_diagnostic(origin: &str, payload: &Box<dyn std::any::Any + Send>) -> String {
    format!("{origin}:\n{}\n", panic_message(payload))
}

/// Drive a libhegel run to completion against the user's test function.
///
/// Installs the cross-backend panic hook, starts the engine through the C ABI
/// (`hegel_run_start`), and pulls each test case the engine schedules
/// (`hegel_next_test_case`), wrapping every one in [`run_test_case`]. The
/// engine owns the whole exploration — generation, shrinking, and the final
/// replay of every failure it is about to report — and prints each test
/// case's output, keeping the report of each failure itself.
///
/// Once the loop drains, a failed verdict prints each distinct failure as
/// one grouped block — the engine's report of it (the printed lines and
/// diagnostic), the caveat when the run handled nondeterminism, and the
/// reproducer line when the failure carries a blob — and ends by re-raising
/// the failing test's own
/// panic (or, for several distinct bugs, a panic carrying the count). A
/// run-level error — a failed health check, a nondeterminism abort under
/// `error` strictness, an engine panic — surfaces with the engine's own
/// message instead of the `Property test failed:` framing.
///
/// `explicit_failures` are the failures the explicit phase collected under
/// `report_multiple_failures` (see [`ExplicitRun`](crate::ExplicitRun)).
/// They are reported first, a generated failure sharing one's origin is
/// dropped, and a run-level error does not hide them: it is reported after
/// them as one more distinct failure.
pub(crate) fn drive<F>(
    test_fn: F,
    settings: &Settings,
    database_key: Option<&str>,
    test_location: Option<&TestLocation>,
    reproduce_attribute: Option<&str>,
    explicit_failures: Vec<ExplicitFailure>,
) where
    F: FnMut(TestCase),
{
    init_panic_hook();
    let mut test_fn = test_fn;
    let output = RunOutput::resolve();
    let quiet = settings.verbosity == Verbosity::Quiet;

    let c_settings = SettingsHandle::build(settings, database_key, test_location);
    let run = match RunHandle::start(&c_settings, output.sink()) {
        Ok(run) => run,
        Err(message) => panic!("RunError: {message}"), // nocov
    };

    let generated = drive_run(run, &mut test_fn, settings, reproduce_attribute);
    if explicit_failures.is_empty() {
        match generated {
            Ok(failures) if failures.is_empty() => return,
            Ok(failures) => raise_failures(failures, quiet, &output),
            Err(message) => panic!("{message}"),
        }
    }
    let explicit_origins: Vec<String> = explicit_failures
        .iter()
        .map(|failure| failure.origin.clone())
        .collect();
    let mut failures: Vec<ReportedFailure> = explicit_failures
        .into_iter()
        .map(ExplicitFailure::into_reported)
        .collect();
    match generated {
        Ok(generated) => failures.extend(
            generated
                .into_iter()
                .filter(|failure| !explicit_origins.contains(&failure.origin)),
        ),
        Err(message) => failures.push(ReportedFailure {
            origin: String::new(),
            report: if quiet {
                Vec::new()
            } else {
                message.lines().map(str::to_string).collect()
            },
            payload: Box::new(message),
            caveat: None,
            reproducer: None,
        }),
    }
    raise_failures(failures, quiet, &output);
}

/// One failure a run reported: its origin, the lines of its report (the
/// printed output and diagnostic of a failing execution; empty under
/// [`Verbosity::Quiet`]), the panic payload to re-raise for it, and the
/// engine's caveat and the reproducer line to print after it.
pub(crate) struct ReportedFailure {
    origin: String,
    report: Vec<String>,
    payload: Box<dyn std::any::Any + Send>,
    caveat: Option<String>,
    reproducer: Option<String>,
}

/// The shared pump loop behind [`drive`] and [`drive_blob_replay`]: pull
/// every test case off `run`, keep a panic payload per failing origin, then
/// read the run's verdict. A passing run returns no failures, a failed one
/// returns every failure it reported, in the engine's order, and an
/// errored one returns its [`run_error_message`] as the error.
///
/// The engine's report of a failure is that of the latest failing execution
/// it wanted the output of, so the payload kept for an origin is replaced
/// by each such execution's (and kept from any other until one comes): the
/// re-raised panic then matches the printed diagnostic. A failure's report
/// is the lines its execution printed (`hegel_failure_output`) followed by
/// its diagnostic (`hegel_failure_diagnostic`), or by its
/// [`bare_diagnostic`] when the engine kept none.
///
/// `#[inline(always)]`: without it the test body's call stops being
/// inlined and panic backtraces lose the user closure's source location
/// (pinned by `test_output`'s backtrace tests).
#[inline(always)]
fn drive_run<F: FnMut(TestCase)>(
    run: RunHandle,
    test_fn: &mut F,
    settings: &Settings,
    reproduce_attribute: Option<&str>,
) -> Result<Vec<ReportedFailure>, String> {
    let quiet = settings.verbosity == Verbosity::Quiet;
    let mut payloads: HashMap<String, Box<dyn std::any::Any + Send>> = HashMap::new();
    while let Some(c_tc) = run.next_test_case() {
        let (tc_result, payload, wants_output) = run_test_case(c_tc, test_fn);
        if let TestCaseResult::Interesting(failure) = tc_result {
            let payload = hegel_internal_unwrap!(
                payload,
                "an interesting case carries the caught panic payload"
            );
            match payloads.entry(failure.origin) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(payload);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    if wants_output {
                        entry.insert(payload);
                    }
                }
            }
        }
    }

    let result = run.result();
    use crate::ffi::sys::hegel_run_status_t as RunStatus;
    match result.status() {
        RunStatus::HEGEL_RUN_STATUS_PASSED => Ok(Vec::new()),
        RunStatus::HEGEL_RUN_STATUS_ERROR => {
            Err(run_error_message(result.error_kind(), result.error()))
        }
        RunStatus::HEGEL_RUN_STATUS_FAILED => Ok((0..result.failure_count())
            .map(|index| {
                let failure = result.failure(index);
                let payload = hegel_internal_unwrap!(
                    payloads.remove(&failure.origin),
                    "failure {index} has no panic payload"
                );
                let report = if quiet {
                    Vec::new()
                } else {
                    let diagnostic = failure
                        .diagnostic
                        .unwrap_or_else(|| bare_diagnostic(&failure.origin, &payload));
                    let output = failure.output.unwrap_or_default();
                    output
                        .lines()
                        .chain(diagnostic.lines())
                        .map(str::to_string)
                        .collect()
                };
                ReportedFailure {
                    origin: failure.origin,
                    report,
                    payload,
                    reproducer: reproducer_line(
                        settings,
                        failure.reproduce_blob.as_deref(),
                        reproduce_attribute,
                    ),
                    caveat: failure.caveat,
                }
            })
            .collect()),
    }
}

/// Print every failure in `failures` as one grouped block each, under a
/// count header when there are several, and end by re-raising the failing
/// test's own panic, or, for several failures, a panic carrying the count.
pub(crate) fn raise_failures(failures: Vec<ReportedFailure>, quiet: bool, output: &RunOutput) -> ! {
    let count = failures.len();
    let multiple = count > 1;
    if multiple && !quiet {
        output.line(&format!(
            "Property-based test failed with {count} distinct failures."
        ));
    }
    let mut last_payload: Option<Box<dyn std::any::Any + Send>> = None;
    for failure in failures {
        if multiple && !quiet {
            output.line("");
        }
        for line in &failure.report {
            output.line(line);
        }
        if !quiet {
            if let Some(caveat) = &failure.caveat {
                output.line(&format!("note: {caveat}"));
            }
        }
        if let Some(line) = failure.reproducer {
            output.block(&format!("{line}\n"));
        }
        last_payload = Some(failure.payload);
    }

    if multiple {
        std::panic::resume_unwind(Box::new(format!(
            "Property-based test failed with {count} distinct failures."
        )));
    }
    std::panic::resume_unwind(
        last_payload.unwrap_or_else(|| hegel_internal_error!("a failed run has no failures")),
    );
}

/// The panic message for a run that errored rather than passing or
/// failing: the engine's diagnostic, guaranteed to start with the prefix
/// [`Hegel::run`](crate::Hegel::run) documents for its kind —
/// `FailedHealthCheck: <check>`, `Flaky`, `Unsatisfiable` or `RunError`.
/// The engine's own wording usually starts that way already; when it does
/// not, the prefix is added in front of it. A run that errored without a
/// diagnostic gets a placeholder message.
fn run_error_message(kind: RunErrorKind, message: Option<String>) -> String {
    let message = message.unwrap_or_else(|| "the run failed with an unknown error".to_string());
    let prefix = match kind {
        RunErrorKind::HealthCheck(check) => format!("FailedHealthCheck: {check:?}"),
        RunErrorKind::Flaky => "Flaky".to_string(),
        RunErrorKind::Unsatisfiable => "Unsatisfiable".to_string(),
        RunErrorKind::Other => "RunError".to_string(),
    };
    if message.starts_with(&prefix) {
        message
    } else {
        format!("{prefix}: {message}")
    }
}

/// Replay every base64 failure blob in `blobs` through the C ABI
/// (`hegel_run_start_blob`), one replay after another, bypassing
/// generation and shrinking.
///
/// The engine owns each replay: a deterministic blob replays its choices up
/// to four times, stopping at the first failure; a nondeterministic blob
/// replays its stored timelines until one fails, like database reuse.
/// Decoding failures (corrupt or incompatible blobs) panic with the
/// engine's diagnostic. Every failure the replays reproduce is reported,
/// as a run reports the distinct failures it finds: one re-raises the
/// test's own panic, several panic with their count. A replayed example
/// has no fresh blob to print. When no blob reproduces a failure, this
/// panics naming both hypotheses: the failures are fixed, or a
/// nondeterministic one did not recur within the replay budget.
pub(crate) fn drive_blob_replay<F>(
    test_fn: F,
    settings: &Settings,
    database_key: Option<&str>,
    blobs: &[String],
    test_location: Option<&TestLocation>,
    reproduce_attribute: Option<&str>,
) where
    F: FnMut(TestCase),
{
    init_panic_hook();
    let mut test_fn = test_fn;
    let output = RunOutput::resolve();
    let c_settings = SettingsHandle::build(settings, database_key, test_location);
    let mut failures = Vec::new();
    for blob in blobs {
        let run = RunHandle::start_blob(&c_settings, blob, output.sink());
        match drive_run(run, &mut test_fn, settings, reproduce_attribute) {
            Ok(reproduced) => failures.extend(reproduced),
            Err(message) => panic!("{message}"),
        }
    }
    if failures.is_empty() {
        panic!(
            "reproduce_failure: the supplied failure blob did not reproduce a \
             failure. The failure may have been fixed — or, for a nondeterministic \
             blob, it may not have recurred within the replay budget (a bug failing \
             10% of the time escapes it about 5% of the time). Re-run to retry, or \
             delete the attribute once the failure is fixed."
        );
    }
    raise_failures(failures, settings.verbosity == Verbosity::Quiet, &output);
}

#[cfg(test)]
#[path = "../tests/embedded/run_lifecycle_tests.rs"]
mod tests;
