//! pyo3 bindings over the libhegel C ABI (the `hegeltest-c` crate), exposed
//! to Python as `hegel._native`. This layer is deliberately thin: it owns
//! handles, converts arguments, maps result codes to exceptions, and leaves
//! every policy decision to the Python package.

#![allow(clippy::too_many_arguments)]

use hegel_c as c;
use hegel_c::hegel_result_t as Rc;
use num_bigint::BigInt;
use pyo3::create_exception;
use pyo3::exceptions::{PyBaseException, PyException};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::{Arc, Mutex};

mod printer;
mod settings;
mod strings;
mod test_case;

pub(crate) use printer::Printer;
pub(crate) use settings::Settings;
pub(crate) use strings::StringGenerator;
pub(crate) use test_case::{Collection, Pool, Recursion, Run, RunResult, StateMachine, TestCase};

create_exception!(
    hegel._native,
    HegelError,
    PyException,
    "Base class for Hegel errors."
);
create_exception!(
    hegel._native,
    InvalidArgument,
    HegelError,
    "An argument passed to Hegel was invalid."
);
create_exception!(
    hegel._native,
    BackendError,
    HegelError,
    "The Hegel engine reported an error."
);
create_exception!(
    hegel._native,
    InternalError,
    HegelError,
    "An internal error in Hegel. Please report this as a bug."
);
create_exception!(
    hegel._native,
    StopTest,
    PyBaseException,
    "Control flow: the engine abandoned the current test case."
);
create_exception!(
    hegel._native,
    AssumeFailed,
    PyBaseException,
    "Control flow: the current test case was rejected by an assumption."
);
create_exception!(
    hegel._native,
    RetrySignal,
    PyBaseException,
    "Control flow: a recursive generation attempt must be retried."
);

struct ThreadContext(*mut c::HegelContext);

impl Drop for ThreadContext {
    fn drop(&mut self) {
        // SAFETY: the pointer came from hegel_context_new and is freed once.
        let _ = unsafe { c::hegel_context_free(self.0) };
    }
}

thread_local! {
    static CONTEXT: ThreadContext = ThreadContext(c::hegel_context_new());
}

/// Run `f` with this thread's libhegel context.
pub(crate) fn with_ctx<R>(f: impl FnOnce(*mut c::HegelContext) -> R) -> R {
    CONTEXT.with(|ctx| f(ctx.0))
}

pub(crate) fn last_error() -> String {
    with_ctx(|ctx| {
        // SAFETY: ctx is a live context; the returned pointer is valid until
        // the next call on this context, and we copy it out immediately.
        let p = unsafe { c::hegel_context_last_error(ctx) };
        cstr_to_string(p).unwrap_or_default()
    })
}

pub(crate) fn cstr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        // SAFETY: the engine guarantees a NUL-terminated string.
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }
}

pub(crate) fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', "\u{fffd}")).expect("NUL bytes were replaced")
}

/// Map a libhegel result code to `Ok(())` or the matching Python exception.
pub(crate) fn check(rc: Rc) -> PyResult<()> {
    match rc {
        Rc::HEGEL_OK => Ok(()),
        Rc::HEGEL_E_STOP_TEST => Err(StopTest::new_err(())),
        Rc::HEGEL_E_ASSUME => Err(AssumeFailed::new_err(())),
        Rc::HEGEL_E_RETRY => Err(RetrySignal::new_err(())),
        Rc::HEGEL_E_INVALID_ARG => Err(InvalidArgument::new_err(last_error())),
        Rc::HEGEL_E_BACKEND => Err(BackendError::new_err(last_error())),
        // A test body can't see its own test case finish, so this means a
        // TestCase (or a pool or collection holding one) outlived its test.
        Rc::HEGEL_E_ALREADY_COMPLETE => Err(InvalidArgument::new_err(
            "this test case has already finished: a TestCase can only be used while \
             the test body it was passed to is running. Was it kept for later, or \
             used from a thread that outlived the test? Join any thread that draws \
             before the test returns.",
        )),
        other => Err(InternalError::new_err(format!(
            "libhegel returned {other:?}: {}",
            last_error()
        ))),
    }
}

/// A raw pointer that may cross a `Python::detach` boundary. The engine
/// serialises concurrent use of every handle itself.
pub(crate) struct SendPtr<T>(pub(crate) *mut T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

impl<T> Clone for SendPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SendPtr<T> {}

impl<T> SendPtr<T> {
    /// Read the pointer through the wrapper, so a closure captures the
    /// wrapper (which is `Send`) rather than the raw pointer field.
    pub(crate) fn get(&self) -> *mut T {
        self.0
    }
}

/// Lines emitted by the engine through its output callback, drained by the
/// Python layer after each engine call.
#[derive(Default)]
pub(crate) struct OutputBuffer(Mutex<Vec<String>>);

impl OutputBuffer {
    pub(crate) fn push(&self, line: String) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(line);
    }

    pub(crate) fn drain(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

pub(crate) unsafe extern "C" fn output_trampoline(
    user_data: *mut c_void,
    line: *const c_char,
    len: usize,
) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: user_data is the Arc<OutputBuffer> pointer registered with the
    // run (kept alive until the run or replayed test case is freed); line is
    // valid for len bytes for the duration of the call.
    let buffer = unsafe { &*(user_data as *const OutputBuffer) };
    let bytes = if line.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(line as *const u8, len) }
    };
    buffer.push(String::from_utf8_lossy(bytes).into_owned());
}

pub(crate) fn output_user_data(buffer: &Arc<OutputBuffer>) -> *mut c_void {
    Arc::as_ptr(buffer) as *mut c_void
}

pub(crate) fn big_to_le(value: &BigInt) -> Vec<u8> {
    value.to_signed_bytes_le()
}

pub(crate) fn big_from_le(bytes: &[u8]) -> BigInt {
    BigInt::from_signed_bytes_le(bytes)
}

#[pyfunction]
fn version() -> PyResult<String> {
    let mut p: *const c_char = ptr::null();
    with_ctx(|ctx| check(unsafe { c::hegel_version(ctx, &mut p) }))?;
    Ok(cstr_to_string(p).unwrap_or_default())
}

#[pyfunction]
fn label_from_name(name: &str) -> PyResult<u64> {
    let c_name = cstring(name);
    let mut out = 0u64;
    with_ctx(|ctx| check(unsafe { c::hegel_label_from_name(ctx, c_name.as_ptr(), &mut out) }))?;
    Ok(out)
}

#[pyfunction]
fn label_combine(labels: Vec<u64>) -> PyResult<u64> {
    let mut out = 0u64;
    with_ctx(|ctx| {
        check(unsafe { c::hegel_label_combine(ctx, labels.as_ptr(), labels.len(), &mut out) })
    })?;
    Ok(out)
}

#[pyfunction]
#[pyo3(signature = (name))]
fn set_default_profile(name: Option<&str>) -> PyResult<()> {
    let c_name = name.map(cstring);
    let ptr = c_name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
    with_ctx(|ctx| check(unsafe { c::hegel_set_default_profile(ctx, ptr) }))
}

#[pyfunction]
fn register_profile(name: &str, settings: &Settings) -> PyResult<()> {
    let c_name = cstring(name);
    with_ctx(|ctx| {
        check(unsafe { c::hegel_settings_register_profile(ctx, c_name.as_ptr(), settings.raw()) })
    })
}

#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("HegelError", py.get_type::<HegelError>())?;
    m.add("InvalidArgument", py.get_type::<InvalidArgument>())?;
    m.add("BackendError", py.get_type::<BackendError>())?;
    m.add("InternalError", py.get_type::<InternalError>())?;
    m.add("StopTest", py.get_type::<StopTest>())?;
    m.add("AssumeFailed", py.get_type::<AssumeFailed>())?;
    m.add("RetrySignal", py.get_type::<RetrySignal>())?;
    m.add_class::<Settings>()?;
    m.add_class::<Run>()?;
    m.add_class::<RunResult>()?;
    m.add_class::<TestCase>()?;
    m.add_class::<Collection>()?;
    m.add_class::<Recursion>()?;
    m.add_class::<Pool>()?;
    m.add_class::<StateMachine>()?;
    m.add_class::<StringGenerator>()?;
    m.add_class::<Printer>()?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(label_from_name, m)?)?;
    m.add_function(wrap_pyfunction!(label_combine, m)?)?;
    m.add_function(wrap_pyfunction!(set_default_profile, m)?)?;
    m.add_function(wrap_pyfunction!(register_profile, m)?)?;
    m.add("STATUS_VALID", c::hegel_status_t::HEGEL_STATUS_VALID as u32)?;
    m.add(
        "STATUS_INVALID",
        c::hegel_status_t::HEGEL_STATUS_INVALID as u32,
    )?;
    m.add(
        "STATUS_OVERRUN",
        c::hegel_status_t::HEGEL_STATUS_OVERRUN as u32,
    )?;
    m.add(
        "STATUS_INTERESTING",
        c::hegel_status_t::HEGEL_STATUS_INTERESTING as u32,
    )?;
    m.add(
        "RUN_STATUS_PASSED",
        c::hegel_run_status_t::HEGEL_RUN_STATUS_PASSED as u32,
    )?;
    m.add(
        "RUN_STATUS_FAILED",
        c::hegel_run_status_t::HEGEL_RUN_STATUS_FAILED as u32,
    )?;
    m.add(
        "RUN_STATUS_ERROR",
        c::hegel_run_status_t::HEGEL_RUN_STATUS_ERROR as u32,
    )?;
    m.add(
        "NONDETERMINISM_QUIET",
        c::hegel_nondeterminism_strictness_t::HEGEL_NONDETERMINISM_QUIET as u32,
    )?;
    m.add(
        "NONDETERMINISM_WARN",
        c::hegel_nondeterminism_strictness_t::HEGEL_NONDETERMINISM_WARN as u32,
    )?;
    m.add(
        "NONDETERMINISM_ERROR",
        c::hegel_nondeterminism_strictness_t::HEGEL_NONDETERMINISM_ERROR as u32,
    )?;
    m.add(
        "VERBOSITY_QUIET",
        c::hegel_verbosity_t::HEGEL_VERBOSITY_QUIET as u32,
    )?;
    m.add(
        "VERBOSITY_NORMAL",
        c::hegel_verbosity_t::HEGEL_VERBOSITY_NORMAL as u32,
    )?;
    m.add(
        "VERBOSITY_VERBOSE",
        c::hegel_verbosity_t::HEGEL_VERBOSITY_VERBOSE as u32,
    )?;
    m.add(
        "VERBOSITY_DEBUG",
        c::hegel_verbosity_t::HEGEL_VERBOSITY_DEBUG as u32,
    )?;
    m.add(
        "BACKEND_DEFAULT",
        c::hegel_backend_t::HEGEL_BACKEND_DEFAULT as u32,
    )?;
    m.add(
        "BACKEND_URANDOM",
        c::hegel_backend_t::HEGEL_BACKEND_URANDOM as u32,
    )?;
    m.add(
        "PHASE_EXPLICIT",
        c::hegel_phase_t::HEGEL_PHASE_EXPLICIT as u32,
    )?;
    m.add("PHASE_REUSE", c::hegel_phase_t::HEGEL_PHASE_REUSE as u32)?;
    m.add(
        "PHASE_GENERATE",
        c::hegel_phase_t::HEGEL_PHASE_GENERATE as u32,
    )?;
    m.add("PHASE_TARGET", c::hegel_phase_t::HEGEL_PHASE_TARGET as u32)?;
    m.add("PHASE_SHRINK", c::hegel_phase_t::HEGEL_PHASE_SHRINK as u32)?;
    m.add(
        "HC_FILTER_TOO_MUCH",
        c::hegel_health_check_t::HEGEL_HC_FILTER_TOO_MUCH as u32,
    )?;
    m.add(
        "HC_TOO_SLOW",
        c::hegel_health_check_t::HEGEL_HC_TOO_SLOW as u32,
    )?;
    m.add(
        "HC_TEST_CASES_TOO_LARGE",
        c::hegel_health_check_t::HEGEL_HC_TEST_CASES_TOO_LARGE as u32,
    )?;
    m.add(
        "HC_LARGE_INITIAL_TEST_CASE",
        c::hegel_health_check_t::HEGEL_HC_LARGE_INITIAL_TEST_CASE as u32,
    )?;
    Ok(())
}
