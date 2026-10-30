use std::fmt::Display;
use std::sync::OnceLock;

use jiff::civil::{Date, DateTime, Era, ISOWeekDate, Time, Weekday};
use jiff::tz::{Offset, TimeZone};
use jiff::{SignedDuration, Span, Timestamp, Zoned};
use std::sync::LazyLock;

use crate::datetime::{
    self as dt, Ambiguous, Granularity, Kind, Month, Precision, SpanSign, SpanUnit, Tag, UtcOffset,
    info,
};
use crate::generators::{
    self as gs, BoolGenerator, Generator, IntegerGenerator, Mapped, PrintableGenerator, TestCase,
    label_from_name,
};
use crate::pretty::PrettyPrinter;
use crate::test_case::invalid_argument;

static DATE_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.dates"));
static TIME_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.times"));
static DATETIME_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.datetimes"));
static TIMESTAMP_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.timestamps"));
static SPAN_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.spans"));
static SIGNED_DURATION_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.jiff.signed_durations"));
static OFFSET_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.offsets"));
static ZONED_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.zoneds"));
static TIME_ZONE_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.timezones"));

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// Draw from `generator` and print the value's `Display` form, jiff's
/// RFC 9557 spelling, which is also the engine's.
fn draw_and_print_display<T: Display>(
    generator: &impl Generator<T>,
    tc: &TestCase,
    printer: &mut PrettyPrinter,
) -> T {
    let value = generator.do_draw(tc);
    printer.text(&value.to_string());
    value
}

fn engine_date(d: Date) -> dt::Date {
    dt::Date {
        year: i32::from(d.year()),
        month: d.month() as u8,
        day: d.day() as u8,
    }
}

fn jiff_date(d: dt::Date) -> Date {
    Date::new(d.year as i16, d.month as i8, d.day as i8).unwrap()
}

fn engine_time(t: Time) -> dt::Time {
    dt::Time {
        hour: t.hour() as u8,
        minute: t.minute() as u8,
        second: t.second() as u8,
        nanosecond: t.subsec_nanosecond() as u32,
    }
}

fn jiff_time(t: dt::Time) -> Time {
    Time::new(
        t.hour as i8,
        t.minute as i8,
        t.second as i8,
        t.nanosecond as i32,
    )
    .unwrap()
}

pub(super) fn engine_datetime(d: DateTime) -> dt::DateTime {
    dt::DateTime {
        date: engine_date(d.date()),
        time: engine_time(d.time()),
    }
}

pub(super) fn jiff_datetime(d: dt::DateTime) -> DateTime {
    DateTime::from_parts(jiff_date(d.date), jiff_time(d.time))
}

fn engine_timestamp(t: Timestamp) -> dt::Timestamp {
    let nanos = t.as_nanosecond();
    dt::Timestamp {
        second: nanos.div_euclid(NANOS_PER_SECOND) as i64,
        nanosecond: nanos.rem_euclid(NANOS_PER_SECOND) as u32,
    }
}

fn jiff_timestamp(t: dt::Timestamp) -> Timestamp {
    Timestamp::new(t.second, t.nanosecond as i32).unwrap()
}

fn engine_duration(d: SignedDuration) -> dt::SignedDuration {
    dt::SignedDuration {
        seconds: d.as_secs(),
        nanoseconds: d.subsec_nanos(),
    }
}

fn jiff_duration(d: dt::SignedDuration) -> SignedDuration {
    SignedDuration::new(d.seconds, d.nanoseconds)
}

fn engine_weekday(w: Weekday) -> dt::Weekday {
    match w {
        Weekday::Monday => dt::Weekday::Monday,
        Weekday::Tuesday => dt::Weekday::Tuesday,
        Weekday::Wednesday => dt::Weekday::Wednesday,
        Weekday::Thursday => dt::Weekday::Thursday,
        Weekday::Friday => dt::Weekday::Friday,
        Weekday::Saturday => dt::Weekday::Saturday,
        Weekday::Sunday => dt::Weekday::Sunday,
    }
}

/// Generator for [`jiff::civil::Date`] values. Created by [`dates()`].
pub struct DateGenerator {
    inner: gs::DateGenerator,
}

impl DateGenerator {
    /// Set the minimum date (inclusive).
    pub fn min_value(self, min: Date) -> Self {
        DateGenerator {
            inner: self.inner.min_value(engine_date(min)),
        }
    }

    /// Set the maximum date (inclusive).
    pub fn max_value(self, max: Date) -> Self {
        DateGenerator {
            inner: self.inner.max_value(engine_date(max)),
        }
    }

    /// Draw only dates on these weekdays. A drawn day on another weekday
    /// is moved to the nearest allowed day inside the bounds rather than
    /// rejected, so the draw never filters.
    pub fn weekdays(self, weekdays: impl IntoIterator<Item = Weekday>) -> Self {
        DateGenerator {
            inner: self
                .inner
                .weekdays(weekdays.into_iter().map(engine_weekday)),
        }
    }

    /// Draw only dates in these months, moving a drawn day in another
    /// month to the nearest allowed day inside the bounds as
    /// [`weekdays`](Self::weekdays) does.
    pub fn months(self, months: impl IntoIterator<Item = Month>) -> Self {
        DateGenerator {
            inner: self.inner.months(months),
        }
    }
}

impl Generator<Date> for DateGenerator {
    fn label(&self) -> u64 {
        *DATE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Date {
        jiff_date(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<Date> for DateGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Date {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::civil::Date`] values.
///
/// Defaults span years 1–9999 (`0001-01-01` through `9999-12-31`) on any
/// weekday and in any month; shrinks toward 2000-01-01. Use the builder
/// methods to constrain the range, or to widen it down to jiff's minimum
/// of `-9999-01-01`. See [`DateGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::civil::{Date, Weekday};
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = Date::constant(2024, 1, 1);
///     let d = tc.draw(jiff_gs::dates().min_value(min).weekdays([Weekday::Saturday]));
///     assert!(d >= min && d.weekday() == Weekday::Saturday);
/// }
/// ```
pub fn dates() -> DateGenerator {
    DateGenerator {
        inner: gs::dates()
            .min_value(engine_date(Date::constant(1, 1, 1)))
            .max_value(engine_date(Date::constant(9999, 12, 31))),
    }
}

/// Generator for [`jiff::civil::Time`] values. Created by [`times()`].
pub struct TimeGenerator {
    inner: gs::TimeGenerator,
}

impl TimeGenerator {
    /// Set the minimum time (inclusive).
    pub fn min_value(self, min: Time) -> Self {
        TimeGenerator {
            inner: self.inner.min_value(engine_time(min)),
        }
    }

    /// Set the maximum time (inclusive).
    pub fn max_value(self, max: Time) -> Self {
        TimeGenerator {
            inner: self.inner.max_value(engine_time(max)),
        }
    }

    /// Draw only times that are a whole number of `precision` units
    /// since midnight.
    pub fn precision(self, precision: Precision) -> Self {
        TimeGenerator {
            inner: self.inner.precision(precision),
        }
    }
}

impl Generator<Time> for TimeGenerator {
    fn label(&self) -> u64 {
        *TIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Time {
        jiff_time(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<Time> for TimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Time {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::civil::Time`] values.
///
/// By default any time of day at nanosecond precision; shrinks toward
/// midnight. See [`TimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::Precision;
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::civil::Time;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let max = Time::constant(12, 0, 0, 0);
///     let t = tc.draw(jiff_gs::times().max_value(max).precision(Precision::Seconds));
///     assert!(t <= max && t.subsec_nanosecond() == 0);
/// }
/// ```
pub fn times() -> TimeGenerator {
    TimeGenerator { inner: gs::times() }
}

/// Generator for [`jiff::civil::DateTime`] values. Created by [`datetimes()`].
pub struct DateTimeGenerator {
    inner: gs::DateTimeGenerator,
}

impl DateTimeGenerator {
    /// Set the minimum datetime (inclusive).
    pub fn min_value(self, min: DateTime) -> Self {
        DateTimeGenerator {
            inner: self.inner.min_value(engine_datetime(min)),
        }
    }

    /// Set the maximum datetime (inclusive).
    pub fn max_value(self, max: DateTime) -> Self {
        DateTimeGenerator {
            inner: self.inner.max_value(engine_datetime(max)),
        }
    }

    /// Draw only datetimes whose time is a whole number of `precision`
    /// units since midnight.
    pub fn precision(self, precision: Precision) -> Self {
        DateTimeGenerator {
            inner: self.inner.precision(precision),
        }
    }
}

impl Generator<DateTime> for DateTimeGenerator {
    fn label(&self) -> u64 {
        *DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> DateTime {
        jiff_datetime(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<DateTime> for DateTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> DateTime {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::civil::DateTime`] values.
///
/// By default any datetime jiff holds, `-9999-01-01T00:00:00` through
/// `9999-12-31T23:59:59.999999999`, at nanosecond precision; shrinks
/// toward 2000-01-01T00:00:00. See [`DateTimeGenerator`] for builder
/// methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::civil::DateTime;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = DateTime::constant(2024, 1, 1, 0, 0, 0, 0);
///     let dt = tc.draw(jiff_gs::datetimes().min_value(min));
///     assert!(dt >= min);
/// }
/// ```
pub fn datetimes() -> DateTimeGenerator {
    DateTimeGenerator {
        inner: gs::datetimes()
            .min_value(engine_datetime(DateTime::MIN))
            .max_value(engine_datetime(DateTime::MAX)),
    }
}

/// Generator for [`jiff::Timestamp`] values. Created by [`timestamps()`].
pub struct TimestampGenerator {
    inner: gs::TimestampGenerator,
}

impl TimestampGenerator {
    /// Set the minimum timestamp (inclusive).
    pub fn min_value(self, min: Timestamp) -> Self {
        TimestampGenerator {
            inner: self.inner.min_value(engine_timestamp(min)),
        }
    }

    /// Set the maximum timestamp (inclusive).
    pub fn max_value(self, max: Timestamp) -> Self {
        TimestampGenerator {
            inner: self.inner.max_value(engine_timestamp(max)),
        }
    }

    /// Draw only timestamps that are a whole number of `precision` units
    /// since the Unix epoch.
    pub fn precision(self, precision: Precision) -> Self {
        TimestampGenerator {
            inner: self.inner.precision(precision),
        }
    }
}

impl Generator<Timestamp> for TimestampGenerator {
    fn label(&self) -> u64 {
        *TIMESTAMP_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Timestamp {
        jiff_timestamp(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<Timestamp> for TimestampGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Timestamp {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::Timestamp`] values.
///
/// By default any instant jiff holds at nanosecond precision; shrinks
/// toward 2000-01-01T00:00:00Z. One draw in twenty on average targets an
/// interesting instant: the Unix epoch, the 32-bit and 64-bit boundaries,
/// a leap second's or a time zone transition's. See
/// [`TimestampGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::Timestamp;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let t = tc.draw(jiff_gs::timestamps()
///         .min_value(Timestamp::UNIX_EPOCH));
///     assert!(t >= Timestamp::UNIX_EPOCH);
/// }
/// ```
pub fn timestamps() -> TimestampGenerator {
    TimestampGenerator {
        inner: gs::timestamps()
            .min_value(engine_timestamp(Timestamp::MIN))
            .max_value(engine_timestamp(Timestamp::MAX)),
    }
}

/// The largest magnitude of each unit of a jiff [`Span`], in
/// [`SpanUnit::ALL`] order.
const UNIT_LIMITS: [i64; 10] = [
    19_998,
    239_976,
    1_043_497,
    7_304_484,
    175_307_616,
    10_518_456_960,
    631_107_417_600,
    631_107_417_600_000,
    631_107_417_600_000_000,
    i64::MAX,
];

const UNIT_NAMES: [&str; 10] = [
    "years",
    "months",
    "weeks",
    "days",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
    "microseconds",
    "nanoseconds",
];

/// Generator for [`jiff::Span`] values. Created by [`spans()`].
pub struct SpanGenerator {
    bounds: Option<[Option<(i64, i64)>; 10]>,
    non_negative: bool,
    inner: OnceLock<gs::SpanGenerator>,
}

impl SpanGenerator {
    fn unit_bounds(mut self, unit: SpanUnit, min: i64, max: i64) -> Self {
        self.bounds.get_or_insert([None; 10])[unit as usize] = Some((min, max));
        self.inner = OnceLock::new();
        self
    }

    /// Draw years within `[min, max]` (inclusive). The first per-unit
    /// call replaces the default of every unit with just this one; later
    /// calls add units, so a span of days and hours is
    /// `spans().days(0, 30).hours(0, 23)`. A unit never enabled is
    /// always 0.
    pub fn years(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Years, min, max)
    }

    /// Draw months within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn months(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Months, min, max)
    }

    /// Draw weeks within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn weeks(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Weeks, min, max)
    }

    /// Draw days within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn days(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Days, min, max)
    }

    /// Draw hours within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn hours(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Hours, min, max)
    }

    /// Draw minutes within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn minutes(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Minutes, min, max)
    }

    /// Draw seconds within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn seconds(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Seconds, min, max)
    }

    /// Draw milliseconds within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn milliseconds(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Milliseconds, min, max)
    }

    /// Draw microseconds within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn microseconds(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Microseconds, min, max)
    }

    /// Draw nanoseconds within `[min, max]` (inclusive); see
    /// [`years`](Self::years) for how units combine.
    pub fn nanoseconds(self, min: i64, max: i64) -> Self {
        self.unit_bounds(SpanUnit::Nanoseconds, min, max)
    }

    /// Set the minimum number of nanoseconds, keeping the maximum:
    /// `nanoseconds(min, max)` with the current maximum.
    pub fn min_nanoseconds(self, min: i64) -> Self {
        let (_, max) = self.nanosecond_bounds();
        self.nanoseconds(min, max)
    }

    /// Set the maximum number of nanoseconds, keeping the minimum:
    /// `nanoseconds(min, max)` with the current minimum.
    pub fn max_nanoseconds(self, max: i64) -> Self {
        let (min, _) = self.nanosecond_bounds();
        self.nanoseconds(min, max)
    }

    /// Whether every unit is drawn as a magnitude, with no negative span
    /// (off by default, when the whole span is of one drawn sign).
    pub fn non_negative(mut self, non_negative: bool) -> Self {
        self.non_negative = non_negative;
        self.inner = OnceLock::new();
        self
    }

    fn default_min(&self, limit: i64) -> i64 {
        if self.non_negative { 0 } else { -limit }
    }

    fn nanosecond_bounds(&self) -> (i64, i64) {
        let nanoseconds = SpanUnit::Nanoseconds as usize;
        self.bounds
            .and_then(|bounds| bounds[nanoseconds])
            .unwrap_or((self.default_min(i64::MAX), i64::MAX))
    }

    fn inner(&self) -> &gs::SpanGenerator {
        self.inner.get_or_init(|| {
            let sign = if self.non_negative {
                SpanSign::NonNegative
            } else {
                SpanSign::Uniform
            };
            let mut generator = gs::spans().sign(sign);
            for (index, unit) in SpanUnit::ALL.into_iter().enumerate() {
                let (name, limit) = (UNIT_NAMES[index], UNIT_LIMITS[index]);
                let bound = match self.bounds {
                    None => Some((self.default_min(limit), limit)),
                    Some(bounds) => bounds[index],
                };
                let Some((min, max)) = bound else {
                    continue;
                };
                if min > max {
                    invalid_argument!("Cannot have max_{name} < min_{name}");
                }
                if min < -limit || max > limit {
                    invalid_argument!("{name} must be within ±{limit}, the limit of a jiff Span");
                }
                generator = generator.unit_bounds(unit, min, max);
            }
            generator
        })
    }
}

fn jiff_span(s: dt::Span) -> Span {
    Span::new()
        .years(s.years)
        .months(s.months)
        .weeks(s.weeks)
        .days(s.days)
        .hours(s.hours)
        .minutes(s.minutes)
        .seconds(s.seconds)
        .milliseconds(s.milliseconds)
        .microseconds(s.microseconds)
        .nanoseconds(s.nanoseconds)
}

impl Generator<Span> for SpanGenerator {
    fn label(&self) -> u64 {
        *SPAN_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Span {
        jiff_span(self.inner().do_draw(tc))
    }
}

impl PrintableGenerator<Span> for SpanGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Span {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::Span`] values.
///
/// By default every unit from years to nanoseconds within jiff's limit
/// for it, all of one drawn sign; shrinks toward the zero span. See
/// [`SpanGenerator`] for the per-unit builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let s = tc.draw(jiff_gs::spans().days(0, 30).hours(0, 23));
///     assert!((0..=30).contains(&s.get_days()) && s.get_years() == 0);
/// }
/// ```
pub fn spans() -> SpanGenerator {
    SpanGenerator {
        bounds: None,
        non_negative: false,
        inner: OnceLock::new(),
    }
}

/// Generator for [`jiff::SignedDuration`] values. Created by
/// [`signed_durations()`].
pub struct SignedDurationGenerator {
    inner: gs::SignedDurationGenerator,
}

impl SignedDurationGenerator {
    /// Set the minimum duration (inclusive).
    pub fn min_value(self, min: SignedDuration) -> Self {
        SignedDurationGenerator {
            inner: self.inner.min_value(engine_duration(min)),
        }
    }

    /// Set the maximum duration (inclusive).
    pub fn max_value(self, max: SignedDuration) -> Self {
        SignedDurationGenerator {
            inner: self.inner.max_value(engine_duration(max)),
        }
    }

    /// Draw only durations that are a whole number of `precision` units.
    pub fn precision(self, precision: Precision) -> Self {
        SignedDurationGenerator {
            inner: self.inner.precision(precision),
        }
    }
}

impl Generator<SignedDuration> for SignedDurationGenerator {
    fn label(&self) -> u64 {
        *SIGNED_DURATION_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> SignedDuration {
        jiff_duration(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<SignedDuration> for SignedDurationGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> SignedDuration {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::SignedDuration`] values.
///
/// By default any duration jiff holds, ±`i64` seconds, at nanosecond
/// precision; shrinks toward zero. One draw in twenty on average targets
/// a bound or a duration such as one unit, 24 hours or the `i32` seconds
/// limit. See [`SignedDurationGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::SignedDuration;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let d = tc.draw(jiff_gs::signed_durations().min_value(SignedDuration::ZERO));
///     assert!(d.as_nanos() >= 0);
/// }
/// ```
pub fn signed_durations() -> SignedDurationGenerator {
    SignedDurationGenerator {
        inner: gs::signed_durations()
            .min_value(engine_duration(SignedDuration::MIN))
            .max_value(engine_duration(SignedDuration::MAX)),
    }
}

/// Generator for [`jiff::tz::Offset`] values. Created by [`offsets()`].
pub struct OffsetGenerator {
    inner: gs::UtcOffsetGenerator,
}

impl OffsetGenerator {
    /// Set the minimum offset (inclusive).
    pub fn min_value(self, min: Offset) -> Self {
        OffsetGenerator {
            inner: self.inner.min_value(UtcOffset::Known(min.seconds())),
        }
    }

    /// Set the maximum offset (inclusive).
    pub fn max_value(self, max: Offset) -> Self {
        OffsetGenerator {
            inner: self.inner.max_value(UtcOffset::Known(max.seconds())),
        }
    }

    /// Draw only offsets that are a whole number of `granularity` units.
    pub fn granularity(self, granularity: Granularity) -> Self {
        OffsetGenerator {
            inner: self.inner.granularity(granularity),
        }
    }
}

impl Generator<Offset> for OffsetGenerator {
    fn label(&self) -> u64 {
        *OFFSET_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Offset {
        match self.inner.do_draw(tc) {
            UtcOffset::Known(seconds) => Offset::from_seconds(seconds).unwrap(),
            UtcOffset::Unknown => unreachable!(),
        }
    }
}

impl PrintableGenerator<Offset> for OffsetGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Offset {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::tz::Offset`] values.
///
/// By default any offset jiff holds, within ±25:59:59, at second
/// granularity; shrinks toward `+00:00`. One draw in twenty on average
/// targets an offset some time zone has used, so `+05:45` and `-00:44:30`
/// appear. An [`Offset`] is always known, so the engine's unknown offset
/// is never drawn; a zone with an unknown offset is
/// [`TimeZone::unknown`], which [`timezones()`] draws when asked to. See
/// [`OffsetGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::Granularity;
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::tz::Offset;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let o = tc.draw(jiff_gs::offsets()
///         .min_value(Offset::ZERO)
///         .granularity(Granularity::Minutes));
///     assert!(o.seconds() >= 0 && o.seconds() % 60 == 0);
/// }
/// ```
pub fn offsets() -> OffsetGenerator {
    OffsetGenerator {
        inner: gs::utc_offsets()
            .min_value(UtcOffset::Known(Offset::MIN.seconds()))
            .max_value(UtcOffset::Known(Offset::MAX.seconds())),
    }
}

struct KnownName {
    name: String,
    link: bool,
    legacy: bool,
}

/// Every name jiff's database and the engine's both know, with the
/// engine's link and legacy classification of it.
fn known_names() -> &'static [KnownName] {
    static NAMES: OnceLock<Vec<KnownName>> = OnceLock::new();
    NAMES.get_or_init(|| {
        jiff::tz::db()
            .available()
            .filter_map(|name| {
                let name = name.to_string();
                info(&name).map(|info| KnownName {
                    link: info.is_link,
                    legacy: info.tags.contains(&Tag::Legacy),
                    name,
                })
            })
            .collect()
    })
}

/// The engine generator over the names of `known` the link and legacy
/// flags admit, which is every name both databases know by default.
fn default_names(known: &[KnownName], links: bool, legacy: bool) -> gs::TimeZoneGenerator {
    if known.is_empty() {
        invalid_argument!(
            "jiff's time zone database is empty, so no IANA zone can be drawn: enable one of \
             jiff's tzdb features, such as tzdb-bundle-platform or tzdb-bundle-always"
        );
    }
    gs::timezones().names(
        known
            .iter()
            .filter(|known| (links || !known.link) && (legacy || !known.legacy))
            .map(|known| known.name.clone()),
    )
}

/// Generator for [`jiff::tz::TimeZone`] values named in the IANA
/// database. Created by [`timezones()`].
pub struct TimeZoneGenerator {
    names: Option<Vec<String>>,
    links: bool,
    legacy: bool,
    unknown: bool,
    inner: OnceLock<gs::TimeZoneGenerator>,
}

impl TimeZoneGenerator {
    /// Draw only these names, each one jiff's database resolves or
    /// `Etc/Unknown`. The [`links`](Self::links) and
    /// [`legacy`](Self::legacy) flags do not apply to an explicit list.
    pub fn names(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.names = Some(names.into_iter().map(Into::into).collect());
        self.inner = OnceLock::new();
        self
    }

    /// Whether the default set includes link names such as
    /// `Asia/Calcutta` (on by default).
    pub fn links(mut self, links: bool) -> Self {
        self.links = links;
        self.inner = OnceLock::new();
        self
    }

    /// Whether the default set includes legacy names such as `EST5EDT`
    /// and `US/Eastern` (off by default).
    pub fn legacy(mut self, legacy: bool) -> Self {
        self.legacy = legacy;
        self.inner = OnceLock::new();
        self
    }

    /// Whether [`TimeZone::unknown`], the zone `Etc/Unknown` with a known
    /// instant and an unknown offset, is drawn (off by default).
    pub fn unknown(mut self, unknown: bool) -> Self {
        self.unknown = unknown;
        self.inner = OnceLock::new();
        self
    }

    /// The engine generator over the names this one draws: an explicit
    /// list checked against jiff's database, or every name both
    /// databases know, so the engine decides which zone to draw and jiff
    /// decides what it means.
    pub(super) fn engine_generator(&self) -> gs::TimeZoneGenerator {
        let generator = match &self.names {
            Some(names) => {
                for name in names {
                    if TimeZone::get(name).is_err() {
                        invalid_argument!("jiff's time zone database does not know {name:?}");
                    }
                }
                gs::timezones().names(names.clone()).opaque(true)
            }
            None => default_names(known_names(), self.links, self.legacy),
        };
        generator.unknown(self.unknown)
    }

    fn inner(&self) -> &gs::TimeZoneGenerator {
        self.inner.get_or_init(|| self.engine_generator())
    }
}

/// The jiff zone of a name the engine drew under a spec restricted to
/// jiff's database.
fn jiff_time_zone(zone: &dt::TimeZone) -> TimeZone {
    TimeZone::get(&zone.name).unwrap()
}

impl Generator<TimeZone> for TimeZoneGenerator {
    fn label(&self) -> u64 {
        *TIME_ZONE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> TimeZone {
        jiff_time_zone(&self.inner().do_draw(tc))
    }
}

impl PrintableGenerator<TimeZone> for TimeZoneGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> TimeZone {
        let zone = self.inner().do_draw(tc);
        printer.text(&zone.name);
        jiff_time_zone(&zone)
    }
}

/// Generate [`jiff::tz::TimeZone`] values named in the IANA database.
///
/// By default any name that both jiff's database and the engine's know,
/// link names in and legacy names out, resolved with [`TimeZone::get`]
/// so a link such as `Asia/Calcutta` keeps its name; shrinks down a
/// ladder of names to `UTC` when it is allowed. Half the draws are
/// uniform over the allowed names and half weighted by the zones'
/// features, so a zone with a rare feature (a fractional offset, a
/// skipped day, negative DST) is drawn at a rate a short run reaches.
/// Fixed-offset zones are a different generator: compose
/// `offsets().map(TimeZone::fixed)` with this one through
/// [`one_of`](crate::generators::one_of) for a mix, as
/// `gs::default::<TimeZone>()` does. jiff must have a time zone database
/// for any name to be known, one of its `tzdb-*` features or the
/// system's; without one the draw is a usage error. See
/// [`TimeZoneGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zone = tc.draw(jiff_gs::timezones().names(["Europe/London", "Asia/Calcutta"]));
///     assert!(zone.iana_name() == Some("Europe/London") || zone.iana_name() == Some("Asia/Calcutta"));
/// }
/// ```
pub fn timezones() -> TimeZoneGenerator {
    TimeZoneGenerator {
        names: None,
        links: true,
        legacy: false,
        unknown: false,
        inner: OnceLock::new(),
    }
}

/// The earliest wall-clock time jiff converts to an instant at every
/// offset it allows: 26 hours after [`Timestamp::MIN`].
pub const MIN_CIVIL: DateTime = DateTime::constant(-9999, 1, 3, 4, 0, 0, 0);

/// The latest wall-clock time jiff converts to an instant at every
/// offset it allows: 26 hours before [`Timestamp::MAX`].
pub const MAX_CIVIL: DateTime = DateTime::constant(9999, 12, 29, 20, 0, 0, 0);

enum ZonedBounds {
    Civil(DateTime, DateTime),
    Instant(Timestamp, Timestamp),
}

/// Generator for [`jiff::Zoned`] values. Created by [`zoneds()`].
pub struct ZonedGenerator {
    zones: TimeZoneGenerator,
    bounds: ZonedBounds,
    precision: Precision,
    allow_imaginary: bool,
    ambiguous: Ambiguous,
    inner: OnceLock<gs::ZonedDateTimeGenerator>,
}

impl ZonedGenerator {
    /// Draw the zone from `zones` rather than from every name both
    /// databases know.
    pub fn timezones(mut self, zones: TimeZoneGenerator) -> Self {
        self.zones = zones;
        self.inner = OnceLock::new();
        self
    }

    /// Set the minimum wall-clock time (inclusive), in whatever zone is
    /// drawn, at or after [`MIN_CIVIL`]. Replaces instant bounds.
    pub fn min_value(mut self, min: DateTime) -> Self {
        let max = match self.bounds {
            ZonedBounds::Civil(_, max) => max,
            ZonedBounds::Instant(..) => MAX_CIVIL,
        };
        self.bounds = ZonedBounds::Civil(min, max);
        self.inner = OnceLock::new();
        self
    }

    /// Set the maximum wall-clock time (inclusive), in whatever zone is
    /// drawn, at or before [`MAX_CIVIL`]. Replaces instant bounds.
    pub fn max_value(mut self, max: DateTime) -> Self {
        let min = match self.bounds {
            ZonedBounds::Civil(min, _) => min,
            ZonedBounds::Instant(..) => MIN_CIVIL,
        };
        self.bounds = ZonedBounds::Civil(min, max);
        self.inner = OnceLock::new();
        self
    }

    /// Set the minimum instant (inclusive). Replaces wall-clock bounds.
    pub fn min_instant(mut self, min: Timestamp) -> Self {
        let max = match self.bounds {
            ZonedBounds::Instant(_, max) => max,
            ZonedBounds::Civil(..) => Timestamp::MAX,
        };
        self.bounds = ZonedBounds::Instant(min, max);
        self.inner = OnceLock::new();
        self
    }

    /// Set the maximum instant (inclusive). Replaces wall-clock bounds.
    pub fn max_instant(mut self, max: Timestamp) -> Self {
        let min = match self.bounds {
            ZonedBounds::Instant(min, _) => min,
            ZonedBounds::Civil(..) => Timestamp::MIN,
        };
        self.bounds = ZonedBounds::Instant(min, max);
        self.inner = OnceLock::new();
        self
    }

    /// Draw only wall-clock times that are a whole number of `precision`
    /// units since midnight.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self.inner = OnceLock::new();
        self
    }

    /// Whether a wall time the clocks skipped may be drawn (`false` by
    /// default, since a [`Zoned`] cannot hold one). Allowed, the value is
    /// the instant the skipped time resolves to with the offset in force
    /// before the transition, as jiff's own `compatible` resolution would
    /// give; its own wall time lies past the gap, so it can exceed
    /// [`max_value`](Self::max_value) by the gap's width.
    pub fn allow_imaginary(mut self, allow: bool) -> Self {
        self.allow_imaginary = allow;
        self.inner = OnceLock::new();
        self
    }

    /// Which instant a wall time the clocks repeated means
    /// ([`Ambiguous::Both`] by default).
    pub fn ambiguous(mut self, ambiguous: Ambiguous) -> Self {
        self.ambiguous = ambiguous;
        self.inner = OnceLock::new();
        self
    }

    fn inner(&self) -> &gs::ZonedDateTimeGenerator {
        self.inner.get_or_init(|| {
            let generator = gs::zoned_datetimes()
                .timezones(self.zones.engine_generator())
                .precision(self.precision)
                .allow_imaginary(self.allow_imaginary)
                .ambiguous(self.ambiguous);
            match self.bounds {
                ZonedBounds::Civil(min, max) => {
                    if min < MIN_CIVIL {
                        invalid_argument!(
                            "min_value must be at or after {MIN_CIVIL}, the earliest wall time \
                             jiff converts to an instant at every offset"
                        );
                    }
                    if max > MAX_CIVIL {
                        invalid_argument!(
                            "max_value must be at or before {MAX_CIVIL}, the latest wall time \
                             jiff converts to an instant at every offset"
                        );
                    }
                    generator
                        .min_value(engine_datetime(min))
                        .max_value(engine_datetime(max))
                }
                ZonedBounds::Instant(min, max) => generator
                    .min_instant(engine_timestamp(min))
                    .max_instant(engine_timestamp(max)),
            }
        })
    }
}

/// Build the jiff value of an engine draw from its wall time, zone and
/// fold, and from its instant for a skipped wall time. `None` when jiff
/// has no value at that wall time: its database skips it, or it reads
/// the instant it resolves to back as a different wall time, as it does
/// in the last second before a pre-1970 gap.
fn jiff_zoned(z: &dt::ZonedDateTime) -> Option<Zoned> {
    let zone = jiff_time_zone(&z.zone);
    match z.kind {
        Kind::Gap => Some(jiff_timestamp(z.timestamp).to_zoned(zone)),
        Kind::Unique | Kind::Fold => {
            let civil = jiff_datetime(z.civil);
            let ambiguous = zone.to_ambiguous_zoned(civil);
            let resolved = if z.fold {
                ambiguous.later()
            } else {
                ambiguous.earlier()
            };
            resolved.ok().filter(|zoned| zoned.datetime() == civil)
        }
    }
}

/// Whether jiff's reading of a draw lies within the instant bounds the
/// engine kept the draw within under its own rules, which jiff's
/// database can read with another offset near a transition the two
/// disagree on.
fn within_instant_bounds(zoned: &Zoned, bounds: &ZonedBounds) -> bool {
    match bounds {
        ZonedBounds::Civil(..) => true,
        ZonedBounds::Instant(min, max) => (min..=max).contains(&&zoned.timestamp()),
    }
}

impl Generator<Zoned> for ZonedGenerator {
    fn label(&self) -> u64 {
        *ZONED_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Zoned {
        jiff_zoned(&self.inner().do_draw(tc))
            .filter(|zoned| within_instant_bounds(zoned, &self.bounds))
            .unwrap_or_else(|| tc.reject())
    }
}

impl PrintableGenerator<Zoned> for ZonedGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Zoned {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`jiff::Zoned`] values: a zone, a wall-clock time in it and,
/// when the clocks repeated that time, which repeat is meant.
///
/// By default any zone [`timezones()`] draws and any wall-clock time
/// from [`MIN_CIVIL`] to [`MAX_CIVIL`] at nanosecond precision, skipped
/// wall times avoided and both repeats of a fold drawn; shrinks down the
/// zone ladder, then toward 2000-01-01T00:00:00, preferring a unique wall
/// time to a repeated one. One draw in twenty on average targets a
/// transition of the zone, which is how folds and the exact boundary
/// instants are reached. The value is built from the wall time, the zone
/// name and the fold with jiff's own rules (`to_ambiguous_zoned` and
/// `earlier` or `later`), so a newer database on jiff's side changes what
/// a value means; a wall time jiff has no value at, because its database
/// skips it or it reads the instant back as another wall time, is
/// rejected as if by `assume(false)`, as is a wall time whose instant
/// under jiff's rules falls outside instant bounds the engine kept it
/// within. The engine's database release is
/// [`tzdb_version`](crate::datetime::tzdb_version). See [`ZonedGenerator`]
/// for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::civil::DateTime;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = DateTime::constant(2024, 1, 1, 0, 0, 0, 0);
///     let z = tc.draw(jiff_gs::zoneds()
///         .timezones(jiff_gs::timezones().names(["Europe/London"]))
///         .min_value(min));
///     assert!(z.datetime() >= min && z.time_zone().iana_name() == Some("Europe/London"));
/// }
/// ```
pub fn zoneds() -> ZonedGenerator {
    ZonedGenerator {
        zones: timezones(),
        bounds: ZonedBounds::Civil(MIN_CIVIL, MAX_CIVIL),
        precision: Precision::Nanoseconds,
        allow_imaginary: false,
        ambiguous: Ambiguous::Both,
        inner: OnceLock::new(),
    }
}

/// Generator for [`jiff::civil::ISOWeekDate`] values. Created by
/// [`iso_week_dates()`].
pub type ISOWeekDateGenerator = Mapped<Date, ISOWeekDate, fn(Date) -> ISOWeekDate, DateGenerator>;

/// Generate [`jiff::civil::ISOWeekDate`] values: the ISO week date of a
/// date [`dates()`] draws, so years 1–9999 and shrinking toward
/// 2000-01-01.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let w = tc.draw(jiff_gs::iso_week_dates());
///     assert!((1..=53).contains(&w.week()));
/// }
/// ```
pub fn iso_week_dates() -> ISOWeekDateGenerator {
    dates().map(|d| d.iso_week_date())
}

/// Generator for [`jiff::civil::Weekday`] values. Created by
/// [`weekdays()`].
pub type WeekdayGenerator = Mapped<i8, Weekday, fn(i8) -> Weekday, IntegerGenerator<i8>>;

/// Generate [`jiff::civil::Weekday`] values, uniformly; shrinks toward
/// Monday.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::civil::Weekday;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let w = tc.draw(jiff_gs::weekdays());
///     assert!(w.since(Weekday::Monday) < 7);
/// }
/// ```
pub fn weekdays() -> WeekdayGenerator {
    gs::integers::<i8>()
        .min_value(0)
        .max_value(6)
        .map(|n| Weekday::from_monday_zero_offset(n).unwrap())
}

/// Generator for [`jiff::civil::Era`] values. Created by [`eras()`].
pub type EraGenerator = Mapped<bool, Era, fn(bool) -> Era, BoolGenerator>;

/// Generate [`jiff::civil::Era`] values, either with equal probability;
/// shrinks toward [`Era::CE`].
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use jiff::civil::Era;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let era = tc.draw(jiff_gs::eras());
///     assert!(era == Era::CE || era == Era::BCE);
/// }
/// ```
pub fn eras() -> EraGenerator {
    gs::booleans().map(|bce| if bce { Era::BCE } else { Era::CE })
}

#[cfg(test)]
#[path = "../../../tests/embedded/extras/jiff_generators_tests.rs"]
mod tests;
