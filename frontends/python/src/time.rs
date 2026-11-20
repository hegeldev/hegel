use crate::{SendPtr, ZoneSource, check, hstr, hstrs, string_from, with_ctx};
use hegel_c as c;
use pyo3::prelude::*;
use std::ptr;

pub(crate) type PyDate = (i32, u8, u8);
pub(crate) type PyTime = (u8, u8, u8, u32);
pub(crate) type PyDateTime = (i32, u8, u8, u8, u8, u8, u32);
pub(crate) type PyTimestamp = (i64, u32);
pub(crate) type PyDuration = (i64, i32);
pub(crate) type PySpan = (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64);

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
    /// The names of `source` that the link and legacy switches admit, or
    /// the explicit list `names`, checked against the source as they are
    /// now. The check may run the source's loader, so the GIL is released
    /// meanwhile.
    #[new]
    #[pyo3(signature = (names, allow_links, allow_legacy, source))]
    fn new(
        py: Python<'_>,
        names: Option<Vec<String>>,
        allow_links: bool,
        allow_legacy: bool,
        source: &ZoneSource,
    ) -> PyResult<Self> {
        let mut raw: *mut c::HegelTimezoneSpec = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_timezone_spec_new(ctx, &mut raw) }))?;
        let spec = TimeZoneSpec { raw: SendPtr(raw) };
        let raw = SendPtr(raw);
        let source = SendPtr(source.raw() as *mut c::HegelZoneSource);
        let rc = py.detach(move || {
            with_ctx(|ctx| unsafe {
                let raw = raw.get();
                let steps: [&dyn Fn() -> c::hegel_result_t; 5] = [
                    &|| c::hegel_timezone_spec_set_source(ctx, raw, source.get()),
                    &|| c::hegel_timezone_spec_set_links(ctx, raw, allow_links),
                    &|| c::hegel_timezone_spec_set_legacy(ctx, raw, allow_legacy),
                    &|| match &names {
                        Some(names) => {
                            let views = hstrs(names);
                            c::hegel_timezone_spec_set_names(ctx, raw, views.as_ptr(), views.len())
                        }
                        None => c::hegel_result_t::HEGEL_OK,
                    },
                    &|| c::hegel_timezone_spec_validate(ctx, raw),
                ];
                steps
                    .into_iter()
                    .map(|step| step())
                    .find(|rc| *rc != c::hegel_result_t::HEGEL_OK)
                    .unwrap_or(c::hegel_result_t::HEGEL_OK)
            })
        });
        check(rc)?;
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
            check(unsafe { c::hegel_zoned_spec_validate(ctx, raw) })
        })?;
        Ok(spec)
    }
}

/// An owned libhegel span-spec handle: the unit bounds and sign policy of
/// a calendar span draw. Immutable after construction.
#[pyclass(frozen, name = "SpanSpec", module = "hegel._native")]
pub(crate) struct SpanSpec {
    raw: SendPtr<c::HegelSpanSpec>,
}

impl SpanSpec {
    pub(crate) fn raw(&self) -> *const c::HegelSpanSpec {
        self.raw.0
    }
}

impl Drop for SpanSpec {
    fn drop(&mut self) {
        with_ctx(|ctx| unsafe {
            let _ = c::hegel_span_spec_free(ctx, self.raw.0);
        });
    }
}

#[pymethods]
impl SpanSpec {
    /// The inclusive bounds of each enabled unit, as `(unit, min, max)`
    /// with `unit` a `SPAN_UNIT_*` constant, and the sign policy (a
    /// `SPAN_SIGN_*` constant).
    #[new]
    #[pyo3(signature = (bounds, sign))]
    fn new(bounds: Vec<(u32, i64, i64)>, sign: u32) -> PyResult<Self> {
        let mut raw: *mut c::HegelSpanSpec = ptr::null_mut();
        with_ctx(|ctx| check(unsafe { c::hegel_span_spec_new(ctx, &mut raw) }))?;
        let spec = SpanSpec { raw: SendPtr(raw) };
        with_ctx(|ctx| {
            check(unsafe { c::hegel_span_spec_set_sign(ctx, raw, sign) })?;
            for (unit, min, max) in bounds {
                check(unsafe { c::hegel_span_spec_set_unit_bounds(ctx, raw, unit, min, max) })?;
            }
            check(unsafe { c::hegel_span_spec_validate(ctx, raw) })
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

/// Check the arguments of a `TestCase.generate_date` call without drawing:
/// an `InvalidArgument` with the message the draw would give.
#[pyfunction]
#[pyo3(signature = (min_value, max_value, options))]
pub(crate) fn validate_date(
    min_value: PyDate,
    max_value: PyDate,
    options: Option<&DateOptions>,
) -> PyResult<()> {
    with_ctx(|ctx| {
        check(unsafe {
            c::hegel_validate_date(
                ctx,
                date_to_c(min_value),
                date_to_c(max_value),
                options.map_or(ptr::null(), DateOptions::raw),
            )
        })
    })
}

/// Check the arguments of a `TestCase.generate_time` call without drawing.
#[pyfunction]
pub(crate) fn validate_time(min_value: PyTime, max_value: PyTime, precision: u32) -> PyResult<()> {
    with_ctx(|ctx| {
        check(unsafe {
            c::hegel_validate_time(ctx, time_to_c(min_value), time_to_c(max_value), precision)
        })
    })
}

/// Check the arguments of a `TestCase.generate_datetime` call without
/// drawing.
#[pyfunction]
pub(crate) fn validate_datetime(
    min_value: PyDateTime,
    max_value: PyDateTime,
    precision: u32,
) -> PyResult<()> {
    with_ctx(|ctx| {
        check(unsafe {
            c::hegel_validate_datetime(
                ctx,
                datetime_to_c(min_value),
                datetime_to_c(max_value),
                precision,
            )
        })
    })
}

/// Check the arguments of a `TestCase.generate_timestamp` call without
/// drawing.
#[pyfunction]
pub(crate) fn validate_timestamp(
    min_value: PyTimestamp,
    max_value: PyTimestamp,
    precision: u32,
) -> PyResult<()> {
    with_ctx(|ctx| {
        check(unsafe {
            c::hegel_validate_timestamp(
                ctx,
                timestamp_to_c(min_value),
                timestamp_to_c(max_value),
                precision,
            )
        })
    })
}

/// Check the arguments of a `TestCase.generate_offset` call without
/// drawing.
#[pyfunction]
pub(crate) fn validate_offset(
    min_seconds: i32,
    max_seconds: i32,
    granularity: u32,
) -> PyResult<()> {
    with_ctx(|ctx| {
        check(unsafe {
            c::hegel_validate_offset(ctx, min_seconds, max_seconds, granularity, false)
        })
    })
}

/// Check the arguments of a `TestCase.generate_duration` call without
/// drawing.
#[pyfunction]
pub(crate) fn validate_duration(
    min_value: PyDuration,
    max_value: PyDuration,
    precision: u32,
) -> PyResult<()> {
    with_ctx(|ctx| {
        check(unsafe {
            c::hegel_validate_duration(
                ctx,
                duration_to_c(min_value),
                duration_to_c(max_value),
                precision,
            )
        })
    })
}

/// The canonical text of a calendar span, given as its ten unit values
/// from years to nanoseconds.
#[pyfunction]
pub(crate) fn format_span(span: PySpan) -> PyResult<String> {
    let value = c::hegel_span_t {
        years: span.0,
        months: span.1,
        weeks: span.2,
        days: span.3,
        hours: span.4,
        minutes: span.5,
        seconds: span.6,
        milliseconds: span.7,
        microseconds: span.8,
        nanoseconds: span.9,
    };
    let mut out = c::hegel_owned_string_t::default();
    with_ctx(|ctx| {
        check(unsafe { c::hegel_format_span(ctx, &value, &mut out) })?;
        let text = string_from(out);
        let _ = unsafe { c::hegel_owned_string_free(ctx, &mut out) };
        Ok(text)
    })
}

/// What `source` (the process's default when `None`) knows about the
/// zone `name`, as `(primary_name, is_link, rank, tags)`, or `None` for a
/// name it lacks.
#[pyfunction]
#[pyo3(signature = (name, source=None))]
pub(crate) fn timezone_info(
    py: Python<'_>,
    name: &str,
    source: Option<&ZoneSource>,
) -> PyResult<Option<(String, bool, u16, u32)>> {
    let source = SendPtr(source.map_or(ptr::null(), ZoneSource::raw) as *mut c::HegelZoneSource);
    let (rc, out) = py.detach(move || {
        let mut out = c::hegel_timezone_info_t {
            known: false,
            primary_name: c::hegel_string_t::default(),
            is_link: false,
            rank: 0,
            tags: 0,
        };
        let rc = with_ctx(|ctx| unsafe {
            c::hegel_timezone_info_in(ctx, source.get(), hstr(name), &mut out)
        });
        (
            rc,
            out.known.then(|| {
                (
                    string_from(out.primary_name),
                    out.is_link,
                    out.rank,
                    out.tags,
                )
            }),
        )
    });
    check(rc)?;
    Ok(out)
}

/// Classify the wall time `civil` in the zone `name` of `source` (the
/// process's default when `None`), as `(kind, offset_before, offset_after,
/// offset_seconds, offset_known, timestamp)`.
#[pyfunction]
#[pyo3(signature = (name, civil, source=None))]
pub(crate) fn timezone_resolve(
    py: Python<'_>,
    name: &str,
    civil: PyDateTime,
    source: Option<&ZoneSource>,
) -> PyResult<(u32, i32, i32, i32, bool, PyTimestamp)> {
    let source = SendPtr(source.map_or(ptr::null(), ZoneSource::raw) as *mut c::HegelZoneSource);
    let (rc, out) = py.detach(move || {
        let mut out = c::hegel_timezone_resolution_t {
            kind: c::hegel_timezone_kind_t::HEGEL_TIMEZONE_KIND_UNIQUE,
            offset_before: 0,
            offset_after: 0,
            offset_seconds: 0,
            offset_known: false,
            timestamp: timestamp_to_c((0, 0)),
        };
        let rc = with_ctx(|ctx| unsafe {
            c::hegel_timezone_resolve_in(
                ctx,
                source.get(),
                hstr(name),
                datetime_to_c(civil),
                &mut out,
            )
        });
        (rc, out)
    });
    check(rc)?;
    Ok((
        out.kind as u32,
        out.offset_before,
        out.offset_after,
        out.offset_seconds,
        out.offset_known,
        timestamp_from_c(out.timestamp),
    ))
}

/// The offset the zone `name` of `source` (the process's default when
/// `None`) has in force at the instant `timestamp`, as `(offset_seconds,
/// offset_known, is_dst, abbreviation)`.
#[pyfunction]
#[pyo3(signature = (name, timestamp, source=None))]
pub(crate) fn timezone_offset_at(
    py: Python<'_>,
    name: &str,
    timestamp: PyTimestamp,
    source: Option<&ZoneSource>,
) -> PyResult<(i32, bool, bool, String)> {
    let source = SendPtr(source.map_or(ptr::null(), ZoneSource::raw) as *mut c::HegelZoneSource);
    let (rc, out) = py.detach(move || {
        let mut out = c::hegel_timezone_offset_t {
            offset_seconds: 0,
            offset_known: false,
            is_dst: false,
            abbreviation: c::hegel_string_t::default(),
        };
        let rc = with_ctx(|ctx| unsafe {
            c::hegel_timezone_offset_at_in(
                ctx,
                source.get(),
                hstr(name),
                timestamp_to_c(timestamp),
                &mut out,
            )
        });
        (
            rc,
            (
                out.offset_seconds,
                out.offset_known,
                out.is_dst,
                string_from(out.abbreviation),
            ),
        )
    });
    check(rc)?;
    Ok(out)
}
