use crate::printer::{Printer, with_options};
use crate::{
    InternalError, OutputBuffer, SendPtr, Settings, StringGenerator, big_from_le, big_to_le, check,
    cstr_to_string, cstring, output_trampoline, output_user_data, with_ctx,
};
use hegel_c as c;
use hegel_c::hegel_result_t as Rc;
use num_bigint::BigInt;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::ffi::c_char;
use std::ptr;
use std::sync::Arc;

/// One property-test run: the engine's generate/shrink loop, driven through
/// `next_test_case`.
#[pyclass(frozen, name = "Run", module = "hegel._native")]
pub(crate) struct Run {
    raw: SendPtr<c::HegelRun>,
    output: Arc<OutputBuffer>,
}

impl Drop for Run {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_run_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl Run {
    #[staticmethod]
    fn start(py: Python<'_>, settings: &Settings) -> PyResult<Self> {
        let output = Arc::new(OutputBuffer::default());
        let user_data = SendPtr(output_user_data(&output));
        let settings_ptr = SendPtr(settings.raw() as *mut c::HegelSettings);
        let mut raw: *mut c::HegelRun = ptr::null_mut();
        let raw_out = SendPtr(&mut raw as *mut *mut c::HegelRun);
        let rc = py.detach(move || {
            with_ctx(|ctx| unsafe {
                c::hegel_run_start(
                    ctx,
                    settings_ptr.get(),
                    Some(output_trampoline),
                    user_data.get(),
                    raw_out.get(),
                )
            })
        });
        let run = Run {
            raw: SendPtr(raw),
            output,
        };
        check(rc)?;
        Ok(run)
    }

    /// The next test case to drive, or `None` once the run is finished.
    fn next_test_case(&self, py: Python<'_>) -> PyResult<Option<TestCase>> {
        let run = self.raw;
        let mut raw: *mut c::HegelTestCase = ptr::null_mut();
        let raw_out = SendPtr(&mut raw as *mut *mut c::HegelTestCase);
        let rc = py.detach(move || {
            with_ctx(|ctx| unsafe { c::hegel_next_test_case(ctx, run.get(), raw_out.get()) })
        });
        check(rc)?;
        Ok((!raw.is_null()).then(|| TestCase {
            raw: SendPtr(raw),
            output: None,
        }))
    }

    /// Engine output lines emitted since the last drain.
    fn drain_output(&self) -> Vec<String> {
        self.output.drain()
    }

    fn result(&self, py: Python<'_>) -> PyResult<RunResult> {
        let run = self.raw;
        let mut raw: *mut c::HegelRunResult = ptr::null_mut();
        let raw_out = SendPtr(&mut raw as *mut *mut c::HegelRunResult);
        let rc = py.detach(move || {
            with_ctx(|ctx| unsafe { c::hegel_run_result(ctx, run.get(), raw_out.get()) })
        });
        check(rc)?;
        Ok(RunResult { raw: SendPtr(raw) })
    }
}

/// The outcome of a finished run.
#[pyclass(frozen, name = "RunResult", module = "hegel._native")]
pub(crate) struct RunResult {
    raw: SendPtr<c::HegelRunResult>,
}

impl Drop for RunResult {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_run_result_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl RunResult {
    fn status(&self) -> PyResult<u32> {
        let mut status = c::hegel_run_status_t::HEGEL_RUN_STATUS_ERROR;
        with_ctx(|ctx| check(unsafe { c::hegel_run_result_status(ctx, self.raw.0, &mut status) }))?;
        Ok(status as u32)
    }

    fn error(&self) -> PyResult<Option<String>> {
        let mut p: *const c_char = ptr::null();
        with_ctx(|ctx| check(unsafe { c::hegel_run_result_error(ctx, self.raw.0, &mut p) }))?;
        Ok(cstr_to_string(p))
    }

    fn failure_count(&self) -> PyResult<usize> {
        let mut count = 0usize;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_run_result_failure_count(ctx, self.raw.0, &mut count) })
        })?;
        Ok(count)
    }

    /// The `index`-th distinct failure as `(origin, reproduction_blob)`.
    fn failure(&self, index: usize) -> PyResult<(String, Option<String>)> {
        let mut f: *mut c::HegelFailure = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe { c::hegel_run_result_failure(ctx, self.raw.0, index, &mut f) })?;
            let mut origin: *const c_char = ptr::null();
            let mut blob: *const c_char = ptr::null();
            let result = check(unsafe { c::hegel_failure_origin(ctx, f, &mut origin) })
                .and_then(|()| {
                    check(unsafe { c::hegel_failure_reproduction_blob(ctx, f, &mut blob) })
                })
                .map(|()| {
                    (
                        cstr_to_string(origin).unwrap_or_default(),
                        cstr_to_string(blob),
                    )
                });
            let _ = unsafe { c::hegel_failure_free(ctx, f) };
            result
        })
    }
}

/// A handle onto one test case. Clones share the test case but own
/// independent choice streams; block handles share the stream and nest
/// their printed output.
#[pyclass(frozen, name = "TestCase", module = "hegel._native")]
pub(crate) struct TestCase {
    raw: SendPtr<c::HegelTestCase>,
    /// Present only for a blob replay, which has no run to drain from.
    output: Option<Arc<OutputBuffer>>,
}

impl Drop for TestCase {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_test_case_free(ctx, self.raw.0);
        });
    }
}

impl TestCase {
    fn tc(&self) -> *mut c::HegelTestCase {
        self.raw.0
    }
}

fn date_to_c(d: (i32, u8, u8)) -> c::hegel_date_t {
    c::hegel_date_t {
        year: d.0,
        month: d.1,
        day: d.2,
    }
}

fn date_from_c(d: c::hegel_date_t) -> (i32, u8, u8) {
    (d.year, d.month, d.day)
}

fn time_to_c(t: (u8, u8, u8, u32)) -> c::hegel_time_t {
    c::hegel_time_t {
        hour: t.0,
        minute: t.1,
        second: t.2,
        nanosecond: t.3,
    }
}

fn time_from_c(t: c::hegel_time_t) -> (u8, u8, u8, u32) {
    (t.hour, t.minute, t.second, t.nanosecond)
}

type PyDateTime = (i32, u8, u8, u8, u8, u8, u32);

fn datetime_to_c(d: PyDateTime) -> c::hegel_datetime_t {
    c::hegel_datetime_t {
        date: date_to_c((d.0, d.1, d.2)),
        time: time_to_c((d.3, d.4, d.5, d.6)),
    }
}

fn datetime_from_c(d: c::hegel_datetime_t) -> PyDateTime {
    (
        d.date.year,
        d.date.month,
        d.date.day,
        d.time.hour,
        d.time.minute,
        d.time.second,
        d.time.nanosecond,
    )
}

#[pymethods]
impl TestCase {
    /// Replay a reproduction blob as a standalone test case.
    #[staticmethod]
    fn from_blob(py: Python<'_>, settings: &Settings, blob: &str) -> PyResult<Self> {
        let output = Arc::new(OutputBuffer::default());
        let user_data = SendPtr(output_user_data(&output));
        let settings_ptr = SendPtr(settings.raw() as *mut c::HegelSettings);
        let c_blob = cstring(blob);
        let blob_ptr = SendPtr(c_blob.as_ptr() as *mut c_char);
        let mut raw: *mut c::HegelTestCase = ptr::null_mut();
        let raw_out = SendPtr(&mut raw as *mut *mut c::HegelTestCase);
        let rc = py.detach(move || {
            with_ctx(|ctx| unsafe {
                c::hegel_test_case_from_blob(
                    ctx,
                    settings_ptr.get(),
                    blob_ptr.get(),
                    Some(output_trampoline),
                    user_data.get(),
                    raw_out.get(),
                )
            })
        });
        check(rc)?;
        Ok(TestCase {
            raw: SendPtr(raw),
            output: Some(output),
        })
    }

    fn drain_output(&self) -> Vec<String> {
        self.output.as_ref().map(|o| o.drain()).unwrap_or_default()
    }

    /// A handle with an independent choice stream, for another thread.
    fn clone(&self) -> PyResult<TestCase> {
        let mut raw: *mut c::HegelTestCase = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_test_case_clone(ctx, self.tc(), &mut raw) }))?;
        Ok(TestCase {
            raw: SendPtr(raw),
            output: self.output.clone(),
        })
    }

    /// A handle sharing this one's choice stream whose printed output is a
    /// nested block indented `indent` further.
    fn block(&self, indent: u64) -> PyResult<TestCase> {
        let mut raw: *mut c::HegelTestCase = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe { c::hegel_test_case_block(ctx, self.tc(), indent, &mut raw) })
        })?;
        Ok(TestCase {
            raw: SendPtr(raw),
            output: self.output.clone(),
        })
    }

    fn set_worker(&self, worker_index: i64) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe { c::hegel_test_case_set_worker(ctx, self.tc(), worker_index) })
        })
    }

    fn is_nondeterministic(&self) -> PyResult<bool> {
        let mut out = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_test_case_is_nondeterministic(ctx, self.tc(), &mut out) })
        })?;
        Ok(out)
    }

    #[pyo3(signature = (status, origin))]
    fn mark_complete(&self, py: Python<'_>, status: u32, origin: Option<&str>) -> PyResult<()> {
        let c_origin = origin.map(cstring);
        let origin_ptr =
            SendPtr(c_origin.as_ref().map_or(ptr::null(), |s| s.as_ptr()) as *mut c_char);
        let tc = self.raw;
        let rc = py.detach(move || {
            with_ctx(|ctx| unsafe {
                c::hegel_mark_complete(ctx, tc.get(), status, origin_ptr.get())
            })
        });
        check(rc)
    }

    #[pyo3(signature = (p, forced=None))]
    fn generate_boolean(&self, p: f64, forced: Option<bool>) -> PyResult<bool> {
        let mut out = false;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_boolean(
                    ctx,
                    self.tc(),
                    p,
                    forced.unwrap_or(false),
                    forced.is_some(),
                    &mut out,
                )
            })
        })?;
        Ok(out)
    }

    /// Draw an integer in `[min_value, max_value]`, of any magnitude.
    fn generate_integer(&self, min_value: BigInt, max_value: BigInt) -> PyResult<BigInt> {
        if let (Ok(lo), Ok(hi)) = (i64::try_from(&min_value), i64::try_from(&max_value)) {
            let mut out = 0i64;
            with_ctx(|ctx| {
                check(unsafe { c::hegel_generate_integer(ctx, self.tc(), lo, hi, &mut out) })
            })?;
            return Ok(BigInt::from(out));
        }
        let lo = big_to_le(&min_value);
        let hi = big_to_le(&max_value);
        let mut out = vec![0u8; lo.len().max(hi.len()) + 1];
        let mut out_len = 0usize;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_integer_big(
                    ctx,
                    self.tc(),
                    lo.as_ptr(),
                    lo.len(),
                    hi.as_ptr(),
                    hi.len(),
                    out.as_mut_ptr(),
                    out.len(),
                    &mut out_len,
                )
            })
        })?;
        if out_len > out.len() {
            return Err(InternalError::new_err(
                "hegel_generate_integer_big wrote more bytes than its buffer holds",
            ));
        }
        Ok(big_from_le(&out[..out_len]))
    }

    fn generate_float(
        &self,
        width: u32,
        min_value: f64,
        max_value: f64,
        allow_nan: bool,
        allow_infinity: bool,
        exclude_min: bool,
        exclude_max: bool,
        smallest_nonzero_magnitude: f64,
    ) -> PyResult<f64> {
        let mut out = 0f64;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_float(
                    ctx,
                    self.tc(),
                    width,
                    min_value,
                    max_value,
                    allow_nan,
                    allow_infinity,
                    exclude_min,
                    exclude_max,
                    smallest_nonzero_magnitude,
                    &mut out,
                )
            })
        })?;
        Ok(out)
    }

    fn generate_bytes<'py>(
        &self,
        py: Python<'py>,
        min_size: u64,
        max_size: u64,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let mut result = c::hegel_generate_bytes_result_t {
            data: ptr::null_mut(),
            len: 0,
        };
        let bytes = with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_bytes(ctx, self.tc(), min_size, max_size, &mut result)
            })?;
            // SAFETY: on success data points to len bytes owned by us until
            // released with the matching free.
            let bytes = unsafe { std::slice::from_raw_parts(result.data, result.len) }.to_vec();
            let _ = unsafe { c::hegel_generate_bytes_result_free(ctx, &mut result) };
            Ok::<_, PyErr>(bytes)
        })?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Draw a string as UTF-8 (WTF-8 for surrogates) bytes; decode in Python
    /// with `surrogatepass`.
    fn generate_string<'py>(
        &self,
        py: Python<'py>,
        generator: &StringGenerator,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let mut result = c::hegel_generate_string_result_t {
            data: ptr::null_mut(),
            len: 0,
        };
        let bytes = with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_string(ctx, self.tc(), generator.raw(), &mut result)
            })?;
            // SAFETY: as for generate_bytes.
            let bytes = unsafe { std::slice::from_raw_parts(result.data as *const u8, result.len) }
                .to_vec();
            let _ = unsafe { c::hegel_generate_string_result_free(ctx, &mut result) };
            Ok::<_, PyErr>(bytes)
        })?;
        Ok(PyBytes::new(py, &bytes))
    }

    fn generate_date(
        &self,
        min_value: (i32, u8, u8),
        max_value: (i32, u8, u8),
    ) -> PyResult<(i32, u8, u8)> {
        let mut out = date_to_c(min_value);
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_date(
                    ctx,
                    self.tc(),
                    date_to_c(min_value),
                    date_to_c(max_value),
                    &mut out,
                )
            })
        })?;
        Ok(date_from_c(out))
    }

    fn generate_time(
        &self,
        min_value: (u8, u8, u8, u32),
        max_value: (u8, u8, u8, u32),
    ) -> PyResult<(u8, u8, u8, u32)> {
        let mut out = time_to_c(min_value);
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_time(
                    ctx,
                    self.tc(),
                    time_to_c(min_value),
                    time_to_c(max_value),
                    &mut out,
                )
            })
        })?;
        Ok(time_from_c(out))
    }

    fn generate_datetime(
        &self,
        min_value: PyDateTime,
        max_value: PyDateTime,
    ) -> PyResult<PyDateTime> {
        let mut out = datetime_to_c(min_value);
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_datetime(
                    ctx,
                    self.tc(),
                    datetime_to_c(min_value),
                    datetime_to_c(max_value),
                    &mut out,
                )
            })
        })?;
        Ok(datetime_from_c(out))
    }

    #[pyo3(signature = (version))]
    fn generate_uuid<'py>(
        &self,
        py: Python<'py>,
        version: Option<u8>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let mut out = [0u8; 16];
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_generate_uuid(
                    ctx,
                    self.tc(),
                    version.unwrap_or(0),
                    version.is_some(),
                    out.as_mut_ptr(),
                )
            })
        })?;
        Ok(PyBytes::new(py, &out))
    }

    fn generate_ipv4<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let mut out = [0u8; 4];
        with_ctx(|ctx| check(unsafe { c::hegel_generate_ipv4(ctx, self.tc(), out.as_mut_ptr()) }))?;
        Ok(PyBytes::new(py, &out))
    }

    fn generate_ipv6<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let mut out = [0u8; 16];
        with_ctx(|ctx| check(unsafe { c::hegel_generate_ipv6(ctx, self.tc(), out.as_mut_ptr()) }))?;
        Ok(PyBytes::new(py, &out))
    }

    fn target(&self, value: f64, label: &str) -> PyResult<()> {
        let c_label = cstring(label);
        with_ctx(|ctx| check(unsafe { c::hegel_target(ctx, self.tc(), value, c_label.as_ptr()) }))
    }

    fn event(&self, label: &str) -> PyResult<()> {
        let c_label = cstring(label);
        with_ctx(|ctx| check(unsafe { c::hegel_event(ctx, self.tc(), c_label.as_ptr()) }))
    }

    fn event_value(&self, value: f64, label: &str) -> PyResult<()> {
        let c_label = cstring(label);
        with_ctx(|ctx| {
            check(unsafe { c::hegel_event_value(ctx, self.tc(), value, c_label.as_ptr()) })
        })
    }

    fn start_span(&self, label: u64) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_start_span(ctx, self.tc(), label) }))
    }

    fn stop_span(&self, discard: bool) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_stop_span(ctx, self.tc(), discard) }))
    }

    /// Append whole lines of text to this handle's print region. A dead
    /// region (the document was already read) is a silent no-op.
    fn note(&self, text: &str) -> PyResult<()> {
        with_ctx(
            |ctx| match unsafe { c::hegel_note(ctx, self.tc(), text.as_ptr(), text.len()) } {
                Rc::HEGEL_E_INVALID_HANDLE => Ok(()),
                rc => check(rc),
            },
        )
    }

    /// A printer onto this handle's region of the test case's document.
    fn printer(&self, max_width: u64) -> PyResult<Printer> {
        let mut raw: *mut c::HegelPrinter = ptr::null_mut();
        with_options(max_width, |ctx, options| {
            check(unsafe { c::hegel_test_case_printer(ctx, self.tc(), options, &mut raw) })
        })?;
        Ok(Printer::from_raw(raw))
    }

    #[pyo3(signature = (min_size, max_size))]
    fn new_collection(&self, min_size: u64, max_size: Option<u64>) -> PyResult<Collection> {
        let mut raw: *mut c::HegelCollection = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_new_collection(
                    ctx,
                    self.tc(),
                    min_size,
                    max_size.unwrap_or(u64::MAX),
                    &mut raw,
                )
            })
        })?;
        Ok(Collection { raw: SendPtr(raw) })
    }

    fn new_recursion(&self, max_depth: u64, max_leaves: u64) -> PyResult<Recursion> {
        let mut raw: *mut c::HegelRecursion = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_new_recursion(ctx, self.tc(), max_depth, max_leaves, &mut raw)
            })
        })?;
        Ok(Recursion { raw: SendPtr(raw) })
    }

    fn new_pool(&self) -> PyResult<Pool> {
        let mut raw: *mut c::HegelPool = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_new_pool(ctx, self.tc(), &mut raw) }))?;
        Ok(Pool { raw: SendPtr(raw) })
    }

    /// Register a state machine; returns `(machine, concurrency)`.
    #[pyo3(signature = (rule_names, rule_groups, rule_weights, invariant_names, invariant_always_check, min_concurrency, max_concurrency, step_count))]
    fn new_state_machine(
        &self,
        rule_names: Vec<String>,
        rule_groups: Vec<i64>,
        rule_weights: Option<Vec<f64>>,
        invariant_names: Vec<String>,
        invariant_always_check: Option<Vec<bool>>,
        min_concurrency: i64,
        max_concurrency: i64,
        step_count: i64,
    ) -> PyResult<(StateMachine, i64)> {
        let rule_cs: Vec<_> = rule_names.iter().map(|s| cstring(s)).collect();
        let inv_cs: Vec<_> = invariant_names.iter().map(|s| cstring(s)).collect();
        let rule_ptrs: Vec<*const c_char> = rule_cs.iter().map(|s| s.as_ptr()).collect();
        let inv_ptrs: Vec<*const c_char> = inv_cs.iter().map(|s| s.as_ptr()).collect();
        let mut raw: *mut c::HegelStateMachine = ptr::null_mut();
        let mut concurrency = 0i64;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_new_state_machine(
                    ctx,
                    self.tc(),
                    rule_ptrs.as_ptr(),
                    rule_groups.as_ptr(),
                    rule_weights.as_ref().map_or(ptr::null(), |w| w.as_ptr()),
                    rule_ptrs.len(),
                    inv_ptrs.as_ptr(),
                    invariant_always_check
                        .as_ref()
                        .map_or(ptr::null(), |a| a.as_ptr()),
                    inv_ptrs.len(),
                    min_concurrency,
                    max_concurrency,
                    step_count,
                    &mut raw,
                    &mut concurrency,
                )
            })
        })?;
        Ok((StateMachine { raw: SendPtr(raw) }, concurrency))
    }
}

#[pyclass(frozen, name = "Collection", module = "hegel._native")]
pub(crate) struct Collection {
    raw: SendPtr<c::HegelCollection>,
}

impl Drop for Collection {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_collection_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl Collection {
    fn more(&self, tc: &TestCase) -> PyResult<bool> {
        let mut out = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_collection_more(ctx, tc.tc(), self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    #[pyo3(signature = (tc, why))]
    fn reject(&self, tc: &TestCase, why: Option<&str>) -> PyResult<()> {
        let c_why = why.map(cstring);
        let p = c_why.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        with_ctx(|ctx| check(unsafe { c::hegel_collection_reject(ctx, tc.tc(), self.raw.0, p) }))
    }
}

#[pyclass(frozen, name = "Recursion", module = "hegel._native")]
pub(crate) struct Recursion {
    raw: SendPtr<c::HegelRecursion>,
}

impl Drop for Recursion {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_recursion_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl Recursion {
    fn branch(&self, tc: &TestCase, depth: u64) -> PyResult<bool> {
        let mut out = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_recursion_branch(ctx, tc.tc(), self.raw.0, depth, &mut out) })
        })?;
        Ok(out)
    }

    /// Raises `RetrySignal` when the leaf budget is exceeded.
    fn leaf(&self, tc: &TestCase) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_recursion_leaf(ctx, tc.tc(), self.raw.0) }))
    }

    /// Raises `AssumeFailed` once the retry budget is exhausted.
    fn retry(&self, tc: &TestCase) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_recursion_retry(ctx, tc.tc(), self.raw.0) }))
    }

    /// Raises `RetrySignal` when the finished attempt was mispriced.
    fn finish(&self, tc: &TestCase) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_recursion_finish(ctx, tc.tc(), self.raw.0) }))
    }
}

#[pyclass(frozen, name = "Pool", module = "hegel._native")]
pub(crate) struct Pool {
    raw: SendPtr<c::HegelPool>,
}

impl Drop for Pool {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_pool_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl Pool {
    fn add(&self, tc: &TestCase) -> PyResult<i64> {
        let mut out = 0i64;
        with_ctx(|ctx| check(unsafe { c::hegel_pool_add(ctx, tc.tc(), self.raw.0, &mut out) }))?;
        Ok(out)
    }

    /// Raises `AssumeFailed` when the pool is empty.
    fn generate(&self, tc: &TestCase, consume: bool) -> PyResult<i64> {
        let mut out = 0i64;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_pool_generate(ctx, tc.tc(), self.raw.0, consume, &mut out) })
        })?;
        Ok(out)
    }
}

#[pyclass(frozen, name = "StateMachine", module = "hegel._native")]
pub(crate) struct StateMachine {
    raw: SendPtr<c::HegelStateMachine>,
}

impl Drop for StateMachine {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_state_machine_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl StateMachine {
    /// The next round's group id, or `None` once the machine is done.
    fn next_group(&self, tc: &TestCase) -> PyResult<Option<i64>> {
        let mut out = 0i64;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_state_machine_next_group(ctx, tc.tc(), self.raw.0, &mut out) })
        })?;
        Ok((out != c::HEGEL_STATE_MACHINE_DONE).then_some(out))
    }

    /// The next rule index for `worker_index`, or `None` at the join point.
    fn next_rule(&self, tc: &TestCase, worker_index: i64) -> PyResult<Option<i64>> {
        let mut out = 0i64;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_state_machine_next_rule(ctx, tc.tc(), self.raw.0, worker_index, &mut out)
            })
        })?;
        Ok((out != c::HEGEL_STATE_MACHINE_DONE).then_some(out))
    }

    fn rule_rejected(&self, tc: &TestCase, worker_index: i64) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_state_machine_rule_rejected(ctx, tc.tc(), self.raw.0, worker_index)
            })
        })
    }

    fn should_check_invariant(&self, tc: &TestCase, invariant_index: i64) -> PyResult<bool> {
        let mut out = false;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_state_machine_should_check_invariant(
                    ctx,
                    tc.tc(),
                    self.raw.0,
                    invariant_index,
                    &mut out,
                )
            })
        })?;
        Ok(out)
    }
}
