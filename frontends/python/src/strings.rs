use crate::{SendPtr, check, hstr, hstrs, with_ctx};
use hegel_c as c;
use pyo3::prelude::*;
use std::ptr;

/// An owned libhegel string-generator handle. Immutable after construction,
/// so it may be shared across test cases and threads.
#[pyclass(frozen, name = "StringGenerator", module = "hegel._native")]
pub(crate) struct StringGenerator {
    raw: SendPtr<c::HegelStringGenerator>,
}

impl StringGenerator {
    pub(crate) fn raw(&self) -> *const c::HegelStringGenerator {
        self.raw.0
    }
}

impl Drop for StringGenerator {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_string_generator_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl StringGenerator {
    #[staticmethod]
    #[pyo3(signature = (min_size, max_size, codec, min_codepoint, max_codepoint, categories, exclude_categories, include_characters, exclude_characters))]
    fn text(
        min_size: u64,
        max_size: u64,
        codec: Option<&str>,
        min_codepoint: u32,
        max_codepoint: Option<u32>,
        categories: Option<Vec<String>>,
        exclude_categories: Option<Vec<String>>,
        include_characters: Option<&str>,
        exclude_characters: Option<&str>,
    ) -> PyResult<Self> {
        let codec = codec.map(hstr);
        let cats = categories.as_deref().map(hstrs);
        let excl = exclude_categories.as_deref().map(hstrs);
        let mut raw: *mut c::HegelStringGenerator = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_string_generator_text(
                    ctx,
                    min_size,
                    max_size,
                    codec.as_ref().map_or(ptr::null(), |s| s as *const _),
                    min_codepoint,
                    max_codepoint.unwrap_or(u32::MAX),
                    cats.as_ref().map_or(ptr::null(), |v| v.as_ptr()),
                    cats.as_ref().map_or(0, |v| v.len()),
                    excl.as_ref().map_or(ptr::null(), |v| v.as_ptr()),
                    excl.as_ref().map_or(0, |v| v.len()),
                    hstr(include_characters.unwrap_or("")),
                    hstr(exclude_characters.unwrap_or("")),
                    &mut raw,
                )
            })
        })?;
        Ok(StringGenerator { raw: SendPtr(raw) })
    }

    #[staticmethod]
    #[pyo3(signature = (pattern, fullmatch, alphabet))]
    fn regex(pattern: &str, fullmatch: bool, alphabet: Option<&StringGenerator>) -> PyResult<Self> {
        let mut raw: *mut c::HegelStringGenerator = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_string_generator_regex(
                    ctx,
                    hstr(pattern),
                    fullmatch,
                    alphabet.map_or(ptr::null(), |a| a.raw()),
                    &mut raw,
                )
            })
        })?;
        Ok(StringGenerator { raw: SendPtr(raw) })
    }

    #[staticmethod]
    fn email() -> PyResult<Self> {
        let mut raw: *mut c::HegelStringGenerator = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_string_generator_email(ctx, &mut raw) }))?;
        Ok(StringGenerator { raw: SendPtr(raw) })
    }

    #[staticmethod]
    fn url() -> PyResult<Self> {
        let mut raw: *mut c::HegelStringGenerator = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_string_generator_url(ctx, &mut raw) }))?;
        Ok(StringGenerator { raw: SendPtr(raw) })
    }

    #[staticmethod]
    fn domain(max_length: u64) -> PyResult<Self> {
        let mut raw: *mut c::HegelStringGenerator = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe { c::hegel_string_generator_domain(ctx, max_length, &mut raw) })
        })?;
        Ok(StringGenerator { raw: SendPtr(raw) })
    }
}
