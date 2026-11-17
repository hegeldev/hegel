//! Shared helpers for the in-process C-ABI test binaries.

use hegel_c::hegel_result_t::*;
use hegel_c::{
    HegelContext, HegelRun, HegelSettings, HegelTestCase, hegel_context_last_error,
    hegel_mark_complete, hegel_next_test_case, hegel_output_callback_t, hegel_run_start,
    hegel_settings_new, hegel_settings_set_database, hegel_settings_set_verbosity, hegel_status_t,
    hegel_string_t, hegel_verbosity_t,
};
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::Mutex;

/// Assert a call that should always succeed in these tests returned `HEGEL_OK`.
#[allow(dead_code)]
pub fn ok(rc: hegel_c::hegel_result_t) {
    assert_eq!(rc, HEGEL_OK);
}

#[allow(dead_code)]
pub fn last_error(ctx: *const HegelContext) -> String {
    let p = unsafe { hegel_context_last_error(ctx) };
    assert!(!p.data.is_null());
    assert_eq!(unsafe { *p.data.add(p.len) }, 0);
    String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(p.data.cast(), p.len) })
        .into_owned()
}

#[allow(dead_code)]
pub unsafe fn make_settings(ctx: *mut HegelContext) -> *mut HegelSettings {
    let mut s: *mut HegelSettings = ptr::null_mut();
    assert_eq!(unsafe { hegel_settings_new(ctx, &mut s) }, HEGEL_OK);
    assert!(!s.is_null());
    s
}

/// Like [`make_settings`], but with the failure database disabled.
#[allow(dead_code)]
pub unsafe fn make_settings_no_db(ctx: *mut HegelContext) -> *mut HegelSettings {
    let s = unsafe { make_settings(ctx) };
    assert_eq!(
        unsafe { hegel_settings_set_database(ctx, s, &"".into()) },
        HEGEL_OK
    );
    s
}

#[allow(dead_code)]
pub unsafe fn start(ctx: *mut HegelContext, s: *const HegelSettings) -> *mut HegelRun {
    unsafe { start_with_output(ctx, s, None, ptr::null_mut()) }
}

/// Like [`start`], but routing the run's engine output to `callback` /
/// `user_data` (see `hegel_run_start`).
#[allow(dead_code)]
pub unsafe fn start_with_output(
    ctx: *mut HegelContext,
    s: *const HegelSettings,
    callback: hegel_output_callback_t,
    user_data: *mut c_void,
) -> *mut HegelRun {
    let mut run: *mut HegelRun = ptr::null_mut();
    assert_eq!(
        unsafe { hegel_run_start(ctx, s, callback, user_data, &mut run) },
        HEGEL_OK
    );
    assert!(!run.is_null());
    run
}

/// The next case the run hands out, or null at completion (`HEGEL_OK` + null).
#[allow(dead_code)]
pub unsafe fn next_case(ctx: *mut HegelContext, run: *mut HegelRun) -> *mut HegelTestCase {
    let mut tc: *mut HegelTestCase = ptr::null_mut();
    assert_eq!(unsafe { hegel_next_test_case(ctx, run, &mut tc) }, HEGEL_OK);
    tc
}

/// The lines the engine printed to a run's output callback.
#[allow(dead_code)]
#[derive(Default)]
pub struct Printed(Mutex<Vec<String>>);

#[allow(dead_code)]
impl Printed {
    /// The callback and user data that print into this collector.
    pub fn sink(&self) -> (hegel_output_callback_t, *mut c_void) {
        (
            Some(collect_line),
            ptr::from_ref(self).cast_mut().cast::<c_void>(),
        )
    }

    /// The lines printed since the last call, each ended by a newline.
    pub fn take(&self) -> String {
        std::mem::take(&mut *self.0.lock().unwrap())
            .into_iter()
            .map(|line| line + "\n")
            .collect()
    }
}

unsafe extern "C" fn collect_line(user_data: *mut c_void, line: *const c_char, len: usize) {
    let printed = unsafe { &*user_data.cast::<Printed>() };
    let bytes = unsafe { std::slice::from_raw_parts(line.cast::<u8>(), len) };
    printed
        .0
        .lock()
        .unwrap()
        .push(String::from_utf8_lossy(bytes).into_owned());
}

/// Start a verbose run printing into `printed` and return it with its
/// first test case. What the run printed before the test case started is
/// discarded, so `printed` then collects what the test case prints.
#[allow(dead_code)]
pub unsafe fn verbose_case(
    ctx: *mut HegelContext,
    s: *mut HegelSettings,
    printed: &Printed,
) -> (*mut HegelRun, *mut HegelTestCase) {
    assert_eq!(
        unsafe {
            hegel_settings_set_verbosity(ctx, s, hegel_verbosity_t::HEGEL_VERBOSITY_VERBOSE as u32)
        },
        HEGEL_OK
    );
    let (callback, user_data) = printed.sink();
    let run = unsafe { start_with_output(ctx, s, callback, user_data) };
    let tc = unsafe { next_case(ctx, run) };
    assert!(!tc.is_null());
    printed.take();
    (run, tc)
}

/// Complete `tc` with `status`, passing `origin` and a placeholder summary
/// and detail only when the status is `HEGEL_STATUS_INTERESTING`.
#[allow(dead_code)]
pub unsafe fn complete_case(
    ctx: *mut HegelContext,
    tc: *mut HegelTestCase,
    status: u32,
    origin: *const hegel_string_t,
) -> hegel_c::hegel_result_t {
    if status == hegel_status_t::HEGEL_STATUS_INTERESTING as u32 {
        let failed: hegel_string_t = "failed".into();
        unsafe { hegel_mark_complete(ctx, tc, status, origin, &failed, &failed) }
    } else {
        unsafe { hegel_mark_complete(ctx, tc, status, ptr::null(), ptr::null(), ptr::null()) }
    }
}
