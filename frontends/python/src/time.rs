use crate::{SendPtr, check, hstr, hstrs, string_from, with_ctx};
use hegel_c as c;
use pyo3::prelude::*;
use std::ptr;

pub(crate) type PyDate = (i32, u8, u8);
pub(crate) type PyTime = (u8, u8, u8, u32);
pub(crate) type PyDateTime = (i32, u8, u8, u8, u8, u8, u32);
pub(crate) type PyTimestamp = (i64, u32);
pub(crate) type PyDuration = (i64, i32);

pub(crate) fn date_to_c(d: PyDate) -> c::hegel_date_t {
    c::hegel_date_t {
        year: d.0,
        month: d.1,
        day: d.2,
    }
}

pub(crate) fn date_from_c(d: c::hegel_date_t) -> PyDate {
    (d.year, d.month, d.day)
}

pub(crate) fn time_to_c(t: PyTime) -> c::hegel_time_t {
    c::hegel_time_t {
        hour: t.0,
        minute: t.1,
        second: t.2,
        nanosecond: t.3,
    }
}

pub(crate) fn time_from_c(t: c::hegel_time_t) -> PyTime {
    (t.hour, t.minute, t.second, t.nanosecond)
}

pub(crate) fn datetime_to_c(d: PyDateTime) -> c::hegel_datetime_t {
    c::hegel_datetime_t {
        date: date_to_c((d.0, d.1, d.2)),
        time: time_to_c((d.3, d.4, d.5, d.6)),
    }
}

pub(crate) fn datetime_from_c(d: c::hegel_datetime_t) -> PyDateTime {
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

pub(crate) fn timestamp_to_c(t: PyTimestamp) -> c::hegel_timestamp_t {
    c::hegel_timestamp_t {
        second: t.0,
        nanosecond: t.1,
    }
}

pub(crate) fn timestamp_from_c(t: c::hegel_timestamp_t) -> PyTimestamp {
    (t.second, t.nanosecond)
}

pub(crate) fn duration_to_c(d: PyDuration) -> c::hegel_duration_t {
    c::hegel_duration_t {
        seconds: d.0,
        nanoseconds: d.1,
    }
}

pub(crate) fn duration_from_c(d: c::hegel_duration_t) -> PyDuration {
    (d.seconds, d.nanoseconds)
}

/// An owned libhegel date-options handle: the weekday and month masks of a
/// date draw. Immutable after construction.
#[pyclass(frozen, name = "DateOptions", module = "hegel._native")]
pub(crate) struct DateOptions {
    raw: SendPtr<c::HegelDateOptions>,
}

impl DateOptions {
    pub(crate) fn raw(&self) -> *const c::HegelDateOptions {
        self.raw.0
    }
}

impl Drop for DateOptions {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_date_options_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl DateOptions {
    /// Bit sets with bit 0 for Monday and bit 0 for January; `None` leaves
    /// every weekday or month allowed.
    #[new]
    #[pyo3(signature = (weekdays, months))]
    fn new(weekdays: Option<u32>, months: Option<u32>) -> PyResult<Self> {
        let mut raw: *mut c::HegelDateOptions = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_date_options_new(ctx, &mut raw) }))?;
        let options = DateOptions { raw: SendPtr(raw) };
        with_ctx(|ctx| {
            if let Some(mask) = weekdays {
                check(unsafe { c::hegel_date_options_set_weekdays(ctx, raw, mask) })?;
            }
            if let Some(mask) = months {
                check(unsafe { c::hegel_date_options_set_months(ctx, raw, mask) })?;
            }
            Ok::<(), PyErr>(())
        })?;
        Ok(options)
    }
}

/// An owned libhegel time-zone-spec handle: the names a time zone draw may
/// yield. Immutable after construction, so it may be shared across test
/// cases and threads.
#[pyclass(frozen, name = "TimeZoneSpec", module = "hegel._native")]
pub(crate) struct TimeZoneSpec {
    raw: SendPtr<c::HegelTimezoneSpec>,
}

impl TimeZoneSpec {
    pub(crate) fn raw(&self) -> *const c::HegelTimezoneSpec {
        self.raw.0
    }
}

impl Drop for TimeZoneSpec {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_timezone_spec_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl TimeZoneSpec {
    /// The engine's default set with the link, legacy, `Etc/Unknown` and
    /// opaque-name switches applied, or the explicit list `names`.
    #[new]
    #[pyo3(signature = (names, links, legacy, unknown, opaque))]
    fn new(
        names: Option<Vec<String>>,
        links: bool,
        legacy: bool,
        unknown: bool,
        opaque: bool,
    ) -> PyResult<Self> {
        let mut raw: *mut c::HegelTimezoneSpec = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_timezone_spec_new(ctx, &mut raw) }))?;
        let spec = TimeZoneSpec { raw: SendPtr(raw) };
        with_ctx(|ctx| {
            check(unsafe { c::hegel_timezone_spec_set_links(ctx, raw, links) })?;
            check(unsafe { c::hegel_timezone_spec_set_legacy(ctx, raw, legacy) })?;
            check(unsafe { c::hegel_timezone_spec_set_unknown(ctx, raw, unknown) })?;
            check(unsafe { c::hegel_timezone_spec_set_opaque(ctx, raw, opaque) })?;
            if let Some(names) = &names {
                let views = hstrs(names);
                check(unsafe {
                    c::hegel_timezone_spec_set_names(ctx, raw, views.as_ptr(), views.len())
                })?;
            }
            Ok::<(), PyErr>(())
        })?;
        Ok(spec)
    }
}

/// An owned libhegel zoned-datetime-spec handle: the bounds, precision and
/// policies of a zoned datetime draw. Immutable after construction.
#[pyclass(frozen, name = "ZonedSpec", module = "hegel._native")]
pub(crate) struct ZonedSpec {
    raw: SendPtr<c::HegelZonedSpec>,
}

impl ZonedSpec {
    pub(crate) fn raw(&self) -> *const c::HegelZonedSpec {
        self.raw.0
    }
}

impl Drop for ZonedSpec {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_zoned_spec_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl ZonedSpec {
    /// Bounds on the wall clock (`civil_bounds`) or on the instant
    /// (`instant_bounds`), the unit the time of day is counted in (a
    /// `PRECISION_*` constant), whether a skipped wall time may be drawn,
    /// and which instant a repeated one means (an `AMBIGUOUS_*` constant).
    #[new]
    #[pyo3(signature = (civil_bounds, instant_bounds, precision, allow_imaginary, ambiguous))]
    fn new(
        civil_bounds: Option<(PyDateTime, PyDateTime)>,
        instant_bounds: Option<(PyTimestamp, PyTimestamp)>,
        precision: u32,
        allow_imaginary: bool,
        ambiguous: u32,
    ) -> PyResult<Self> {
        let mut raw: *mut c::HegelZonedSpec = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_zoned_spec_new(ctx, &mut raw) }))?;
        let spec = ZonedSpec { raw: SendPtr(raw) };
        let imaginary = if allow_imaginary {
            c::hegel_imaginary_t::HEGEL_IMAGINARY_ALLOW
        } else {
            c::hegel_imaginary_t::HEGEL_IMAGINARY_AVOID
        } as u32;
        with_ctx(|ctx| {
            check(unsafe { c::hegel_zoned_spec_set_precision(ctx, raw, precision) })?;
            check(unsafe { c::hegel_zoned_spec_set_imaginary(ctx, raw, imaginary) })?;
            check(unsafe { c::hegel_zoned_spec_set_ambiguous(ctx, raw, ambiguous) })?;
            if let Some((min, max)) = civil_bounds {
                check(unsafe {
                    c::hegel_zoned_spec_set_civil_bounds(
                        ctx,
                        raw,
                        datetime_to_c(min),
                        datetime_to_c(max),
                    )
                })?;
            }
            if let Some((min, max)) = instant_bounds {
                check(unsafe {
                    c::hegel_zoned_spec_set_instant_bounds(
                        ctx,
                        raw,
                        timestamp_to_c(min),
                        timestamp_to_c(max),
                    )
                })?;
            }
            Ok::<(), PyErr>(())
        })?;
        Ok(spec)
    }
}

/// The IANA release of the time zone database bundled in the engine.
#[pyfunction]
pub(crate) fn tzdb_version() -> PyResult<String> {
    let mut out = c::hegel_string_t::default();
    with_ctx(|ctx| check(unsafe { c::hegel_tzdb_version(ctx, &mut out) }))?;
    Ok(string_from(out))
}

/// What the engine knows about the zone `name`, as `(primary_name,
/// is_link, rank, tags)`, or `None` for a name its database lacks.
#[pyfunction]
pub(crate) fn timezone_info(name: &str) -> PyResult<Option<(String, bool, u16, u32)>> {
    let mut out = c::hegel_timezone_info_t {
        known: false,
        primary_name: c::hegel_string_t::default(),
        is_link: false,
        rank: 0,
        tags: 0,
    };
    with_ctx(|ctx| check(unsafe { c::hegel_timezone_info(ctx, hstr(name), &mut out) }))?;
    Ok(out.known.then(|| {
        (
            string_from(out.primary_name),
            out.is_link,
            out.rank,
            out.tags,
        )
    }))
}

/// Classify the wall time `civil` in the zone `name`, as `(kind,
/// offset_before, offset_after, offset_seconds, offset_known, timestamp)`.
#[pyfunction]
pub(crate) fn timezone_resolve(
    name: &str,
    civil: PyDateTime,
) -> PyResult<(u32, i32, i32, i32, bool, PyTimestamp)> {
    let mut out = c::hegel_timezone_resolution_t {
        kind: c::hegel_timezone_kind_t::HEGEL_TIMEZONE_KIND_UNIQUE,
        offset_before: 0,
        offset_after: 0,
        offset_seconds: 0,
        offset_known: false,
        timestamp: timestamp_to_c((0, 0)),
    };
    with_ctx(|ctx| {
        check(unsafe { c::hegel_timezone_resolve(ctx, hstr(name), datetime_to_c(civil), &mut out) })
    })?;
    Ok((
        out.kind as u32,
        out.offset_before,
        out.offset_after,
        out.offset_seconds,
        out.offset_known,
        timestamp_from_c(out.timestamp),
    ))
}
