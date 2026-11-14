use std::any::Any;
use std::sync::OnceLock;

use jiff::civil::{Date, DateTime, Era, ISOWeekDate, Time, Weekday};
use jiff::tz::{Offset, TimeZone};
use jiff::{SignedDuration, Span, Timestamp, Zoned};
use std::sync::LazyLock;

use super::source::zone_source;
use crate::generators::temporal::generators::{
    ZonedBounds, draw_and_print_display, zoned_datetimes_within,
};
use crate::generators::temporal::{
    self as dt, Ambiguous, Granularity, Month, Precision, SpanSign, SpanUnit, TransitionKind,
    UtcOffset,
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
static TIME_ZONE_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.jiff.zones"));

const NANOS_PER_SECOND: i128 = 1_000_000_000;

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

fn engine_timestamp(t: Timestamp) -> dt::Instant {
    let nanos = t.as_nanosecond();
    dt::Instant {
        second: nanos.div_euclid(NANOS_PER_SECOND) as i64,
        nanosecond: nanos.rem_euclid(NANOS_PER_SECOND) as u32,
    }
}

fn jiff_timestamp(t: dt::Instant) -> Timestamp {
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
    inner: gs::temporal::DateGenerator,
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
        inner: gs::temporal::dates()
            .min_value(engine_date(Date::constant(1, 1, 1)))
            .max_value(engine_date(Date::constant(9999, 12, 31))),
    }
}

/// Generator for [`jiff::civil::Time`] values. Created by [`times()`].
pub struct TimeGenerator {
    inner: gs::temporal::TimeGenerator,
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
/// use hegel::generators::temporal::Precision;
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
    TimeGenerator {
        inner: gs::temporal::times(),
    }
}

/// Generator for [`jiff::civil::DateTime`] values. Created by [`datetimes()`].
pub struct DateTimeGenerator {
    inner: gs::temporal::DateTimeGenerator,
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
        inner: gs::temporal::datetimes()
            .min_value(engine_datetime(DateTime::MIN))
            .max_value(engine_datetime(DateTime::MAX)),
    }
}

/// Generator for [`jiff::Timestamp`] values. Created by [`timestamps()`].
pub struct TimestampGenerator {
    inner: gs::temporal::InstantGenerator,
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
        inner: gs::temporal::instants()
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
    inner: OnceLock<gs::temporal::SpanGenerator>,
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
    /// `nanoseconds(min, max)` with the current maximum, which like any
    /// per-unit call names nanoseconds as a unit to draw.
    pub fn min_nanoseconds(self, min: i64) -> Self {
        let (_, max) = self.nanosecond_bounds();
        self.nanoseconds(min, max)
    }

    /// Set the maximum number of nanoseconds, keeping the minimum:
    /// `nanoseconds(min, max)` with the current minimum, which like any
    /// per-unit call names nanoseconds as a unit to draw.
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

    fn inner(&self) -> &gs::temporal::SpanGenerator {
        self.inner.get_or_init(|| {
            let sign = if self.non_negative {
                SpanSign::NonNegative
            } else {
                SpanSign::Uniform
            };
            let mut generator = gs::temporal::spans().sign(sign);
            for (index, unit) in SpanUnit::ALL.iter().copied().enumerate() {
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
    inner: gs::temporal::DurationGenerator,
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
        inner: gs::temporal::durations()
            .min_value(engine_duration(SignedDuration::MIN))
            .max_value(engine_duration(SignedDuration::MAX)),
    }
}

/// Generator for [`jiff::tz::Offset`] values. Created by [`offsets()`].
pub struct OffsetGenerator {
    inner: gs::temporal::OffsetGenerator,
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
/// [`TimeZone::unknown`], which [`zones()`] draws when asked to. See
/// [`OffsetGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::generators::temporal::Granularity;
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
        inner: gs::temporal::offsets()
            .min_value(UtcOffset::Known(Offset::MIN.seconds()))
            .max_value(UtcOffset::Known(Offset::MAX.seconds())),
    }
}

/// Fail as a usage error when jiff's database has no names, so no zone
/// can be drawn from it.
fn require_a_database(empty: bool) {
    if empty {
        invalid_argument!(
            "jiff's time zone database is empty, so no IANA zone can be drawn: enable one of \
             jiff's tzdb features, such as tzdb-bundle-platform or tzdb-bundle-always"
        );
    }
}

/// Generator for [`jiff::tz::TimeZone`] values named in the IANA
/// database. Created by [`zones()`].
pub struct ZoneGenerator {
    names: Option<Vec<String>>,
    allow_links: bool,
    allow_legacy: bool,
    allow_unknown: bool,
    inner: OnceLock<gs::temporal::ZoneGenerator>,
}

impl ZoneGenerator {
    /// Draw only these names, each one jiff's database resolves or
    /// `Etc/Unknown`. The [`allow_links`](Self::allow_links) and
    /// [`allow_legacy`](Self::allow_legacy) flags do not apply to an
    /// explicit list.
    pub fn names(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.names = Some(names.into_iter().map(Into::into).collect());
        self.inner = OnceLock::new();
        self
    }

    /// Whether the default set includes link names such as
    /// `Asia/Calcutta` (on by default).
    pub fn allow_links(mut self, allow: bool) -> Self {
        self.allow_links = allow;
        self.inner = OnceLock::new();
        self
    }

    /// Whether the default set includes legacy names such as `EST5EDT`
    /// and `US/Eastern` (off by default).
    pub fn allow_legacy(mut self, allow: bool) -> Self {
        self.allow_legacy = allow;
        self.inner = OnceLock::new();
        self
    }

    /// Whether [`TimeZone::unknown`], the zone `Etc/Unknown` with a known
    /// instant and an unknown offset, is drawn (off by default).
    pub fn allow_unknown(mut self, allow: bool) -> Self {
        self.allow_unknown = allow;
        self.inner = OnceLock::new();
        self
    }

    /// The engine generator over the names this one draws from jiff's
    /// database, through [`zone_source`]: an explicit list checked against
    /// it, or every name it has that the link and legacy flags admit.
    pub(super) fn engine_generator(&self) -> gs::temporal::ZoneGenerator {
        let generator = gs::temporal::zones().source(zone_source());
        let generator = match &self.names {
            Some(names) => {
                for name in names {
                    if TimeZone::get(name).is_err() {
                        invalid_argument!("jiff's time zone database does not know {name:?}");
                    }
                }
                generator.names(names.clone())
            }
            None => {
                require_a_database(jiff::tz::db().available().next().is_none());
                generator
                    .allow_links(self.allow_links)
                    .allow_legacy(self.allow_legacy)
            }
        };
        generator.allow_unknown(self.allow_unknown)
    }

    fn inner(&self) -> &gs::temporal::ZoneGenerator {
        self.inner.get_or_init(|| self.engine_generator())
    }
}

/// The jiff zone of a name the engine drew under a spec restricted to
/// jiff's database.
pub(super) fn jiff_time_zone(name: &str) -> TimeZone {
    TimeZone::get(name).unwrap()
}

impl Generator<TimeZone> for ZoneGenerator {
    fn label(&self) -> u64 {
        *TIME_ZONE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> TimeZone {
        jiff_time_zone(&self.inner().draw_name(tc))
    }
}

impl PrintableGenerator<TimeZone> for ZoneGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> TimeZone {
        let name = self.inner().draw_name(tc);
        printer.text(&name);
        jiff_time_zone(&name)
    }
}

/// Generate [`jiff::tz::TimeZone`] values named in the IANA database.
///
/// By default any name of jiff's database ([`zone_source`]), link names
/// in and legacy names out, resolved with [`TimeZone::get`]
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
/// [`ZoneGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zone = tc.draw(jiff_gs::zones().names(["Europe/London", "Asia/Calcutta"]));
///     assert!(zone.iana_name() == Some("Europe/London") || zone.iana_name() == Some("Asia/Calcutta"));
/// }
/// ```
pub fn zones() -> ZoneGenerator {
    ZoneGenerator {
        names: None,
        allow_links: true,
        allow_legacy: false,
        allow_unknown: false,
        inner: OnceLock::new(),
    }
}

/// The earliest wall-clock time jiff converts to an instant at every
/// offset it allows: 26 hours after [`Timestamp::MIN`].
pub const MIN_CIVIL: DateTime = DateTime::constant(-9999, 1, 3, 4, 0, 0, 0);

/// The latest wall-clock time jiff converts to an instant at every
/// offset it allows: 26 hours before [`Timestamp::MAX`].
pub const MAX_CIVIL: DateTime = DateTime::constant(9999, 12, 29, 20, 0, 0, 0);

trait AnyZones: Generator<TimeZone> + Any + Send + Sync {}

impl<G> AnyZones for G where G: Generator<TimeZone> + Any + Send + Sync {}

/// Generator for [`jiff::Zoned`] values. Created by [`zoneds()`].
pub struct ZonedGenerator {
    zones: Box<dyn AnyZones>,
    min_value: Option<DateTime>,
    max_value: Option<DateTime>,
    min_instant: Option<Timestamp>,
    max_instant: Option<Timestamp>,
    precision: Precision,
    allow_imaginary: bool,
    ambiguous: Ambiguous,
    inner: OnceLock<gs::temporal::ZonedDateTimeGenerator>,
}

impl ZonedGenerator {
    /// Draw the zone from `zones` rather than from every name of jiff's
    /// database.
    ///
    /// Given a [`ZoneGenerator`] (what [`zones()`] returns, with whatever
    /// builder methods were applied), the zone and the wall time come
    /// from one engine draw, so the zone shrinks down the engine's ladder
    /// of names and transitions are targeted across the allowed zones.
    ///
    /// Any other generator of [`TimeZone`] draws the zone first and then
    /// a wall time in it, under the same bounds and options: a zone with
    /// an IANA name is classified against jiff's database
    /// ([`zone_source`]), and a fixed-offset zone such as
    /// `TimeZone::fixed(offset)` or `TimeZone::UTC`, which has no gaps or
    /// folds, takes any wall time in the bounds. So
    /// `gs::just(TimeZone::UTC)`, `offsets().map(TimeZone::fixed)` and
    /// `gs::default::<TimeZone>()` all work here. A zone that is neither,
    /// such as one built from a POSIX time zone string, or one whose name
    /// jiff's database does not have, is a usage error.
    pub fn zones<G>(mut self, zones: G) -> Self
    where
        G: Generator<TimeZone> + Send + Sync + 'static,
    {
        self.zones = Box::new(zones);
        self.inner = OnceLock::new();
        self
    }

    /// Set the minimum wall-clock time (inclusive), in whatever zone is
    /// drawn, at or after [`MIN_CIVIL`]. Wall-clock bounds cannot be
    /// combined with instant bounds.
    pub fn min_value(mut self, min: DateTime) -> Self {
        self.min_value = Some(min);
        self.inner = OnceLock::new();
        self
    }

    /// Set the maximum wall-clock time (inclusive), in whatever zone is
    /// drawn, at or before [`MAX_CIVIL`]. Wall-clock bounds cannot be
    /// combined with instant bounds.
    pub fn max_value(mut self, max: DateTime) -> Self {
        self.max_value = Some(max);
        self.inner = OnceLock::new();
        self
    }

    /// Set the minimum instant (inclusive). Instant bounds cannot be
    /// combined with wall-clock bounds.
    pub fn min_instant(mut self, min: Timestamp) -> Self {
        self.min_instant = Some(min);
        self.inner = OnceLock::new();
        self
    }

    /// Set the maximum instant (inclusive). Instant bounds cannot be
    /// combined with wall-clock bounds.
    pub fn max_instant(mut self, max: Timestamp) -> Self {
        self.max_instant = Some(max);
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

    /// The zones when they are a [`ZoneGenerator`], which the engine
    /// draws together with the wall time.
    fn named_zones(&self) -> Option<&ZoneGenerator> {
        let zones: &dyn Any = &*self.zones;
        zones.downcast_ref::<ZoneGenerator>()
    }

    fn inner(&self) -> &gs::temporal::ZonedDateTimeGenerator {
        self.inner.get_or_init(|| {
            let mut generator = zoned_datetimes_within(
                (engine_datetime(MIN_CIVIL), engine_datetime(MAX_CIVIL)),
                (
                    engine_timestamp(Timestamp::MIN),
                    engine_timestamp(Timestamp::MAX),
                ),
            )
            .precision(self.precision)
            .allow_imaginary(self.allow_imaginary)
            .ambiguous(self.ambiguous);
            generator = match self.named_zones() {
                Some(zones) => generator.zones(zones.engine_generator()),
                None => generator.source(zone_source()),
            };
            if let Some(min) = self.min_value {
                if min < MIN_CIVIL {
                    invalid_argument!(
                        "min_value must be at or after {MIN_CIVIL}, the earliest wall time \
                         jiff converts to an instant at every offset"
                    );
                }
                generator = generator.min_value(engine_datetime(min));
            }
            if let Some(max) = self.max_value {
                if max > MAX_CIVIL {
                    invalid_argument!(
                        "max_value must be at or before {MAX_CIVIL}, the latest wall time \
                         jiff converts to an instant at every offset"
                    );
                }
                generator = generator.max_value(engine_datetime(max));
            }
            if let Some(min) = self.min_instant {
                generator = generator.min_instant(engine_timestamp(min));
            }
            if let Some(max) = self.max_instant {
                generator = generator.max_instant(engine_timestamp(max));
            }
            generator
        })
    }

    /// Draw a wall time in `zone`, which a generator other than
    /// [`zones()`] drew.
    fn draw_in_zone(&self, tc: &TestCase, zone: TimeZone) -> Zoned {
        let inner = self.inner();
        if zone.to_fixed_offset().is_ok() {
            let (min, max) = match inner.bounds() {
                ZonedBounds::Civil(min, max) => (jiff_datetime(min), jiff_datetime(max)),
                ZonedBounds::Instant(min, max) => (
                    jiff_timestamp(min).to_zoned(zone.clone()).datetime(),
                    jiff_timestamp(max).to_zoned(zone.clone()).datetime(),
                ),
            };
            let civil = tc.draw_silent(
                datetimes()
                    .min_value(min)
                    .max_value(max)
                    .precision(self.precision),
            );
            return civil.to_zoned(zone).unwrap();
        }
        let Some(name) = zone.iana_name() else {
            invalid_argument!(
                "zoneds() draws in a time zone with an IANA name or a fixed offset, which \
                 {zone:?} from the zones generator is not"
            );
        };
        let engine_zone = dt::TimeZone {
            name: name.to_string(),
            primary_name: name.to_string(),
        };
        let z = inner
            .draw_in_zone(tc, engine_zone)
            .unwrap_or_else(|message| {
                invalid_argument!(
                    "zoneds(): jiff's time zone database cannot place a wall time in the time zone \
                 {name:?} the zones generator drew: {message}"
                )
            });
        jiff_zoned(&z, zone).unwrap_or_else(|| tc.reject())
    }
}

/// Build the jiff value of an engine draw in `zone` from its wall time and
/// fold, and from its instant for a skipped wall time. `None` when jiff
/// reads the instant it resolves the wall time to back as a different
/// wall time, as it does in the last second before some pre-1970 gaps.
fn jiff_zoned(z: &dt::ZonedDateTime, zone: TimeZone) -> Option<Zoned> {
    match z.kind {
        TransitionKind::Gap => Some(jiff_timestamp(z.instant).to_zoned(zone)),
        TransitionKind::Unique | TransitionKind::Fold => {
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

impl Generator<Zoned> for ZonedGenerator {
    fn label(&self) -> u64 {
        *ZONED_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Zoned {
        if self.named_zones().is_none() {
            let zone = tc.draw_silent(&*self.zones);
            return self.draw_in_zone(tc, zone);
        }
        let z = self.inner().do_draw(tc);
        jiff_zoned(&z, jiff_time_zone(&z.zone.name)).unwrap_or_else(|| tc.reject())
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
/// By default any zone [`zones()`] draws and any wall-clock time
/// from [`MIN_CIVIL`] to [`MAX_CIVIL`] at nanosecond precision, skipped
/// wall times avoided and both repeats of a fold drawn; shrinks down the
/// zone ladder, then toward 2000-01-01T00:00:00, preferring a unique wall
/// time to a repeated one. One draw in twenty on average targets a
/// transition of the zone, which is how folds and the exact boundary
/// instants are reached. The value is built from the wall time, the zone
/// and the fold with jiff's own rules (`to_ambiguous_zoned` and
/// `earlier` or `later`), against the data of jiff's database the engine
/// drew with ([`zone_source`]); a wall time jiff reads back as another
/// wall time, as it does in the last second before some pre-1970 gaps
/// and at some transitions that change only the abbreviation, is rejected
/// as if by `assume(false)`. [`zones`](ZonedGenerator::zones) takes any
/// generator of [`TimeZone`], fixed-offset zones included. See
/// [`ZonedGenerator`] for builder methods.
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
///         .zones(jiff_gs::zones().names(["Europe/London"]))
///         .min_value(min));
///     assert!(z.datetime() >= min && z.time_zone().iana_name() == Some("Europe/London"));
/// }
/// ```
pub fn zoneds() -> ZonedGenerator {
    ZonedGenerator {
        zones: Box::new(zones()),
        min_value: None,
        max_value: None,
        min_instant: None,
        max_instant: None,
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
