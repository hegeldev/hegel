use crate::{SendPtr, check, hstr, opt_string_from, with_ctx};
use hegel_c as c;
use pyo3::prelude::*;
use std::ptr;

/// An owned libhegel settings handle.
#[pyclass(frozen, name = "Settings", module = "hegel._native")]
pub(crate) struct Settings {
    raw: SendPtr<c::HegelSettings>,
}

impl Settings {
    pub(crate) fn raw(&self) -> *const c::HegelSettings {
        self.raw.0
    }

    fn from_raw(raw: *mut c::HegelSettings) -> Self {
        Settings { raw: SendPtr(raw) }
    }
}

impl Drop for Settings {
    fn drop(&mut self) {
        // SAFETY: raw came from a settings constructor and is freed once.
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_settings_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl Settings {
    /// Settings initialised from the default profile.
    #[staticmethod]
    fn new() -> PyResult<Self> {
        let mut raw: *mut c::HegelSettings = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_settings_new(ctx, &mut raw) }))?;
        Ok(Settings::from_raw(raw))
    }

    /// Settings initialised from the named profile.
    #[staticmethod]
    fn for_profile(name: &str) -> PyResult<Self> {
        let mut raw: *mut c::HegelSettings = ptr::null_mut();
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_new_for_profile(ctx, hstr(name), &mut raw) })
        })?;
        Ok(Settings::from_raw(raw))
    }

    fn set_backend(&self, value: u32) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_backend(ctx, self.raw.0, value) }))
    }

    fn set_nondeterminism_strictness(&self, value: u32) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_settings_set_nondeterminism_strictness(ctx, self.raw.0, value)
            })
        })
    }

    fn set_test_cases(&self, value: u64) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_test_cases(ctx, self.raw.0, value) }))
    }

    fn set_verbosity(&self, value: u32) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_verbosity(ctx, self.raw.0, value) }))
    }

    fn set_derandomize(&self, value: bool) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_derandomize(ctx, self.raw.0, value) }))
    }

    fn set_report_multiple_failures(&self, value: bool) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_set_report_multiple_failures(ctx, self.raw.0, value) })
        })
    }

    fn set_stop_on_first_failure(&self, value: bool) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_set_stop_on_first_failure(ctx, self.raw.0, value) })
        })
    }

    fn set_show_statistics(&self, value: bool) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_set_show_statistics(ctx, self.raw.0, value) })
        })
    }

    fn set_unbounded_choices(&self, value: bool) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_set_unbounded_choices(ctx, self.raw.0, value) })
        })
    }

    fn set_phases(&self, value: u32) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_phases(ctx, self.raw.0, value) }))
    }

    fn set_suppress_health_check(&self, value: u32) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_set_suppress_health_check(ctx, self.raw.0, value) })
        })
    }

    fn set_print_blob(&self, value: bool) -> PyResult<()> {
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_print_blob(ctx, self.raw.0, value) }))
    }

    #[pyo3(signature = (seed))]
    fn set_seed(&self, seed: Option<u64>) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_settings_set_seed(ctx, self.raw.0, seed.unwrap_or(0), seed.is_some())
            })
        })
    }

    /// `None` selects the engine default, `""` disables the database, and
    /// any other string is used as the database directory.
    #[pyo3(signature = (database))]
    fn set_database(&self, database: Option<&str>) -> PyResult<()> {
        let s = database.map(hstr);
        let p = s.as_ref().map_or(ptr::null(), |s| s as *const _);
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_database(ctx, self.raw.0, p) }))
    }

    #[pyo3(signature = (key))]
    fn set_database_key(&self, key: Option<&str>) -> PyResult<()> {
        let s = key.map(hstr);
        let p = s.as_ref().map_or(ptr::null(), |s| s as *const _);
        with_ctx(|ctx| check(unsafe { c::hegel_settings_set_database_key(ctx, self.raw.0, p) }))
    }

    fn set_test_location(
        &self,
        file: &str,
        begin_line: u32,
        class_name: &str,
        function: &str,
    ) -> PyResult<()> {
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_settings_set_test_location(
                    ctx,
                    self.raw.0,
                    hstr(file),
                    begin_line,
                    hstr(class_name),
                    hstr(function),
                )
            })
        })
    }

    fn get_test_cases(&self) -> PyResult<u64> {
        let mut out: u64 = 0;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_test_cases(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    fn get_derandomize(&self) -> PyResult<bool> {
        let mut out: bool = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_derandomize(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    fn get_phases(&self) -> PyResult<u32> {
        let mut out: u32 = 0;
        with_ctx(|ctx| check(unsafe { c::hegel_settings_get_phases(ctx, self.raw.0, &mut out) }))?;
        Ok(out)
    }

    fn get_print_blob(&self) -> PyResult<bool> {
        let mut out: bool = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_print_blob(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    fn get_report_multiple_failures(&self) -> PyResult<bool> {
        let mut out: bool = false;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_settings_get_report_multiple_failures(ctx, self.raw.0, &mut out)
            })
        })?;
        Ok(out)
    }

    fn get_stop_on_first_failure(&self) -> PyResult<bool> {
        let mut out: bool = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_stop_on_first_failure(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    fn get_show_statistics(&self) -> PyResult<bool> {
        let mut out: bool = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_show_statistics(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    fn get_unbounded_choices(&self) -> PyResult<bool> {
        let mut out: bool = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_unbounded_choices(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    fn get_suppress_health_check(&self) -> PyResult<u32> {
        let mut out: u32 = 0;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_suppress_health_check(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out)
    }

    fn get_verbosity(&self) -> PyResult<u32> {
        let mut out = c::hegel_verbosity_t::HEGEL_VERBOSITY_NORMAL;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_verbosity(ctx, self.raw.0, &mut out) })
        })?;
        Ok(out as u32)
    }

    fn get_backend(&self) -> PyResult<u32> {
        let mut out = c::hegel_backend_t::HEGEL_BACKEND_DEFAULT;
        with_ctx(|ctx| check(unsafe { c::hegel_settings_get_backend(ctx, self.raw.0, &mut out) }))?;
        Ok(out as u32)
    }

    fn get_nondeterminism_strictness(&self) -> PyResult<u32> {
        let mut out = c::hegel_nondeterminism_strictness_t::HEGEL_NONDETERMINISM_QUIET;
        with_ctx(|ctx| {
            check(unsafe {
                c::hegel_settings_get_nondeterminism_strictness(ctx, self.raw.0, &mut out)
            })
        })?;
        Ok(out as u32)
    }

    fn get_seed(&self) -> PyResult<Option<u64>> {
        let mut seed = 0u64;
        let mut has_seed = false;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_settings_get_seed(ctx, self.raw.0, &mut seed, &mut has_seed) })
        })?;
        Ok(has_seed.then_some(seed))
    }

    fn get_database(&self) -> PyResult<Option<String>> {
        let mut p: *const c::hegel_string_t = ptr::null();
        with_ctx(|ctx| check(unsafe { c::hegel_settings_get_database(ctx, self.raw.0, &mut p) }))?;
        Ok(opt_string_from(p))
    }
}
