use std::fmt::Display;
use std::sync::OnceLock;

use super::{Generator, PrintableGenerator, TestCase, label_from_name};
use crate::datetime::{
    Ambiguous, Date, DateTime, Granularity, Imaginary, MAX_INSTANT, MAX_YEAR, MIN_INSTANT,
    MIN_YEAR, Month, Precision, SignedDuration, Span, SpanSign, SpanUnit, Time, TimeZone,
    Timestamp, UtcOffset, Weekday, ZonedDateTime, date_from_c, date_to_c, datetime_from_c,
    datetime_to_c, info, kind_from_c, offset_from_c, time_from_c, time_to_c, timestamp_from_c,
    timestamp_to_c,
};
use crate::ffi;
use crate::ffi::sys as hegel_c;
use crate::pretty::PrettyPrinter;
use crate::test_case::invalid_argument;

const DATE_LABEL: u64 = label_from_name("hegel.datetime.dates");
const TIME_LABEL: u64 = label_from_name("hegel.datetime.times");
const DATETIME_LABEL: u64 = label_from_name("hegel.datetime.datetimes");
const TIMESTAMP_LABEL: u64 = label_from_name("hegel.datetime.timestamps");
const UTC_OFFSET_LABEL: u64 = label_from_name("hegel.datetime.utc_offsets");
const TIME_ZONE_LABEL: u64 = label_from_name("hegel.datetime.time_zones");
const ZONED_DATETIME_LABEL: u64 = label_from_name("hegel.datetime.zoned_datetimes");
const SIGNED_DURATION_LABEL: u64 = label_from_name("hegel.datetime.signed_durations");
const SPAN_LABEL: u64 = label_from_name("hegel.datetime.spans");

/// The first wall-clock time of the engine's civil range.
pub(crate) const MIN_DATETIME: DateTime = DateTime {
    date: Date {
        year: MIN_YEAR,
        month: 1,
        day: 1,
    },
    time: Time {
        hour: 0,
        minute: 0,
        second: 0,
        nanosecond: 0,
    },
};

/// The last wall-clock time of the engine's civil range.
pub(crate) const MAX_DATETIME: DateTime = DateTime {
    date: Date {
        year: MAX_YEAR,
        month: 12,
        day: 31,
    },
    time: Time {
        hour: 23,
        minute: 59,
        second: 59,
        nanosecond: 999_999_999,
    },
};

const MIN_TIMESTAMP: Timestamp = Timestamp {
    second: MIN_INSTANT,
    nanosecond: 0,
};

const MAX_TIMESTAMP: Timestamp = Timestamp {
    second: MAX_INSTANT,
    nanosecond: 999_999_999,
};

pub(crate) fn precision_to_c(precision: Precision) -> u32 {
    use hegel_c::hegel_precision_t as c;
    (match precision {
        Precision::Nanoseconds => c::HEGEL_PRECISION_NANOSECONDS,
        Precision::Microseconds => c::HEGEL_PRECISION_MICROSECONDS,
        Precision::Milliseconds => c::HEGEL_PRECISION_MILLISECONDS,
        Precision::Seconds => c::HEGEL_PRECISION_SECONDS,
        Precision::Minutes => c::HEGEL_PRECISION_MINUTES,
        Precision::Hours => c::HEGEL_PRECISION_HOURS,
    }) as u32
}

fn granularity_to_c(granularity: Granularity) -> u32 {
    use hegel_c::hegel_offset_granularity_t as c;
    (match granularity {
        Granularity::Hours => c::HEGEL_OFFSET_GRANULARITY_HOURS,
        Granularity::QuarterHours => c::HEGEL_OFFSET_GRANULARITY_QUARTER_HOURS,
        Granularity::Minutes => c::HEGEL_OFFSET_GRANULARITY_MINUTES,
        Granularity::Seconds => c::HEGEL_OFFSET_GRANULARITY_SECONDS,
    }) as u32
}

fn imaginary_to_c(imaginary: Imaginary) -> u32 {
    use hegel_c::hegel_imaginary_t as c;
    (match imaginary {
        Imaginary::Allow => c::HEGEL_IMAGINARY_ALLOW,
        Imaginary::Avoid => c::HEGEL_IMAGINARY_AVOID,
    }) as u32
}

fn ambiguous_to_c(ambiguous: Ambiguous) -> u32 {
    use hegel_c::hegel_ambiguous_t as c;
    (match ambiguous {
        Ambiguous::Both => c::HEGEL_AMBIGUOUS_BOTH,
        Ambiguous::Earlier => c::HEGEL_AMBIGUOUS_EARLIER,
        Ambiguous::Later => c::HEGEL_AMBIGUOUS_LATER,
    }) as u32
}

fn span_sign_to_c(sign: SpanSign) -> u32 {
    use hegel_c::hegel_span_sign_t as c;
    (match sign {
        SpanSign::Uniform => c::HEGEL_SPAN_SIGN_UNIFORM,
        SpanSign::Mixed => c::HEGEL_SPAN_SIGN_MIXED,
        SpanSign::NonNegative => c::HEGEL_SPAN_SIGN_NONNEGATIVE,
    }) as u32
}

fn span_unit_to_c(unit: SpanUnit) -> u32 {
    use hegel_c::hegel_span_unit_t as c;
    (match unit {
        SpanUnit::Years => c::HEGEL_SPAN_UNIT_YEARS,
        SpanUnit::Months => c::HEGEL_SPAN_UNIT_MONTHS,
        SpanUnit::Weeks => c::HEGEL_SPAN_UNIT_WEEKS,
        SpanUnit::Days => c::HEGEL_SPAN_UNIT_DAYS,
        SpanUnit::Hours => c::HEGEL_SPAN_UNIT_HOURS,
        SpanUnit::Minutes => c::HEGEL_SPAN_UNIT_MINUTES,
        SpanUnit::Seconds => c::HEGEL_SPAN_UNIT_SECONDS,
        SpanUnit::Milliseconds => c::HEGEL_SPAN_UNIT_MILLISECONDS,
        SpanUnit::Microseconds => c::HEGEL_SPAN_UNIT_MICROSECONDS,
        SpanUnit::Nanoseconds => c::HEGEL_SPAN_UNIT_NANOSECONDS,
    }) as u32
}

/// Report a configuration the engine rejected as a usage error.
fn configured(result: Result<(), String>) {
    result.unwrap_or_else(|message| invalid_argument!("{message}"));
}

/// Draw from `generator` and print the value's `Display` form, the RFC
/// 9557 spelling every frontend prints a counterexample in.
pub(crate) fn draw_and_print_display<T: Display>(
    generator: &impl Generator<T>,
    tc: &TestCase,
    printer: &mut PrettyPrinter,
) -> T {
    let value = generator.do_draw(tc);
    printer.text(&value.to_string());
    value
}

/// Generator for [`Date`] values. Created by [`dates()`].
pub struct DateGenerator {
    min_value: Date,
    max_value: Date,
    weekdays: u32,
    months: u32,
    options: OnceLock<ffi::DateOptions>,
}

const ALL_WEEKDAYS: u32 = 0x7f;
const ALL_MONTHS: u32 = 0xfff;

impl DateGenerator {
    /// Set the minimum date (inclusive).
    pub fn min_value(mut self, min: Date) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum date (inclusive).
    pub fn max_value(mut self, max: Date) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only dates on these weekdays. A drawn day on another weekday
    /// is moved to the nearest allowed day inside the bounds rather than
    /// rejected, so the draw never filters.
    pub fn weekdays(mut self, weekdays: impl IntoIterator<Item = Weekday>) -> Self {
        self.weekdays = weekdays
            .into_iter()
            .fold(0, |mask, day| mask | 1 << day as u32);
        self.options = OnceLock::new();
        self
    }

    /// Draw only dates in these months, moving a drawn day in another
    /// month to the nearest allowed day inside the bounds as
    /// [`weekdays`](Self::weekdays) does.
    pub fn months(mut self, months: impl IntoIterator<Item = Month>) -> Self {
        self.months = months
            .into_iter()
            .fold(0, |mask, month| mask | 1 << month as u32);
        self.options = OnceLock::new();
        self
    }

    fn options(&self) -> Option<&ffi::DateOptions> {
        if self.weekdays == ALL_WEEKDAYS && self.months == ALL_MONTHS {
            return None;
        }
        Some(self.options.get_or_init(|| {
            let mut options = ffi::DateOptions::new();
            configured(options.set_weekdays(self.weekdays));
            configured(options.set_months(self.months));
            options
        }))
    }
}

impl Generator<Date> for DateGenerator {
    fn label(&self) -> u64 {
        DATE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Date {
        if self.min_value > self.max_value {
            invalid_argument!("Cannot have max_value < min_value");
        }
        date_from_c(tc.generate_date(
            date_to_c(self.min_value),
            date_to_c(self.max_value),
            self.options(),
        ))
    }
}

impl PrintableGenerator<Date> for DateGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Date {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`Date`] values.
///
/// By default any date of the engine's range, years ±999,999, on any
/// weekday and in any month; shrinks toward 2000-01-01. See
/// [`DateGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::{Date, Weekday};
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = Date::new(2024, 1, 1).unwrap();
///     let d = tc.draw(gs::dates().min_value(min).weekdays([Weekday::Saturday, Weekday::Sunday]));
///     assert!(d >= min);
/// }
/// ```
pub fn dates() -> DateGenerator {
    DateGenerator {
        min_value: MIN_DATETIME.date,
        max_value: MAX_DATETIME.date,
        weekdays: ALL_WEEKDAYS,
        months: ALL_MONTHS,
        options: OnceLock::new(),
    }
}

/// Generator for [`Time`] values. Created by [`times()`].
pub struct TimeGenerator {
    min_value: Time,
    max_value: Time,
    precision: Precision,
}

impl TimeGenerator {
    /// Set the minimum time (inclusive).
    pub fn min_value(mut self, min: Time) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum time (inclusive).
    pub fn max_value(mut self, max: Time) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only times that are a whole number of `precision` units
    /// since midnight.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

impl Generator<Time> for TimeGenerator {
    fn label(&self) -> u64 {
        TIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Time {
        if self.min_value > self.max_value {
            invalid_argument!("Cannot have max_value < min_value");
        }
        time_from_c(tc.generate_time(
            time_to_c(self.min_value),
            time_to_c(self.max_value),
            precision_to_c(self.precision),
        ))
    }
}

impl PrintableGenerator<Time> for TimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Time {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`Time`] values.
///
/// By default any time of day at nanosecond precision; shrinks toward
/// midnight. See [`TimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::Precision;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let t = tc.draw(gs::times().precision(Precision::Seconds));
///     assert_eq!(t.nanosecond, 0);
/// }
/// ```
pub fn times() -> TimeGenerator {
    TimeGenerator {
        min_value: MIN_DATETIME.time,
        max_value: MAX_DATETIME.time,
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`DateTime`] values. Created by [`datetimes()`].
pub struct DateTimeGenerator {
    min_value: DateTime,
    max_value: DateTime,
    precision: Precision,
}

impl DateTimeGenerator {
    /// Set the minimum datetime (inclusive).
    pub fn min_value(mut self, min: DateTime) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum datetime (inclusive).
    pub fn max_value(mut self, max: DateTime) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only datetimes whose time is a whole number of `precision`
    /// units since midnight.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

impl Generator<DateTime> for DateTimeGenerator {
    fn label(&self) -> u64 {
        DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> DateTime {
        if self.min_value > self.max_value {
            invalid_argument!("Cannot have max_value < min_value");
        }
        datetime_from_c(tc.generate_datetime(
            datetime_to_c(self.min_value),
            datetime_to_c(self.max_value),
            precision_to_c(self.precision),
        ))
    }
}

impl PrintableGenerator<DateTime> for DateTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> DateTime {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`DateTime`] values.
///
/// By default any wall-clock time of the engine's range, years ±999,999,
/// at nanosecond precision; shrinks toward 2000-01-01T00:00:00. See
/// [`DateTimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::{Date, DateTime, Time};
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = DateTime::new(Date::new(2024, 1, 1).unwrap(), Time::new(0, 0, 0, 0).unwrap()).unwrap();
///     let dt = tc.draw(gs::datetimes().min_value(min));
///     assert!(dt >= min);
/// }
/// ```
pub fn datetimes() -> DateTimeGenerator {
    DateTimeGenerator {
        min_value: MIN_DATETIME,
        max_value: MAX_DATETIME,
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`Timestamp`] values. Created by [`timestamps()`].
pub struct TimestampGenerator {
    min_value: Timestamp,
    max_value: Timestamp,
    precision: Precision,
}

impl TimestampGenerator {
    /// Set the minimum instant (inclusive).
    pub fn min_value(mut self, min: Timestamp) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum instant (inclusive).
    pub fn max_value(mut self, max: Timestamp) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only instants that are a whole number of `precision` units
    /// since the Unix epoch.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

impl Generator<Timestamp> for TimestampGenerator {
    fn label(&self) -> u64 {
        TIMESTAMP_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Timestamp {
        if self.min_value > self.max_value {
            invalid_argument!("Cannot have max_value < min_value");
        }
        timestamp_from_c(tc.generate_timestamp(
            timestamp_to_c(self.min_value),
            timestamp_to_c(self.max_value),
            precision_to_c(self.precision),
        ))
    }
}

impl PrintableGenerator<Timestamp> for TimestampGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Timestamp {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`Timestamp`] values.
///
/// By default any instant of the engine's range, years ±999,999, at
/// nanosecond precision; shrinks toward 2000-01-01T00:00:00Z. One draw in
/// twenty on average targets an interesting instant: the Unix epoch, the
/// 32-bit and 64-bit boundaries, a leap second's or a time zone
/// transition's. See [`TimestampGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::Timestamp;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let epoch = Timestamp::new(0, 0).unwrap();
///     let t = tc.draw(gs::timestamps().min_value(epoch));
///     assert!(t >= epoch);
/// }
/// ```
pub fn timestamps() -> TimestampGenerator {
    TimestampGenerator {
        min_value: MIN_TIMESTAMP,
        max_value: MAX_TIMESTAMP,
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`UtcOffset`] values. Created by [`utc_offsets()`].
pub struct UtcOffsetGenerator {
    min_value: UtcOffset,
    max_value: UtcOffset,
    granularity: Granularity,
    unknown: bool,
}

impl UtcOffsetGenerator {
    /// Set the minimum offset (inclusive), which must be known.
    pub fn min_value(mut self, min: UtcOffset) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum offset (inclusive), which must be known.
    pub fn max_value(mut self, max: UtcOffset) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only offsets that are a whole number of `granularity` units.
    pub fn granularity(mut self, granularity: Granularity) -> Self {
        self.granularity = granularity;
        self
    }

    /// Whether the draw may yield [`UtcOffset::Unknown`], RFC 3339's
    /// `-00:00` (off by default). Shrinks to a known offset first.
    pub fn unknown(mut self, unknown: bool) -> Self {
        self.unknown = unknown;
        self
    }
}

fn known_bound(what: &str, bound: UtcOffset) -> i32 {
    match bound {
        UtcOffset::Known(seconds) => seconds,
        UtcOffset::Unknown => invalid_argument!("{what} must be a known offset"),
    }
}

impl Generator<UtcOffset> for UtcOffsetGenerator {
    fn label(&self) -> u64 {
        UTC_OFFSET_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> UtcOffset {
        let min = known_bound("min_value", self.min_value);
        let max = known_bound("max_value", self.max_value);
        if min > max {
            invalid_argument!("Cannot have max_value < min_value");
        }
        let o = tc.generate_offset(min, max, granularity_to_c(self.granularity), self.unknown);
        offset_from_c(o.seconds, o.known)
    }
}

impl PrintableGenerator<UtcOffset> for UtcOffsetGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> UtcOffset {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`UtcOffset`] values.
///
/// By default any known offset within ±25:59:59 at second granularity;
/// shrinks toward `+00:00`. One draw in twenty on average targets an
/// offset some time zone has used, so `+05:45` and `-00:44:30` appear.
/// See [`UtcOffsetGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::{Granularity, UtcOffset};
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let o = tc.draw(gs::utc_offsets().granularity(Granularity::Minutes));
///     let UtcOffset::Known(seconds) = o else { unreachable!() };
///     assert_eq!(seconds % 60, 0);
/// }
/// ```
pub fn utc_offsets() -> UtcOffsetGenerator {
    UtcOffsetGenerator {
        min_value: UtcOffset::Known(-crate::datetime::MAX_OFFSET),
        max_value: UtcOffset::Known(crate::datetime::MAX_OFFSET),
        granularity: Granularity::Seconds,
        unknown: false,
    }
}

/// Generator for [`TimeZone`] values. Created by [`time_zones()`].
pub struct TimeZoneGenerator {
    names: Option<Vec<String>>,
    links: bool,
    legacy: bool,
    unknown: bool,
    opaque: bool,
    spec: OnceLock<ffi::TimeZoneSpec>,
}

impl TimeZoneGenerator {
    /// Draw only these names, each exactly as the IANA database spells it
    /// or `Etc/Unknown`. The [`links`](Self::links) and
    /// [`legacy`](Self::legacy) flags do not apply to an explicit list.
    pub fn names(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.names = Some(names.into_iter().map(Into::into).collect());
        self.spec = OnceLock::new();
        self
    }

    /// Whether the default set includes link names such as
    /// `Asia/Calcutta` (on by default).
    pub fn links(mut self, links: bool) -> Self {
        self.links = links;
        self.spec = OnceLock::new();
        self
    }

    /// Whether the default set includes legacy names such as `EST5EDT`
    /// and `US/Eastern` (off by default).
    pub fn legacy(mut self, legacy: bool) -> Self {
        self.legacy = legacy;
        self.spec = OnceLock::new();
        self
    }

    /// Whether `Etc/Unknown`, the zone with a known instant and an unknown
    /// offset, is drawn (off by default).
    pub fn unknown(mut self, unknown: bool) -> Self {
        self.unknown = unknown;
        self.spec = OnceLock::new();
        self
    }

    /// Whether a name in an explicit [`names`](Self::names) list that the
    /// engine's database lacks is accepted as an opaque zone (off by
    /// default): one only the caller's database knows, drawn like any
    /// other name but evaluated like `Etc/Unknown`, with its civil time
    /// read as UTC and an unknown offset.
    pub fn opaque(mut self, opaque: bool) -> Self {
        self.opaque = opaque;
        self.spec = OnceLock::new();
        self
    }

    pub(crate) fn spec(&self) -> &ffi::TimeZoneSpec {
        self.spec.get_or_init(|| {
            let mut spec = ffi::TimeZoneSpec::new();
            configured(spec.set_opaque(self.opaque));
            if let Some(names) = &self.names {
                configured(spec.set_names(names));
            }
            configured(spec.set_links(self.links));
            configured(spec.set_legacy(self.legacy));
            configured(spec.set_unknown(self.unknown));
            spec
        })
    }
}

fn time_zone_named(name: String) -> TimeZone {
    let primary_name = info(&name).map_or_else(|| name.clone(), |info| info.primary_name);
    TimeZone { name, primary_name }
}

impl Generator<TimeZone> for TimeZoneGenerator {
    fn label(&self) -> u64 {
        TIME_ZONE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> TimeZone {
        time_zone_named(tc.generate_timezone(self.spec()))
    }
}

impl PrintableGenerator<TimeZone> for TimeZoneGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> TimeZone {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`TimeZone`] values.
///
/// By default any name of the engine's bundled IANA database, link names
/// in and legacy names out; shrinks down a ladder of names to `UTC` when
/// it is allowed, otherwise to the lowest-ranked allowed name. Half
/// the draws are uniform over the allowed names and half weighted by the
/// zones' features, so a zone with a rare feature (a fractional offset, a
/// skipped day, negative DST) is drawn at a rate a short run reaches. See
/// [`TimeZoneGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zone = tc.draw(gs::time_zones().names(["Europe/London", "Asia/Calcutta"]));
///     assert!(zone.primary_name == "Europe/London" || zone.primary_name == "Asia/Kolkata");
/// }
/// ```
pub fn time_zones() -> TimeZoneGenerator {
    TimeZoneGenerator {
        names: None,
        links: true,
        legacy: false,
        unknown: false,
        opaque: false,
        spec: OnceLock::new(),
    }
}

enum ZonedBounds {
    Civil(DateTime, DateTime),
    Instant(Timestamp, Timestamp),
}

/// Generator for [`ZonedDateTime`] values. Created by
/// [`zoned_datetimes()`].
pub struct ZonedDateTimeGenerator {
    zones: TimeZoneGenerator,
    bounds: ZonedBounds,
    precision: Precision,
    imaginary: Imaginary,
    ambiguous: Ambiguous,
    spec: OnceLock<ffi::ZonedSpec>,
}

impl ZonedDateTimeGenerator {
    /// Draw the zone from `zones` rather than from every bundled name.
    pub fn time_zones(mut self, zones: TimeZoneGenerator) -> Self {
        self.zones = zones;
        self
    }

    /// Set the minimum wall-clock time (inclusive), in whatever zone is
    /// drawn. Replaces instant bounds.
    pub fn min_value(mut self, min: DateTime) -> Self {
        let max = match self.bounds {
            ZonedBounds::Civil(_, max) => max,
            ZonedBounds::Instant(..) => MAX_DATETIME,
        };
        self.bounds = ZonedBounds::Civil(min, max);
        self.spec = OnceLock::new();
        self
    }

    /// Set the maximum wall-clock time (inclusive), in whatever zone is
    /// drawn. Replaces instant bounds.
    pub fn max_value(mut self, max: DateTime) -> Self {
        let min = match self.bounds {
            ZonedBounds::Civil(min, _) => min,
            ZonedBounds::Instant(..) => MIN_DATETIME,
        };
        self.bounds = ZonedBounds::Civil(min, max);
        self.spec = OnceLock::new();
        self
    }

    /// Set the minimum instant (inclusive). Replaces wall-clock bounds.
    pub fn min_instant(mut self, min: Timestamp) -> Self {
        let max = match self.bounds {
            ZonedBounds::Instant(_, max) => max,
            ZonedBounds::Civil(..) => MAX_TIMESTAMP,
        };
        self.bounds = ZonedBounds::Instant(min, max);
        self.spec = OnceLock::new();
        self
    }

    /// Set the maximum instant (inclusive). Replaces wall-clock bounds.
    pub fn max_instant(mut self, max: Timestamp) -> Self {
        let min = match self.bounds {
            ZonedBounds::Instant(min, _) => min,
            ZonedBounds::Civil(..) => MIN_TIMESTAMP,
        };
        self.bounds = ZonedBounds::Instant(min, max);
        self.spec = OnceLock::new();
        self
    }

    /// Draw only wall-clock times that are a whole number of `precision`
    /// units since midnight.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self.spec = OnceLock::new();
        self
    }

    /// What to do with a wall time the clocks skipped
    /// ([`Imaginary::Allow`] by default).
    pub fn imaginary(mut self, imaginary: Imaginary) -> Self {
        self.imaginary = imaginary;
        self.spec = OnceLock::new();
        self
    }

    /// Which instant a wall time the clocks repeated means
    /// ([`Ambiguous::Both`] by default).
    pub fn ambiguous(mut self, ambiguous: Ambiguous) -> Self {
        self.ambiguous = ambiguous;
        self.spec = OnceLock::new();
        self
    }

    fn spec(&self) -> &ffi::ZonedSpec {
        self.spec.get_or_init(|| {
            let mut spec = ffi::ZonedSpec::new();
            configured(spec.set_precision(precision_to_c(self.precision)));
            match self.bounds {
                ZonedBounds::Civil(min, max) => {
                    if min > max {
                        invalid_argument!("Cannot have max_value < min_value");
                    }
                    configured(spec.set_civil_bounds(datetime_to_c(min), datetime_to_c(max)));
                }
                ZonedBounds::Instant(min, max) => {
                    if min > max {
                        invalid_argument!("Cannot have max_instant < min_instant");
                    }
                    configured(spec.set_instant_bounds(timestamp_to_c(min), timestamp_to_c(max)));
                }
            }
            configured(spec.set_imaginary(imaginary_to_c(self.imaginary)));
            configured(spec.set_ambiguous(ambiguous_to_c(self.ambiguous)));
            spec
        })
    }
}

impl Generator<ZonedDateTime> for ZonedDateTimeGenerator {
    fn label(&self) -> u64 {
        ZONED_DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> ZonedDateTime {
        let (z, name) = tc.generate_zoned_datetime(self.zones.spec(), self.spec());
        ZonedDateTime {
            zone: time_zone_named(name),
            civil: datetime_from_c(z.civil),
            kind: kind_from_c(z.kind),
            offset_before: z.offset_before,
            offset_after: z.offset_after,
            fold: z.fold,
            offset: offset_from_c(z.offset_seconds, z.offset_known),
            timestamp: timestamp_from_c(z.timestamp),
        }
    }
}

impl PrintableGenerator<ZonedDateTime> for ZonedDateTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> ZonedDateTime {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`ZonedDateTime`] values: a zone, a wall-clock time in it
/// and, when the clocks repeated that time, which repeat is meant.
///
/// By default any zone [`time_zones()`] draws and any wall-clock time of
/// the engine's range at nanosecond precision, with skipped wall times
/// reported as [`Kind::Gap`](crate::datetime::Kind::Gap) and both repeats
/// of a fold drawn; shrinks down the zone ladder, then toward
/// 2000-01-01T00:00:00, preferring a unique wall time to a repeated one
/// (so a shrink whose lower bound lies in a fold ends just after the
/// fold). One draw in twenty on average targets a transition
/// of the zone, which is how gaps, folds and the exact boundary instants
/// are reached. Build a native value from `civil`, the zone name and
/// `fold` (or, for a gap, from `timestamp`), never from the offsets, so
/// that a newer database on your side changes what a value means but
/// never makes it invalid. See [`ZonedDateTimeGenerator`] for builder
/// methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::{Imaginary, Kind};
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let z = tc.draw(gs::zoned_datetimes().imaginary(Imaginary::Avoid));
///     assert_ne!(z.kind, Kind::Gap);
/// }
/// ```
pub fn zoned_datetimes() -> ZonedDateTimeGenerator {
    ZonedDateTimeGenerator {
        zones: time_zones(),
        bounds: ZonedBounds::Civil(MIN_DATETIME, MAX_DATETIME),
        precision: Precision::Nanoseconds,
        imaginary: Imaginary::Allow,
        ambiguous: Ambiguous::Both,
        spec: OnceLock::new(),
    }
}

/// Generator for [`SignedDuration`] values. Created by
/// [`signed_durations()`].
pub struct SignedDurationGenerator {
    min_value: SignedDuration,
    max_value: SignedDuration,
    precision: Precision,
}

impl SignedDurationGenerator {
    /// Set the minimum duration (inclusive).
    pub fn min_value(mut self, min: SignedDuration) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum duration (inclusive).
    pub fn max_value(mut self, max: SignedDuration) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only durations that are a whole number of `precision` units.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

pub(crate) fn duration_to_c(d: SignedDuration) -> hegel_c::hegel_duration_t {
    hegel_c::hegel_duration_t {
        seconds: d.seconds,
        nanoseconds: d.nanoseconds,
    }
}

pub(crate) fn duration_from_c(d: hegel_c::hegel_duration_t) -> SignedDuration {
    SignedDuration {
        seconds: d.seconds,
        nanoseconds: d.nanoseconds,
    }
}

impl Generator<SignedDuration> for SignedDurationGenerator {
    fn label(&self) -> u64 {
        SIGNED_DURATION_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> SignedDuration {
        if self.min_value > self.max_value {
            invalid_argument!("Cannot have max_value < min_value");
        }
        duration_from_c(tc.generate_duration(
            duration_to_c(self.min_value),
            duration_to_c(self.max_value),
            precision_to_c(self.precision),
        ))
    }
}

impl PrintableGenerator<SignedDuration> for SignedDurationGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> SignedDuration {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`SignedDuration`] values.
///
/// By default any duration of ±`i64` seconds at nanosecond precision;
/// shrinks toward zero. One draw in twenty on average targets a bound or
/// a duration such as one unit, 24 hours or the `i32` seconds limit. See
/// [`SignedDurationGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::SignedDuration;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zero = SignedDuration::new(0, 0).unwrap();
///     let d = tc.draw(gs::signed_durations().min_value(zero));
///     assert!(d >= zero);
/// }
/// ```
pub fn signed_durations() -> SignedDurationGenerator {
    SignedDurationGenerator {
        min_value: SignedDuration {
            seconds: i64::MIN,
            nanoseconds: -999_999_999,
        },
        max_value: SignedDuration {
            seconds: i64::MAX,
            nanoseconds: 999_999_999,
        },
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`Span`] values. Created by [`spans()`].
pub struct SpanGenerator {
    bounds: Option<[Option<(i64, i64)>; 10]>,
    sign: SpanSign,
    spec: OnceLock<ffi::SpanSpec>,
}

impl SpanGenerator {
    /// Draw `unit` within `[min, max]` (inclusive). The first call
    /// replaces the default of every unit with just this one; later calls
    /// add units, so a span of days and hours is
    /// `spans().unit_bounds(SpanUnit::Days, 0, 30).unit_bounds(SpanUnit::Hours, 0, 23)`.
    /// A unit never enabled is always 0.
    pub fn unit_bounds(mut self, unit: SpanUnit, min: i64, max: i64) -> Self {
        let bounds = self.bounds.get_or_insert([None; 10]);
        bounds[unit as usize] = Some((min, max));
        self.spec = OnceLock::new();
        self
    }

    /// How the units' signs are drawn ([`SpanSign::Uniform`] by default).
    pub fn sign(mut self, sign: SpanSign) -> Self {
        self.sign = sign;
        self.spec = OnceLock::new();
        self
    }

    fn spec(&self) -> &ffi::SpanSpec {
        self.spec.get_or_init(|| {
            let mut spec = ffi::SpanSpec::new();
            configured(spec.set_sign(span_sign_to_c(self.sign)));
            let min = if self.sign == SpanSign::NonNegative {
                0
            } else {
                -i64::MAX
            };
            let bounds = self.bounds.unwrap_or([Some((min, i64::MAX)); 10]);
            for (unit, bound) in SpanUnit::ALL.into_iter().zip(bounds) {
                if let Some((min, max)) = bound {
                    configured(spec.set_unit_bounds(span_unit_to_c(unit), min, max));
                }
            }
            spec
        })
    }
}

impl Generator<Span> for SpanGenerator {
    fn label(&self) -> u64 {
        SPAN_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Span {
        let s = tc.generate_span(self.spec());
        Span {
            years: s.years,
            months: s.months,
            weeks: s.weeks,
            days: s.days,
            hours: s.hours,
            minutes: s.minutes,
            seconds: s.seconds,
            milliseconds: s.milliseconds,
            microseconds: s.microseconds,
            nanoseconds: s.nanoseconds,
        }
    }
}

impl PrintableGenerator<Span> for SpanGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Span {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`Span`] values.
///
/// By default every unit from years to nanoseconds within ±`i64::MAX`
/// (or `[0, i64::MAX]` under [`SpanSign::NonNegative`]), all of one drawn
/// sign; shrinks toward the zero span. See
/// [`SpanGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::SpanUnit;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let s = tc.draw(gs::spans().unit_bounds(SpanUnit::Days, 0, 30));
///     assert!((0..=30).contains(&s.days) && s.years == 0);
/// }
/// ```
pub fn spans() -> SpanGenerator {
    SpanGenerator {
        bounds: None,
        sign: SpanSign::Uniform,
        spec: OnceLock::new(),
    }
}
