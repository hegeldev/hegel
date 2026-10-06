use crate::{InternalError, InvalidArgument, SendPtr, check, last_error, with_ctx};
use hegel_c as c;
use hegel_c::hegel_result_t as Rc;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::ptr;

/// A handle onto an engine-side pretty-printing document.
#[pyclass(frozen, name = "Printer", module = "hegel._native")]
pub(crate) struct Printer {
    raw: SendPtr<c::HegelPrinter>,
}

impl Printer {
    pub(crate) fn from_raw(raw: *mut c::HegelPrinter) -> Self {
        Printer { raw: SendPtr(raw) }
    }
}

impl Drop for Printer {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_printer_free(ctx, self.raw.0);
        });
    }
}

/// Printer calls on a dead region (the document was already read, or the
/// region's speculative anchor was aborted) are silent no-ops, as in the
/// reference frontend; genuine misuse is reported.
fn tolerate(rc: Rc) -> PyResult<()> {
    match rc {
        Rc::HEGEL_OK | Rc::HEGEL_E_INVALID_HANDLE => Ok(()),
        Rc::HEGEL_E_INVALID_ARG => Err(InvalidArgument::new_err(last_error())),
        other => Err(InternalError::new_err(format!(
            "libhegel printer call returned {other:?}: {}",
            last_error()
        ))),
    }
}

pub(crate) fn with_options<R>(
    max_width: u64,
    f: impl FnOnce(*mut c::HegelContext, *const c::HegelPrinterOptions) -> PyResult<R>,
) -> PyResult<R> {
    with_ctx(|ctx| {
        let mut options: *mut c::HegelPrinterOptions = ptr::null_mut();
        check(unsafe { c::hegel_printer_options_new(ctx, &mut options) })?;
        let result = check(unsafe { c::hegel_printer_options_set_max_width(ctx, options, max_width) })
            .and_then(|()| f(ctx, options));
        let _ = unsafe { c::hegel_printer_options_free(ctx, options) };
        result
    })
}

#[pymethods]
impl Printer {
    #[staticmethod]
    fn new(max_width: u64) -> PyResult<Self> {
        let mut raw: *mut c::HegelPrinter = ptr::null_mut();
        with_options(max_width, |ctx, options| {
            check(unsafe { c::hegel_printer_new(ctx, options, &mut raw) })
        })?;
        Ok(Printer::from_raw(raw))
    }

    fn text(&self, s: &str) -> PyResult<()> {
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_text(ctx, self.raw.0, s.as_ptr(), s.len()) }))
    }

    fn if_break(&self, s: &str) -> PyResult<()> {
        with_ctx(|ctx| {
            tolerate(unsafe { c::hegel_printer_if_break(ctx, self.raw.0, s.as_ptr(), s.len()) })
        })
    }

    fn breakable(&self, sep: &str) -> PyResult<()> {
        with_ctx(|ctx| {
            tolerate(unsafe { c::hegel_printer_breakable(ctx, self.raw.0, sep.as_ptr(), sep.len()) })
        })
    }

    fn comment(&self, s: &str) -> PyResult<()> {
        with_ctx(|ctx| {
            tolerate(unsafe { c::hegel_printer_comment(ctx, self.raw.0, s.as_ptr(), s.len()) })
        })
    }

    fn hard_break(&self) -> PyResult<()> {
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_hard_break(ctx, self.raw.0) }))
    }

    fn begin_group(&self, indent: u64, open: &str) -> PyResult<()> {
        with_ctx(|ctx| {
            tolerate(unsafe {
                c::hegel_printer_begin_group(ctx, self.raw.0, indent, open.as_ptr(), open.len())
            })
        })
    }

    fn end_group(&self, close: &str) -> PyResult<()> {
        with_ctx(|ctx| {
            tolerate(unsafe {
                c::hegel_printer_end_group(ctx, self.raw.0, close.as_ptr(), close.len())
            })
        })
    }

    fn shift_indent(&self, delta: i64) -> PyResult<()> {
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_shift_indent(ctx, self.raw.0, delta) }))
    }

    fn deferred(&self) -> PyResult<Option<Printer>> {
        let mut raw: *mut c::HegelPrinter = ptr::null_mut();
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_deferred(ctx, self.raw.0, &mut raw) }))?;
        Ok((!raw.is_null()).then(|| Printer::from_raw(raw)))
    }

    fn resolve(&self) -> PyResult<()> {
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_resolve(ctx, self.raw.0) }))
    }

    fn begin_speculative(&self) -> PyResult<()> {
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_begin_speculative(ctx, self.raw.0) }))
    }

    fn commit_speculative(&self) -> PyResult<()> {
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_commit_speculative(ctx, self.raw.0) }))
    }

    fn abort_speculative(&self) -> PyResult<()> {
        with_ctx(|ctx| tolerate(unsafe { c::hegel_printer_abort_speculative(ctx, self.raw.0) }))
    }

    fn is_live(&self) -> PyResult<bool> {
        let mut live = false;
        with_ctx(|ctx| check(unsafe { c::hegel_printer_is_live(ctx, self.raw.0, &mut live) }))?;
        Ok(live)
    }

    /// Flush pending break points and read everything printed so far, as
    /// UTF-8 bytes (decode with `surrogatepass`).
    fn value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let mut result = c::hegel_printer_value_result_t {
            data: ptr::null_mut(),
            len: 0,
        };
        let bytes = with_ctx(|ctx| {
            check(unsafe { c::hegel_printer_value(ctx, self.raw.0, &mut result) })?;
            // SAFETY: on success data points to len bytes owned by us until
            // released with hegel_printer_value_result_free.
            let bytes = unsafe { std::slice::from_raw_parts(result.data as *const u8, result.len) }
                .to_vec();
            let _ = unsafe { c::hegel_printer_value_result_free(ctx, &mut result) };
            Ok::<_, PyErr>(bytes)
        })?;
        Ok(PyBytes::new(py, &bytes))
    }
}
