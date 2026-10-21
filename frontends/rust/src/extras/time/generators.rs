use crate::datetime::{self, Ambiguous, Granularity, Precision};
use crate::generators::{self as gs, Generator, PrintableGenerator, TestCase, label_from_name};
use crate::pretty::PrettyPrinter;
use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset, Weekday};

const DATE_LABEL: u64 = label_from_name("hegel.time.dates");
const TIME_LABEL: u64 = label_from_name("hegel.time.times");
const PRIMITIVE_DATETIME_LABEL: u64 = label_from_name("hegel.time.primitive_datetimes");
const UTC_OFFSET_LABEL: u64 = label_from_name("hegel.time.utc_offsets");
const OFFSET_DATETIME_LABEL: u64 = label_from_name("hegel.time.offset_datetimes");
const DURATION_LABEL: u64 = label_from_name("hegel.time.durations");
const WEEKDAY_LABEL: u64 = label_from_name("hegel.time.weekdays");
const MONTH_LABEL: u64 = label_from_name("hegel.time.months");

const NANOS_PER_SECOND: i128 = 1_000_000_000;

const HEGEL_WEEKDAYS: [datetime::Weekday; 7] = [
    datetime::Weekday::Monday,
    datetime::Weekday::Tuesday,
    datetime::Weekday::Wednesday,
    datetime::Weekday::Thursday,
    datetime::Weekday::Friday,
    datetime::Weekday::Saturday,
    datetime::Weekday::Sunday,
];

const HEGEL_MONTHS: [datetime::Month; 12] = [
    datetime::Month::January,
    datetime::Month::February,
    datetime::Month::March,
    datetime::Month::April,
    datetime::Month::May,
    datetime::Month::June,
    datetime::Month::July,
    datetime::Month::August,
    datetime::Month::September,
    datetime::Month::October,
    datetime::Month::November,
    datetime::Month::December,
];

fn hegel_weekday(weekday: Weekday) -> datetime::Weekday {
    HEGEL_WEEKDAYS[usize::from(weekday.number_days_from_monday())]
}

fn hegel_month(month: Month) -> datetime::Month {
    HEGEL_MONTHS[month as usize - 1]
}

fn hegel_date(d: Date) -> datetime::Date {
    datetime::Date {
        year: d.year(),
        month: d.month() as u8,
        day: d.day(),
    }
}

/// The `time::Date` of an engine date, which must lie within
/// `Date::MIN..=Date::MAX`.
fn time_date(d: datetime::Date) -> Option<Date> {
    Date::from_calendar_date(d.year, Month::try_from(d.month).unwrap(), d.day).ok()
}

fn hegel_time(t: Time) -> datetime::Time {
    let (hour, minute, second, nanosecond) = t.as_hms_nano();
    datetime::Time {
        hour,
        minute,
        second,
        nanosecond,
    }
}

fn time_time(t: datetime::Time) -> Time {
    Time::from_hms_nano(t.hour, t.minute, t.second, t.nanosecond).unwrap()
}

fn hegel_datetime(dt: PrimitiveDateTime) -> datetime::DateTime {
    datetime::DateTime {
        date: hegel_date(dt.date()),
        time: hegel_time(dt.time()),
    }
}

fn time_datetime(dt: datetime::DateTime) -> Option<PrimitiveDateTime> {
    time_date(dt.date).map(|date| PrimitiveDateTime::new(date, time_time(dt.time)))
}

fn hegel_offset(o: UtcOffset) -> datetime::UtcOffset {
    datetime::UtcOffset::Known(o.whole_seconds())
}

fn hegel_duration(d: Duration) -> datetime::SignedDuration {
    datetime::SignedDuration {
        seconds: d.whole_seconds(),
        nanoseconds: d.subsec_nanoseconds(),
    }
}

fn hegel_timestamp(dt: OffsetDateTime) -> datetime::Timestamp {
    let nanos = dt.unix_timestamp_nanos();
    datetime::Timestamp {
        second: nanos.div_euclid(NANOS_PER_SECOND) as i64,
        nanosecond: nanos.rem_euclid(NANOS_PER_SECOND) as u32,
    }
}

/// Generator for [`time::Date`] values. Created by [`dates()`].
pub struct DateGenerator {
    inner: gs::DateGenerator,
}

impl DateGenerator {
    /// Set the minimum date (inclusive).
    pub fn min_value(mut self, min: Date) -> Self {
        self.inner = self.inner.min_value(hegel_date(min));
        self
    }

    /// Set the maximum date (inclusive).
    pub fn max_value(mut self, max: Date) -> Self {
        self.inner = self.inner.max_value(hegel_date(max));
        self
    }

    /// Draw only dates on these weekdays. A drawn day on another weekday
    /// is moved to the nearest allowed day inside the bounds rather than
    /// rejected, so the draw never filters.
    pub fn weekdays(mut self, weekdays: impl IntoIterator<Item = Weekday>) -> Self {
        self.inner = self.inner.weekdays(weekdays.into_iter().map(hegel_weekday));
        self
    }

    /// Draw only dates in these months, moving a drawn day in another
    /// month to the nearest allowed day inside the bounds as
    /// [`weekdays`](Self::weekdays) does.
    pub fn months(mut self, months: impl IntoIterator<Item = Month>) -> Self {
        self.inner = self.inner.months(months.into_iter().map(hegel_month));
        self
    }
}

impl Generator<Date> for DateGenerator {
    fn label(&self) -> u64 {
        DATE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Date {
        time_date(self.inner.do_draw(tc)).unwrap()
    }
}

impl PrintableGenerator<Date> for DateGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Date {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::Date`] values.
///
/// By default any date from `Date::MIN` to `Date::MAX` (years ±9999, or
/// ±999,999 with the `time/large-dates` feature) on any weekday and in
/// any month; shrinks toward 2000-01-01. See [`DateGenerator`] for
/// builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::time as time_gs;
/// use time::{Date, Month, Weekday};
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = Date::from_calendar_date(2024, Month::January, 1).unwrap();
///     let d = tc.draw(time_gs::dates().min_value(min).weekdays([Weekday::Saturday, Weekday::Sunday]));
///     assert!(d >= min);
/// }
/// ```
pub fn dates() -> DateGenerator {
    DateGenerator {
        inner: gs::dates()
            .min_value(hegel_date(Date::MIN))
            .max_value(hegel_date(Date::MAX)),
    }
}

/// Generator for [`time::Time`] values. Created by [`times()`].
pub struct TimeGenerator {
    inner: gs::TimeGenerator,
}

impl TimeGenerator {
    /// Set the minimum time (inclusive).
    pub fn min_value(mut self, min: Time) -> Self {
        self.inner = self.inner.min_value(hegel_time(min));
        self
    }

    /// Set the maximum time (inclusive).
    pub fn max_value(mut self, max: Time) -> Self {
        self.inner = self.inner.max_value(hegel_time(max));
        self
    }

    /// Draw only times that are a whole number of `precision` units
    /// since midnight.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.inner = self.inner.precision(precision);
        self
    }
}

impl Generator<Time> for TimeGenerator {
    fn label(&self) -> u64 {
        TIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Time {
        time_time(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<Time> for TimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Time {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::Time`] values.
///
/// By default any time of day at nanosecond precision; shrinks toward
/// midnight. See [`TimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::Precision;
/// use hegel::extras::time as time_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let t = tc.draw(time_gs::times().precision(Precision::Seconds));
///     assert_eq!(t.nanosecond(), 0);
/// }
/// ```
pub fn times() -> TimeGenerator {
    TimeGenerator { inner: gs::times() }
}

/// Generator for [`time::PrimitiveDateTime`] values. Created by
/// [`primitive_datetimes()`].
pub struct PrimitiveDateTimeGenerator {
    inner: gs::DateTimeGenerator,
}

impl PrimitiveDateTimeGenerator {
    /// Set the minimum datetime (inclusive).
    pub fn min_value(mut self, min: PrimitiveDateTime) -> Self {
        self.inner = self.inner.min_value(hegel_datetime(min));
        self
    }

    /// Set the maximum datetime (inclusive).
    pub fn max_value(mut self, max: PrimitiveDateTime) -> Self {
        self.inner = self.inner.max_value(hegel_datetime(max));
        self
    }

    /// Draw only datetimes whose time is a whole number of `precision`
    /// units since midnight.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.inner = self.inner.precision(precision);
        self
    }
}

impl Generator<PrimitiveDateTime> for PrimitiveDateTimeGenerator {
    fn label(&self) -> u64 {
        PRIMITIVE_DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> PrimitiveDateTime {
        time_datetime(self.inner.do_draw(tc)).unwrap()
    }
}

impl PrintableGenerator<PrimitiveDateTime> for PrimitiveDateTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> PrimitiveDateTime {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::PrimitiveDateTime`] values.
///
/// By default any wall-clock time from `PrimitiveDateTime::MIN` to
/// `PrimitiveDateTime::MAX` (the range of [`dates()`]) at nanosecond
/// precision; shrinks toward 2000-01-01T00:00:00. See
/// [`PrimitiveDateTimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::time as time_gs;
/// use time::{Date, Month, PrimitiveDateTime, Time};
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let date = Date::from_calendar_date(2024, Month::January, 1).unwrap();
///     let min = PrimitiveDateTime::new(date, Time::MIDNIGHT);
///     let dt = tc.draw(time_gs::primitive_datetimes().min_value(min));
///     assert!(dt >= min);
/// }
/// ```
pub fn primitive_datetimes() -> PrimitiveDateTimeGenerator {
    PrimitiveDateTimeGenerator {
        inner: gs::datetimes()
            .min_value(hegel_datetime(PrimitiveDateTime::MIN))
            .max_value(hegel_datetime(PrimitiveDateTime::MAX)),
    }
}

/// Generator for [`time::UtcOffset`] values. Created by [`utc_offsets()`].
pub struct UtcOffsetGenerator {
    inner: gs::UtcOffsetGenerator,
}

impl UtcOffsetGenerator {
    /// Set the minimum offset (inclusive).
    pub fn min_value(mut self, min: UtcOffset) -> Self {
        self.inner = self.inner.min_value(hegel_offset(min));
        self
    }

    /// Set the maximum offset (inclusive).
    pub fn max_value(mut self, max: UtcOffset) -> Self {
        self.inner = self.inner.max_value(hegel_offset(max));
        self
    }

    /// Draw only offsets that are a whole number of `granularity` units.
    pub fn granularity(mut self, granularity: Granularity) -> Self {
        self.inner = self.inner.granularity(granularity);
        self
    }
}

impl Generator<UtcOffset> for UtcOffsetGenerator {
    fn label(&self) -> u64 {
        UTC_OFFSET_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> UtcOffset {
        let datetime::UtcOffset::Known(seconds) = self.inner.do_draw(tc) else {
            unreachable!()
        };
        UtcOffset::from_whole_seconds(seconds).unwrap()
    }
}

impl PrintableGenerator<UtcOffset> for UtcOffsetGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> UtcOffset {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::UtcOffset`] values.
///
/// By default any offset within ±25:59:59 at second granularity; shrinks
/// toward `UtcOffset::UTC`. One draw in twenty on average targets an
/// offset some time zone has used, so `+05:45` and `-00:44:30` appear.
/// See [`UtcOffsetGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::datetime::Granularity;
/// use hegel::extras::time as time_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let o = tc.draw(time_gs::utc_offsets().granularity(Granularity::Minutes));
///     assert_eq!(o.seconds_past_minute(), 0);
/// }
/// ```
pub fn utc_offsets() -> UtcOffsetGenerator {
    UtcOffsetGenerator {
        inner: gs::utc_offsets(),
    }
}

enum OffsetDateTimeBounds {
    Civil(PrimitiveDateTime, PrimitiveDateTime),
    Instant(OffsetDateTime, OffsetDateTime),
}

/// The earliest instant whose wall time is in range at every offset.
fn earliest_instant() -> OffsetDateTime {
    PrimitiveDateTime::MIN.assume_offset(UtcOffset::from_hms(-25, -59, -59).unwrap())
}

/// The latest instant whose wall time is in range at every offset.
fn latest_instant() -> OffsetDateTime {
    PrimitiveDateTime::MAX.assume_offset(UtcOffset::from_hms(25, 59, 59).unwrap())
}

/// Generator for [`time::OffsetDateTime`] values. Created by
/// [`offset_datetimes()`].
pub struct OffsetDateTimeGenerator {
    inner: gs::ZonedDateTimeGenerator,
    bounds: OffsetDateTimeBounds,
}

impl OffsetDateTimeGenerator {
    /// Draw the time zone whose offset the value carries from `zones`
    /// rather than from every bundled IANA name.
    pub fn timezones(mut self, zones: gs::TimeZoneGenerator) -> Self {
        self.inner = self.inner.timezones(zones);
        self
    }

    fn civil_bounds(mut self, min: PrimitiveDateTime, max: PrimitiveDateTime) -> Self {
        self.bounds = OffsetDateTimeBounds::Civil(min, max);
        self.inner = self
            .inner
            .min_value(hegel_datetime(min))
            .max_value(hegel_datetime(max));
        self
    }

    fn instant_bounds(mut self, min: OffsetDateTime, max: OffsetDateTime) -> Self {
        self.bounds = OffsetDateTimeBounds::Instant(min, max);
        self.inner = self
            .inner
            .min_instant(hegel_timestamp(min))
            .max_instant(hegel_timestamp(max));
        self
    }

    /// Set the minimum wall-clock time (inclusive), in whatever zone is
    /// drawn. Replaces instant bounds.
    pub fn min_value(self, min: PrimitiveDateTime) -> Self {
        let max = match self.bounds {
            OffsetDateTimeBounds::Civil(_, max) => max,
            OffsetDateTimeBounds::Instant(..) => PrimitiveDateTime::MAX,
        };
        self.civil_bounds(min, max)
    }

    /// Set the maximum wall-clock time (inclusive), in whatever zone is
    /// drawn. Replaces instant bounds.
    pub fn max_value(self, max: PrimitiveDateTime) -> Self {
        let min = match self.bounds {
            OffsetDateTimeBounds::Civil(min, _) => min,
            OffsetDateTimeBounds::Instant(..) => PrimitiveDateTime::MIN,
        };
        self.civil_bounds(min, max)
    }

    /// Set the minimum instant (inclusive); only `min`'s instant matters,
    /// not its offset. Replaces wall-clock bounds.
    pub fn min_instant(self, min: OffsetDateTime) -> Self {
        let max = match self.bounds {
            OffsetDateTimeBounds::Instant(_, max) => max,
            OffsetDateTimeBounds::Civil(..) => latest_instant(),
        };
        self.instant_bounds(min, max)
    }

    /// Set the maximum instant (inclusive); only `max`'s instant matters,
    /// not its offset. Replaces wall-clock bounds.
    pub fn max_instant(self, max: OffsetDateTime) -> Self {
        let min = match self.bounds {
            OffsetDateTimeBounds::Instant(min, _) => min,
            OffsetDateTimeBounds::Civil(..) => earliest_instant(),
        };
        self.instant_bounds(min, max)
    }

    /// Draw only wall-clock times that are a whole number of `precision`
    /// units since midnight.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.inner = self.inner.precision(precision);
        self
    }

    /// Which instant a wall time the clocks repeated means
    /// ([`Ambiguous::Both`] by default).
    pub fn ambiguous(mut self, ambiguous: Ambiguous) -> Self {
        self.inner = self.inner.ambiguous(ambiguous);
        self
    }
}

impl Generator<OffsetDateTime> for OffsetDateTimeGenerator {
    fn label(&self) -> u64 {
        OFFSET_DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> OffsetDateTime {
        let z = self.inner.do_draw(tc);
        let offset = match z.offset {
            datetime::UtcOffset::Known(seconds) => UtcOffset::from_whole_seconds(seconds).unwrap(),
            datetime::UtcOffset::Unknown => UtcOffset::UTC,
        };
        let civil = time_datetime(z.civil).unwrap_or_else(|| tc.reject());
        let dt = OffsetDateTime::new_in_offset(civil.date(), civil.time(), offset);
        if dt.checked_to_offset(UtcOffset::UTC).is_none() {
            tc.reject();
        }
        dt
    }
}

impl PrintableGenerator<OffsetDateTime> for OffsetDateTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> OffsetDateTime {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::OffsetDateTime`] values: a wall-clock time in a drawn
/// IANA time zone, carrying the offset that zone had in force.
///
/// By default any zone [`timezones`](crate::generators::timezones)
/// draws and any wall-clock time of the [`primitive_datetimes()`] range at
/// nanosecond precision, with both repeats of a wall time the clocks
/// repeated drawn and a wall time the clocks skipped never drawn; shrinks
/// to `UTC`, then toward 2000-01-01T00:00:00. One draw in twenty on
/// average targets a transition of the zone, so instants on either side
/// of a daylight-saving change are reached. The offset is the engine's
/// own reading of the zone, from the IANA release
/// [`tzdb_version`](crate::datetime::tzdb_version) reports, since the
/// `time` crate has no database of its own to resolve the wall time
/// with. The zone with an unknown
/// offset, `Etc/Unknown`, is read as UTC. The value's instant lies within
/// the range too, so `to_utc` never panics on it: a wall time within a
/// day of the range's ends whose instant at the drawn offset falls outside
/// it is rejected as if by `assume(false)`, as is a wall time an instant
/// bound puts past the range. See [`OffsetDateTimeGenerator`] for builder
/// methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::time as time_gs;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zones = gs::timezones().names(["Europe/London", "America/New_York"]);
///     let dt = tc.draw(time_gs::offset_datetimes().timezones(zones));
///     assert!(dt.offset().whole_hours() >= -5 && dt.offset().whole_hours() <= 1);
/// }
/// ```
pub fn offset_datetimes() -> OffsetDateTimeGenerator {
    OffsetDateTimeGenerator {
        inner: gs::zoned_datetimes()
            .min_value(hegel_datetime(PrimitiveDateTime::MIN))
            .max_value(hegel_datetime(PrimitiveDateTime::MAX))
            .allow_imaginary(false),
        bounds: OffsetDateTimeBounds::Civil(PrimitiveDateTime::MIN, PrimitiveDateTime::MAX),
    }
}

/// Generator for [`time::Duration`] values. Created by [`durations()`].
pub struct DurationGenerator {
    inner: gs::SignedDurationGenerator,
}

impl DurationGenerator {
    /// Set the minimum duration (inclusive).
    pub fn min_value(mut self, min: Duration) -> Self {
        self.inner = self.inner.min_value(hegel_duration(min));
        self
    }

    /// Set the maximum duration (inclusive).
    pub fn max_value(mut self, max: Duration) -> Self {
        self.inner = self.inner.max_value(hegel_duration(max));
        self
    }

    /// Draw only durations that are a whole number of `precision` units.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.inner = self.inner.precision(precision);
        self
    }
}

impl Generator<Duration> for DurationGenerator {
    fn label(&self) -> u64 {
        DURATION_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Duration {
        let d = self.inner.do_draw(tc);
        Duration::new(d.seconds, d.nanoseconds)
    }
}

impl PrintableGenerator<Duration> for DurationGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Duration {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::Duration`] values.
///
/// By default any duration from `Duration::MIN` to `Duration::MAX` at
/// nanosecond precision; shrinks toward zero. One draw in twenty on
/// average targets a bound or a duration such as one unit, 24 hours or
/// the `i32` seconds limit. See [`DurationGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::time as time_gs;
/// use time::Duration;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let d = tc.draw(time_gs::durations().min_value(Duration::ZERO));
///     assert!(d >= Duration::ZERO);
/// }
/// ```
pub fn durations() -> DurationGenerator {
    DurationGenerator {
        inner: gs::signed_durations()
            .min_value(hegel_duration(Duration::MIN))
            .max_value(hegel_duration(Duration::MAX)),
    }
}

/// Generator for [`time::Weekday`] values. Created by [`weekdays()`].
pub struct WeekdayGenerator;

impl Generator<Weekday> for WeekdayGenerator {
    fn label(&self) -> u64 {
        WEEKDAY_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Weekday {
        let n = gs::integers::<u8>().min_value(0).max_value(6).do_draw(tc);
        Weekday::Monday.nth_next(n)
    }
}

impl PrintableGenerator<Weekday> for WeekdayGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Weekday {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::Weekday`] values; shrinks toward Monday.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::time as time_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let w = tc.draw(time_gs::weekdays());
///     assert!(w.number_days_from_monday() < 7);
/// }
/// ```
pub fn weekdays() -> WeekdayGenerator {
    WeekdayGenerator
}

/// Generator for [`time::Month`] values. Created by [`months()`].
pub struct MonthGenerator;

impl Generator<Month> for MonthGenerator {
    fn label(&self) -> u64 {
        MONTH_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Month {
        let n = gs::integers::<u8>().min_value(1).max_value(12).do_draw(tc);
        Month::try_from(n).unwrap()
    }
}

impl PrintableGenerator<Month> for MonthGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Month {
        gs::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`time::Month`] values; shrinks toward January.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::time as time_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let m = tc.draw(time_gs::months());
///     assert!((1..=12).contains(&(m as u8)));
/// }
/// ```
pub fn months() -> MonthGenerator {
    MonthGenerator
}
