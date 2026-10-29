use crate::{SendPtr, StringGenerator, TestCase, check, hstr, hstrs, string_from, with_ctx};
use hegel_c as c;
use hegel_c::hegel_result_t as Rc;
use pyo3::prelude::*;
use std::ptr;

/// An owned libhegel grammar builder: a grammar's text and the files its
/// `%import`s need, supplied by the Python layer.
#[pyclass(frozen, name = "GrammarBuilder", module = "hegel._native")]
pub(crate) struct GrammarBuilder {
    raw: SendPtr<c::HegelGrammarBuilder>,
}

impl Drop for GrammarBuilder {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_grammar_builder_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl GrammarBuilder {
    #[new]
    fn new(text: &str) -> PyResult<Self> {
        let mut raw: *mut c::HegelGrammarBuilder = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_grammar_builder_new(ctx, hstr(text), &mut raw) }))?;
        Ok(GrammarBuilder { raw: SendPtr(raw) })
    }

    /// The path of the `index`th file the grammar needs, or `None` past
    /// the end of the list.
    fn file(&self, index: usize) -> PyResult<Option<String>> {
        let mut path = c::hegel_string_t::default();
        let rc = with_ctx(|ctx| unsafe {
            c::hegel_grammar_builder_file(ctx, self.raw.0, index, &mut path)
        });
        if rc == Rc::HEGEL_E_NOT_FOUND {
            return Ok(None);
        }
        check(rc)?;
        Ok(Some(string_from(path)))
    }

    /// Supply the `index`th file's text, or `None` for a missing file.
    #[pyo3(signature = (index, text))]
    fn provide(&self, index: usize, text: Option<&str>) -> PyResult<()> {
        let text = text.map(hstr);
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_grammar_builder_provide(
                    ctx,
                    self.raw.0,
                    index,
                    text.as_ref().map_or(ptr::null(), |t| t as *const _),
                )
            })
        })
    }
}

/// An owned compiled grammar. Immutable, so it may be shared across test
/// cases and threads.
#[pyclass(frozen, name = "Grammar", module = "hegel._native")]
pub(crate) struct Grammar {
    raw: SendPtr<c::HegelGrammar>,
}

impl Drop for Grammar {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_grammar_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl Grammar {
    #[new]
    #[pyo3(signature = (builder, start, max_size, max_depth, alphabet, explicit))]
    fn new(
        builder: &GrammarBuilder,
        start: Option<&str>,
        max_size: u64,
        max_depth: u32,
        alphabet: Option<&StringGenerator>,
        explicit: Vec<String>,
    ) -> PyResult<Self> {
        let start = start.map(hstr);
        let names = hstrs(&explicit);
        let mut raw: *mut c::HegelGrammar = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_grammar_new(
                    ctx,
                    builder.raw.0,
                    start.as_ref().map_or(ptr::null(), |s| s as *const _),
                    max_size,
                    max_depth,
                    alphabet.map_or(ptr::null(), |a| a.raw()),
                    names.as_ptr(),
                    names.len(),
                    &mut raw,
                )
            })
        })?;
        Ok(Grammar { raw: SendPtr(raw) })
    }
}

/// One draw of a grammar string whose explicit terminals take values the
/// Python layer draws. Holds the grammar it borrows.
#[pyclass(frozen, name = "GrammarDraw", module = "hegel._native")]
pub(crate) struct GrammarDraw {
    raw: SendPtr<c::HegelGrammarDraw>,
    _grammar: Py<Grammar>,
}

impl Drop for GrammarDraw {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_grammar_draw_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl GrammarDraw {
    #[new]
    fn new(grammar: Py<Grammar>, max_size: u64) -> PyResult<Self> {
        let mut raw: *mut c::HegelGrammarDraw = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_grammar_draw_new(ctx, grammar.get().raw.0, max_size, &mut raw)
            })
        })?;
        Ok(GrammarDraw {
            raw: SendPtr(raw),
            _grammar: grammar,
        })
    }

    /// The index of the explicit terminal whose value is due next, or
    /// `None` once the string is complete.
    fn next(&self, tc: &TestCase) -> PyResult<Option<usize>> {
        let mut terminal = 0usize;
        let rc = with_ctx(|ctx| unsafe {
            c::hegel_grammar_draw_next(ctx, tc.tc(), self.raw.0, &mut terminal)
        });
        if rc == Rc::HEGEL_E_NOT_FOUND {
            return Ok(None);
        }
        check(rc)?;
        Ok(Some(terminal))
    }

    fn provide(&self, value: &str) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe { c::hegel_grammar_draw_provide(ctx, self.raw.0, hstr(value)) })
        })
    }

    fn result(&self) -> PyResult<String> {
        let mut result = c::hegel_owned_string_t::default();
        with_ctx(|ctx| {
            check(unsafe { c::hegel_grammar_draw_result(ctx, self.raw.0, &mut result) })?;
            // SAFETY: on success data points to len bytes of UTF-8 owned by
            // us until released with the matching free.
            let bytes = unsafe { std::slice::from_raw_parts(result.data as *const u8, result.len) };
            let text = String::from_utf8_lossy(bytes).into_owned();
            let _ = unsafe { c::hegel_owned_string_free(ctx, &mut result) };
            Ok(text)
        })
    }
}
