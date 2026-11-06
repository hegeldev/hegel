use crate::{SendPtr, check, hstr, hstrs, string_from, with_ctx};
use hegel_c as c;
use hegel_c::hegel_result_t as Rc;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;

pub(crate) type PyZoneType = (i32, bool, String);
pub(crate) type PyRuleDate = (u32, u16, u8, u8, u8, i32);
pub(crate) type PyZoneData = (
    PyZoneType,
    Vec<(i64, PyZoneType)>,
    u32,
    Option<PyZoneType>,
    Option<PyZoneType>,
    Option<(PyRuleDate, PyRuleDate)>,
    i32,
);

thread_local! {
    static CALLBACK_ERROR: RefCell<Option<PyErr>> = const { RefCell::new(None) };
}

/// The exception a Python callback raised during the engine call that
/// just failed on this thread, if one did.
pub(crate) fn take_callback_error() -> Option<PyErr> {
    CALLBACK_ERROR.with(|cell| cell.borrow_mut().take())
}

/// Record a callback's exception for the engine call it fails, and set
/// its message on the callback's context.
fn fail_callback(ctx: *mut c::HegelContext, error: PyErr) -> Rc {
    let message = error.to_string();
    let _ = unsafe { c::hegel_context_set_error(ctx, hstr(&message)) };
    CALLBACK_ERROR.with(|cell| *cell.borrow_mut() = Some(error));
    Rc::HEGEL_E_INVALID_ARG
}

/// `error`, the engine's error for a failed engine call, with the
/// exception a Python callback raised during that call as its cause. An
/// exception that is not an `Exception` (a `KeyboardInterrupt`, say) is
/// raised as it is.
pub(crate) fn with_callback_cause(error: PyErr) -> PyErr {
    let Some(cause) = take_callback_error() else {
        return error;
    };
    Python::attach(|py| {
        if cause.is_instance_of::<PyException>(py) {
            error.set_cause(py, Some(cause));
            error
        } else {
            cause
        }
    })
}

fn zone_type(t: &PyZoneType) -> c::hegel_zone_type_t {
    c::hegel_zone_type_t {
        utoff: t.0,
        is_dst: t.1,
        abbreviation: hstr(&t.2),
    }
}

fn rule_date(r: PyRuleDate) -> c::hegel_zone_rule_date_t {
    c::hegel_zone_rule_date_t {
        kind: r.0,
        day: r.1,
        month: r.2,
        week: r.3,
        weekday: r.4,
        time: r.5,
    }
}

unsafe fn add_tzif(
    ctx: *mut c::HegelContext,
    source: *mut c::HegelZoneSource,
    name: &str,
    data: &[u8],
) -> Rc {
    unsafe { c::hegel_zone_source_add_tzif(ctx, source, hstr(name), data.as_ptr(), data.len()) }
}

unsafe fn add_zone(
    ctx: *mut c::HegelContext,
    source: *mut c::HegelZoneSource,
    name: &str,
    data: &PyZoneData,
) -> Rc {
    let (initial, transitions, after, standard, dst, dates, last_year) = data;
    let empty = (0, false, String::new());
    let transitions: Vec<c::hegel_zone_transition_t> = transitions
        .iter()
        .map(|(instant, t)| c::hegel_zone_transition_t {
            instant: *instant,
            local_type: zone_type(t),
        })
        .collect();
    let (dst_start, dst_end) = dates.unwrap_or(((0, 0, 0, 0, 0, 0), (0, 0, 0, 0, 0, 0)));
    let raw = c::hegel_zone_data_t {
        initial: zone_type(initial),
        transitions: transitions.as_ptr(),
        transition_count: transitions.len(),
        after: *after,
        standard: zone_type(standard.as_ref().unwrap_or(&empty)),
        has_dst: dst.is_some(),
        dst: zone_type(dst.as_ref().unwrap_or(&empty)),
        dst_start: rule_date(dst_start),
        dst_end: rule_date(dst_end),
        last_year: *last_year,
    };
    unsafe { c::hegel_zone_source_add_zone(ctx, source, hstr(name), &raw) }
}

struct Oracle {
    offset_at: Py<PyAny>,
    next_transition: Option<Py<PyAny>>,
    abbreviation: RefCell<String>,
}

unsafe extern "C" fn oracle_offset_at(
    user_data: *mut c_void,
    ctx: *mut c::HegelContext,
    instant: i64,
    out_type: *mut c::hegel_zone_type_t,
) -> Rc {
    // SAFETY: user_data is the Oracle add_oracle holds for this call.
    let oracle = unsafe { &*(user_data as *const Oracle) };
    Python::attach(|py| {
        let answer = oracle
            .offset_at
            .call1(py, (instant,))
            .and_then(|answer| answer.extract::<PyZoneType>(py));
        match answer {
            Ok((utoff, is_dst, abbreviation)) => {
                *oracle.abbreviation.borrow_mut() = abbreviation;
                let stored = oracle.abbreviation.borrow();
                unsafe {
                    *out_type = c::hegel_zone_type_t {
                        utoff,
                        is_dst,
                        abbreviation: hstr(&stored),
                    };
                }
                Rc::HEGEL_OK
            }
            Err(error) => fail_callback(ctx, error),
        }
    })
}

unsafe extern "C" fn oracle_next_transition(
    user_data: *mut c_void,
    ctx: *mut c::HegelContext,
    after: i64,
    out_found: *mut bool,
    out_instant: *mut i64,
) -> Rc {
    // SAFETY: as for oracle_offset_at; this is only registered when the
    // oracle has a next_transition.
    let oracle = unsafe { &*(user_data as *const Oracle) };
    Python::attach(|py| {
        let answer = oracle.next_transition.as_ref().map_or(Ok(None), |next| {
            next.call1(py, (after,))
                .and_then(|answer| answer.extract::<Option<i64>>(py))
        });
        match answer {
            Ok(found) => {
                unsafe {
                    *out_found = found.is_some();
                    *out_instant = found.unwrap_or(0);
                }
                Rc::HEGEL_OK
            }
            Err(error) => fail_callback(ctx, error),
        }
    })
}

unsafe fn add_oracle(
    ctx: *mut c::HegelContext,
    source: *mut c::HegelZoneSource,
    name: &str,
    offset_at: Py<PyAny>,
    next_transition: Option<Py<PyAny>>,
    first_year: i32,
    last_year: i32,
    after: u32,
) -> Rc {
    let has_next = next_transition.is_some();
    let oracle = Oracle {
        offset_at,
        next_transition,
        abbreviation: RefCell::new(String::new()),
    };
    let raw = c::hegel_zone_oracle_t {
        offset_at: Some(oracle_offset_at),
        next_transition: if has_next {
            Some(oracle_next_transition)
        } else {
            None
        },
        user_data: &oracle as *const Oracle as *mut c_void,
        first_year,
        last_year,
        after,
    };
    unsafe { c::hegel_zone_source_add_oracle(ctx, source, hstr(name), &raw) }
}

/// Add what a loader returned for `name`: `None`, or a `(kind, value)`
/// pair the Python layer built, `kind` being `"tzif"`, `"zone"`, `"link"`
/// or `"oracle"`.
unsafe fn add_entry(
    ctx: *mut c::HegelContext,
    source: *mut c::HegelZoneSource,
    name: &str,
    entry: &Bound<'_, PyAny>,
) -> PyResult<Rc> {
    if entry.is_none() {
        return Ok(Rc::HEGEL_OK);
    }
    let (kind, value): (String, Bound<'_, PyAny>) = entry.extract()?;
    Ok(match kind.as_str() {
        "tzif" => unsafe { add_tzif(ctx, source, name, value.cast::<PyBytes>()?.as_bytes()) },
        "zone" => unsafe { add_zone(ctx, source, name, &value.extract()?) },
        "link" => unsafe {
            c::hegel_zone_source_add_link(
                ctx,
                source,
                hstr(name),
                hstr(&value.extract::<String>()?),
            )
        },
        _ => {
            let (offset_at, next_transition, first_year, last_year, after): (
                Py<PyAny>,
                Option<Py<PyAny>>,
                i32,
                i32,
                u32,
            ) = value.extract()?;
            unsafe {
                add_oracle(
                    ctx,
                    source,
                    name,
                    offset_at,
                    next_transition,
                    first_year,
                    last_year,
                    after,
                )
            }
        }
    })
}

unsafe extern "C" fn loader_trampoline(
    user_data: *mut c_void,
    ctx: *mut c::HegelContext,
    source: *mut c::HegelZoneSource,
    name: c::hegel_string_t,
) -> Rc {
    // SAFETY: user_data is a leaked Py<PyAny> that set_loader registered,
    // valid for the life of the process.
    let loader = unsafe { &*(user_data as *const Py<PyAny>) };
    let name = string_from(name);
    Python::attach(|py| {
        let result = loader
            .call1(py, (name.as_str(),))
            .and_then(|entry| unsafe { add_entry(ctx, source, &name, entry.bind(py)) });
        match result {
            Ok(rc) => rc,
            Err(error) => fail_callback(ctx, error),
        }
    })
}

/// An owned libhegel zone-source handle: the zone data time zone draws
/// and queries use. The source itself lives on while the default or a
/// spec holds it.
#[pyclass(frozen, name = "ZoneSource", module = "hegel._native")]
pub(crate) struct ZoneSource {
    raw: SendPtr<c::HegelZoneSource>,
}

impl ZoneSource {
    pub(crate) fn raw(&self) -> *const c::HegelZoneSource {
        self.raw.0
    }

    fn change(&self, f: impl FnOnce(*mut c::HegelContext) -> Rc) -> PyResult<()> {
        with_ctx(|ctx| check(f(ctx)))
    }
}

impl Drop for ZoneSource {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_zone_source_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl ZoneSource {
    /// An empty source.
    #[new]
    fn new() -> PyResult<Self> {
        let mut raw: *mut c::HegelZoneSource = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_zone_source_new(ctx, &mut raw) }))?;
        Ok(ZoneSource { raw: SendPtr(raw) })
    }

    /// The engine's bundled database.
    #[staticmethod]
    fn bundled() -> PyResult<Self> {
        let mut raw: *mut c::HegelZoneSource = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_zone_source_bundled(ctx, &mut raw) }))?;
        Ok(ZoneSource { raw: SendPtr(raw) })
    }

    fn add_tzif(&self, name: &str, data: &[u8]) -> PyResult<()> {
        self.change(|ctx| unsafe { add_tzif(ctx, self.raw.0, name, data) })
    }

    /// `data` is `(initial, transitions, after, standard, dst, (dst_start,
    /// dst_end), last_year)`, each type `(utoff, is_dst, abbreviation)`,
    /// each transition `(instant, type)` and each rule date `(kind, day,
    /// month, week, weekday, time)`.
    fn add_zone(&self, name: &str, data: PyZoneData) -> PyResult<()> {
        self.change(|ctx| unsafe { add_zone(ctx, self.raw.0, name, &data) })
    }

    /// Probe `offset_at(instant) -> (utoff, is_dst, abbreviation)` and the
    /// optional `next_transition(after) -> instant | None` over the years
    /// `first_year` to `last_year`; `after` is a `ZONE_AFTER_*` constant.
    #[pyo3(signature = (name, offset_at, next_transition, first_year, last_year, after))]
    fn add_oracle(
        &self,
        name: &str,
        offset_at: Py<PyAny>,
        next_transition: Option<Py<PyAny>>,
        first_year: i32,
        last_year: i32,
        after: u32,
    ) -> PyResult<()> {
        self.change(|ctx| unsafe {
            add_oracle(
                ctx,
                self.raw.0,
                name,
                offset_at,
                next_transition,
                first_year,
                last_year,
                after,
            )
        })
    }

    fn add_link(&self, name: &str, target: &str) -> PyResult<()> {
        self.change(|ctx| unsafe {
            c::hegel_zone_source_add_link(ctx, self.raw.0, hstr(name), hstr(target))
        })
    }

    fn declare(&self, names: Vec<String>) -> PyResult<()> {
        let views = hstrs(&names);
        self.change(|ctx| unsafe {
            c::hegel_zone_source_declare(ctx, self.raw.0, views.as_ptr(), views.len())
        })
    }

    /// Ask `loader(name)` for the data of a name the source lacks; it
    /// returns `None` or a `(kind, value)` pair, as `add_entry` reads it.
    /// The loader is kept for the life of the process, since the source
    /// may outlive this handle.
    #[pyo3(signature = (loader))]
    fn set_loader(&self, loader: Option<Py<PyAny>>) -> PyResult<()> {
        let (callback, user_data): (c::hegel_zone_loader_t, *mut c_void) = match loader {
            Some(loader) => (
                Some(loader_trampoline),
                Box::into_raw(Box::new(loader)) as *mut c_void,
            ),
            None => (None, ptr::null_mut()),
        };
        self.change(|ctx| unsafe {
            c::hegel_zone_source_set_loader(ctx, self.raw.0, callback, user_data)
        })
    }

    fn set_version(&self, version: &str) -> PyResult<()> {
        self.change(|ctx| unsafe {
            c::hegel_zone_source_set_version(ctx, self.raw.0, hstr(version))
        })
    }

    fn version(&self) -> PyResult<String> {
        let mut out = c::hegel_string_t::default();
        with_ctx(|ctx| check(unsafe { c::hegel_zone_source_version(ctx, self.raw.0, &mut out) }))?;
        Ok(string_from(out))
    }
}

/// Make `source` the process's default zone source, or with `None` the
/// bundled database.
#[pyfunction]
#[pyo3(signature = (source))]
pub(crate) fn set_default_zone_source(source: Option<&ZoneSource>) -> PyResult<()> {
    let raw = source.map_or(ptr::null(), ZoneSource::raw);
    with_ctx(|ctx| check(unsafe { c::hegel_set_default_zone_source(ctx, raw) }))
}
