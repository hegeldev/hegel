use crate::{SendPtr, check, cstring, with_ctx};
use hegel_c as c;
use pyo3::prelude::*;
use std::ffi::{CString, c_char};
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

fn c_strings(items: &Option<Vec<String>>) -> Vec<CString> {
    items
        .as_ref()
        .map(|v| v.iter().map(|s| cstring(s)).collect())
        .unwrap_or_default()
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
        let c_codec = codec.map(cstring);
        let cats = c_strings(&categories);
        let excl = c_strings(&exclude_categories);
        let cat_ptrs: Vec<*const c_char> = cats.iter().map(|s| s.as_ptr()).collect();
        let excl_ptrs: Vec<*const c_char> = excl.iter().map(|s| s.as_ptr()).collect();
        let mut raw: *mut c::HegelStringGenerator = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_string_generator_text(
                    ctx,
                    min_size,
                    max_size,
                    c_codec.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
                    min_codepoint,
                    max_codepoint.unwrap_or(u32::MAX),
                    if categories.is_some() {
                        cat_ptrs.as_ptr()
                    } else {
                        ptr::null()
                    },
                    cat_ptrs.len(),
                    if exclude_categories.is_some() {
                        excl_ptrs.as_ptr()
                    } else {
                        ptr::null()
                    },
                    excl_ptrs.len(),
                    include_characters.map_or(ptr::null(), |s| s.as_ptr()),
                    include_characters.map_or(0, |s| s.len()),
                    exclude_characters.map_or(ptr::null(), |s| s.as_ptr()),
                    exclude_characters.map_or(0, |s| s.len()),
                    &mut raw,
                )
            })
        })?;
        Ok(StringGenerator { raw: SendPtr(raw) })
    }

    #[staticmethod]
    #[pyo3(signature = (pattern, fullmatch, alphabet))]
    fn regex(pattern: &str, fullmatch: bool, alphabet: Option<&StringGenerator>) -> PyResult<Self> {
        let c_pattern = cstring(pattern);
        let mut raw: *mut c::HegelStringGenerator = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_string_generator_regex(
                    ctx,
                    c_pattern.as_ptr(),
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
        with_ctx(|ctx| check(unsafe { c::hegel_string_generator_domain(ctx, max_length, &mut raw) }))?;
        Ok(StringGenerator { raw: SendPtr(raw) })
    }
}
