use crate::datetime as hd;
use crate::datetime::{Ambiguous, Granularity, Precision};
use crate::generators as gs;
use crate::generators::{
    DefaultGenerator, Generator, PrintableGenerator, TestCase, combine_labels, hashsets, integers,
    label_from_name,
};
use crate::pretty::PrettyPrinter;
use std::sync::LazyLock;

static WEEKDAY_SET_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.chrono.weekday_sets"));
static FIXED_OFFSET_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.chrono.fixed_offsets"));
static TIME_DELTA_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.chrono.time_deltas"));
static NAIVE_DATE_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.chrono.naive_dates"));
static NAIVE_TIME_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.chrono.naive_times"));
static NAIVE_DATETIME_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.chrono.naive_datetimes"));
static NAIVE_WEEK_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.chrono.naive_weeks"));
static DATETIME_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.chrono.datetimes"));
#[cfg(feature = "chrono-tz")]
static TIME_ZONE_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.chrono.timezones"));

use crate::test_case::invalid_argument;
use chrono::{
    DateTime, Datelike, FixedOffset, Month, NaiveDate, NaiveDateTime, NaiveTime, NaiveWeek, Offset,
    TimeDelta, TimeZone, Timelike, Utc, Weekday, WeekdaySet,
};
use std::marker::PhantomData;
use std::sync::OnceLock;

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// Whether `t` uses chrono's leap-second representation, a `nanosecond()`
/// of a billion or more, which the engine's time draw cannot hold.
fn is_leap_second(t: NaiveTime) -> bool {
    t.nanosecond() >= 1_000_000_000
}

/// Default upper bound for [`naive_times`]: the last nanosecond of the day
/// in the ordinary representation.
fn naive_time_default_max() -> NaiveTime {
    NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap()
}

/// Convert a [`NaiveDate`] to the engine's date. Every `NaiveDate` fits:
/// chrono spans years ±262143 and the engine accepts ±999999.
fn hegel_date(d: NaiveDate) -> hd::Date {
    hd::Date {
        year: d.year(),
        month: d.month() as u8,
        day: d.day() as u8,
    }
}

fn naive_date(d: hd::Date) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year, u32::from(d.month), u32::from(d.day)).unwrap()
}

/// Convert a [`NaiveTime`] in the ordinary representation (see
/// [`is_leap_second`]) to the engine's time.
fn hegel_time(t: NaiveTime) -> hd::Time {
    hd::Time {
        hour: t.hour() as u8,
        minute: t.minute() as u8,
        second: t.second() as u8,
        nanosecond: t.nanosecond(),
    }
}

fn naive_time(t: hd::Time) -> NaiveTime {
    NaiveTime::from_hms_nano_opt(
        u32::from(t.hour),
        u32::from(t.minute),
        u32::from(t.second),
        t.nanosecond,
    )
    .unwrap()
}

fn hegel_datetime(dt: NaiveDateTime) -> hd::DateTime {
    hd::DateTime {
        date: hegel_date(dt.date()),
        time: hegel_time(dt.time()),
    }
}

fn naive_datetime(dt: hd::DateTime) -> NaiveDateTime {
    NaiveDateTime::new(naive_date(dt.date), naive_time(dt.time))
}

fn hegel_timestamp(dt: DateTime<Utc>) -> hd::Timestamp {
    hd::Timestamp {
        second: dt.timestamp(),
        nanosecond: dt.timestamp_subsec_nanos(),
    }
}

fn hegel_weekday(day: Weekday) -> hd::Weekday {
    const DAYS: [hd::Weekday; 7] = [
        hd::Weekday::Monday,
        hd::Weekday::Tuesday,
        hd::Weekday::Wednesday,
        hd::Weekday::Thursday,
        hd::Weekday::Friday,
        hd::Weekday::Saturday,
        hd::Weekday::Sunday,
    ];
    DAYS[day.num_days_from_monday() as usize]
}

fn hegel_month(month: Month) -> hd::Month {
    const MONTHS: [hd::Month; 12] = [
        hd::Month::January,
        hd::Month::February,
        hd::Month::March,
        hd::Month::April,
        hd::Month::May,
        hd::Month::June,
        hd::Month::July,
        hd::Month::August,
        hd::Month::September,
        hd::Month::October,
        hd::Month::November,
        hd::Month::December,
    ];
    MONTHS[(month.number_from_month() - 1) as usize]
}

/// Generator for [`chrono::WeekdaySet`] values. Created by [`weekday_sets()`].
pub struct WeekdaySetGenerator;

impl Generator<WeekdaySet> for WeekdaySetGenerator {
    fn label(&self) -> u64 {
        *WEEKDAY_SET_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> WeekdaySet {
        let days = hashsets(integers::<u8>().min_value(0).max_value(6))
            .max_size(7)
            .do_draw(tc);
        let mut set = WeekdaySet::EMPTY;
        for n in days {
            set.insert(Weekday::try_from(n).unwrap());
        }
        set
    }
}

impl PrintableGenerator<WeekdaySet> for WeekdaySetGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> WeekdaySet {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::WeekdaySet`] values.
///
/// # Example
///
/// ```no_run
/// use hegel::extras::chrono as chrono_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let s: chrono::WeekdaySet = tc.draw(chrono_gs::weekday_sets());
///     assert!(s.len() <= 7);
/// }
/// ```
pub fn weekday_sets() -> WeekdaySetGenerator {
    WeekdaySetGenerator
}

/// Generator for [`chrono::FixedOffset`] values. Created by [`fixed_offsets()`].
pub struct FixedOffsetGenerator {
    min_value: FixedOffset,
    max_value: FixedOffset,
    granularity: Granularity,
}

impl FixedOffsetGenerator {
    /// Set the minimum offset (inclusive).
    pub fn min_value(mut self, min: FixedOffset) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum offset (inclusive).
    pub fn max_value(mut self, max: FixedOffset) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only offsets that are a whole number of hours, quarter hours,
    /// minutes or seconds ([`Granularity::Seconds`] by default).
    pub fn granularity(mut self, granularity: Granularity) -> Self {
        self.granularity = granularity;
        self
    }
}

impl Generator<FixedOffset> for FixedOffsetGenerator {
    fn label(&self) -> u64 {
        *FIXED_OFFSET_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> FixedOffset {
        let offset = gs::utc_offsets()
            .min_value(hd::UtcOffset::Known(self.min_value.local_minus_utc()))
            .max_value(hd::UtcOffset::Known(self.max_value.local_minus_utc()))
            .granularity(self.granularity)
            .do_draw(tc);
        let hd::UtcOffset::Known(seconds) = offset else {
            unreachable!()
        };
        FixedOffset::east_opt(seconds).unwrap()
    }
}

impl PrintableGenerator<FixedOffset> for FixedOffsetGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> FixedOffset {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::FixedOffset`] values.
///
/// By default any offset chrono can hold, up to a second short of a day
/// either side of UTC, at second granularity; shrinks toward UTC.
///
/// # Example
///
/// ```no_run
/// use chrono::FixedOffset;
/// use hegel::extras::chrono as chrono_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let max = FixedOffset::east_opt(12 * 3600).unwrap();
///     let off = tc.draw(chrono_gs::fixed_offsets().max_value(max));
///     assert!(off.local_minus_utc() <= 12 * 3600);
/// }
/// ```
pub fn fixed_offsets() -> FixedOffsetGenerator {
    FixedOffsetGenerator {
        min_value: FixedOffset::west_opt(86_399).unwrap(),
        max_value: FixedOffset::east_opt(86_399).unwrap(),
        granularity: Granularity::Seconds,
    }
}

/// The engine's reading of a [`TimeDelta`]: whole seconds and the
/// nanoseconds past them, both of the delta's sign.
fn hegel_time_delta(d: TimeDelta) -> hd::SignedDuration {
    hd::SignedDuration {
        seconds: d.num_seconds(),
        nanoseconds: d.subsec_nanos(),
    }
}

/// The [`TimeDelta`] of an engine duration, which lies within
/// `TimeDelta::MIN..=TimeDelta::MAX` because the draw's bounds do.
fn chrono_time_delta(d: hd::SignedDuration) -> TimeDelta {
    let (seconds, nanoseconds) = if d.nanoseconds < 0 {
        (d.seconds - 1, d.nanoseconds + 1_000_000_000)
    } else {
        (d.seconds, d.nanoseconds)
    };
    TimeDelta::new(seconds, nanoseconds as u32).unwrap()
}

/// Generator for [`chrono::TimeDelta`] values. Created by [`time_deltas()`].
pub struct TimeDeltaGenerator {
    inner: gs::SignedDurationGenerator,
}

impl TimeDeltaGenerator {
    /// Set the minimum delta (inclusive).
    pub fn min_value(mut self, min: TimeDelta) -> Self {
        self.inner = self.inner.min_value(hegel_time_delta(min));
        self
    }

    /// Set the maximum delta (inclusive).
    pub fn max_value(mut self, max: TimeDelta) -> Self {
        self.inner = self.inner.max_value(hegel_time_delta(max));
        self
    }

    /// Draw only deltas that are a whole number of `precision` units.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.inner = self.inner.precision(precision);
        self
    }
}

impl Generator<TimeDelta> for TimeDeltaGenerator {
    fn label(&self) -> u64 {
        *TIME_DELTA_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> TimeDelta {
        chrono_time_delta(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<TimeDelta> for TimeDeltaGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> TimeDelta {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::TimeDelta`] values.
///
/// By default any delta chrono holds, `TimeDelta::MIN..=TimeDelta::MAX`,
/// at nanosecond precision; shrinks toward zero, and targets the bounds
/// and a few round durations (a unit, a second, a minute, a day, a week,
/// the `i32`-seconds and `i64`-nanoseconds limits) of either sign. See
/// [`TimeDeltaGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use chrono::TimeDelta;
/// use hegel::extras::chrono as chrono_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let max = TimeDelta::seconds(60);
///     let d = tc.draw(chrono_gs::time_deltas().min_value(TimeDelta::zero()).max_value(max));
///     assert!(d >= TimeDelta::zero() && d <= max);
/// }
/// ```
pub fn time_deltas() -> TimeDeltaGenerator {
    TimeDeltaGenerator {
        inner: gs::signed_durations()
            .min_value(hegel_time_delta(TimeDelta::MIN))
            .max_value(hegel_time_delta(TimeDelta::MAX)),
    }
}

/// Generator for [`chrono::NaiveDate`] values. Created by [`naive_dates()`].
///
/// A thin map over [`gs::dates()`], so it draws, targets and shrinks exactly
/// as that generator does.
pub struct NaiveDateGenerator {
    inner: gs::DateGenerator,
}

impl NaiveDateGenerator {
    /// Set the minimum date (inclusive).
    pub fn min_value(self, min: NaiveDate) -> Self {
        Self {
            inner: self.inner.min_value(hegel_date(min)),
        }
    }

    /// Set the maximum date (inclusive).
    pub fn max_value(self, max: NaiveDate) -> Self {
        Self {
            inner: self.inner.max_value(hegel_date(max)),
        }
    }

    /// Draw only dates on these weekdays. A drawn day on another weekday
    /// is moved to the nearest allowed day inside the bounds rather than
    /// rejected, so the draw never filters.
    pub fn weekdays(self, weekdays: impl IntoIterator<Item = Weekday>) -> Self {
        Self {
            inner: self.inner.weekdays(weekdays.into_iter().map(hegel_weekday)),
        }
    }

    /// Draw only dates in these months, moving a drawn day in another
    /// month to the nearest allowed day inside the bounds as
    /// [`weekdays`](Self::weekdays) does.
    pub fn months(self, months: impl IntoIterator<Item = Month>) -> Self {
        Self {
            inner: self.inner.months(months.into_iter().map(hegel_month)),
        }
    }
}

impl Generator<NaiveDate> for NaiveDateGenerator {
    fn label(&self) -> u64 {
        *NAIVE_DATE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> NaiveDate {
        naive_date(self.inner.do_draw(tc))
    }
}

impl PrintableGenerator<NaiveDate> for NaiveDateGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> NaiveDate {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::NaiveDate`] values.
///
/// By default any date chrono can hold, years ±262143, on any weekday and
/// in any month; shrinks toward 2000-01-01. See [`NaiveDateGenerator`] for
/// builder methods.
///
/// # Example
///
/// ```no_run
/// use chrono::{NaiveDate, Weekday};
/// use hegel::extras::chrono as chrono_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
///     let max = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
///     let d = tc.draw(
///         chrono_gs::naive_dates()
///             .min_value(min)
///             .max_value(max)
///             .weekdays([Weekday::Sat, Weekday::Sun]),
///     );
///     assert!(d >= min && d <= max);
/// }
/// ```
pub fn naive_dates() -> NaiveDateGenerator {
    NaiveDateGenerator {
        inner: gs::dates()
            .min_value(hegel_date(NaiveDate::MIN))
            .max_value(hegel_date(NaiveDate::MAX)),
    }
}

/// Generator for [`chrono::NaiveTime`] values. Created by [`naive_times()`].
pub struct NaiveTimeGenerator {
    min_value: NaiveTime,
    max_value: NaiveTime,
    precision: Precision,
}

impl NaiveTimeGenerator {
    /// Set the minimum time (inclusive).
    pub fn min_value(mut self, min: NaiveTime) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum time (inclusive).
    pub fn max_value(mut self, max: NaiveTime) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only times that are a whole number of `precision` units
    /// since midnight ([`Precision::Nanoseconds`] by default).
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

impl Generator<NaiveTime> for NaiveTimeGenerator {
    fn label(&self) -> u64 {
        *NAIVE_TIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> NaiveTime {
        if is_leap_second(self.min_value) || is_leap_second(self.max_value) {
            invalid_argument!("naive_times bounds cannot use a leap second");
        }
        naive_time(
            gs::times()
                .min_value(hegel_time(self.min_value))
                .max_value(hegel_time(self.max_value))
                .precision(self.precision)
                .do_draw(tc),
        )
    }
}

impl PrintableGenerator<NaiveTime> for NaiveTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> NaiveTime {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::NaiveTime`] values.
///
/// By default any time of day at nanosecond precision; shrinks toward
/// midnight. A bound in chrono's leap-second representation (a
/// `nanosecond()` of a billion or more) is a usage error, and no drawn
/// time uses it. See [`NaiveTimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use chrono::NaiveTime;
/// use hegel::datetime::Precision;
/// use hegel::extras::chrono as chrono_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let max = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
///     let t = tc.draw(chrono_gs::naive_times().max_value(max).precision(Precision::Seconds));
///     assert!(t <= max);
/// }
/// ```
pub fn naive_times() -> NaiveTimeGenerator {
    NaiveTimeGenerator {
        min_value: NaiveTime::MIN,
        max_value: naive_time_default_max(),
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`chrono::NaiveDateTime`] values. Created by [`naive_datetimes()`].
pub struct NaiveDateTimeGenerator {
    min_value: NaiveDateTime,
    max_value: NaiveDateTime,
    precision: Precision,
}

impl NaiveDateTimeGenerator {
    /// Set the minimum datetime (inclusive).
    pub fn min_value(mut self, min: NaiveDateTime) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum datetime (inclusive).
    pub fn max_value(mut self, max: NaiveDateTime) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only datetimes whose time is a whole number of `precision`
    /// units since midnight ([`Precision::Nanoseconds`] by default).
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

impl Generator<NaiveDateTime> for NaiveDateTimeGenerator {
    fn label(&self) -> u64 {
        *NAIVE_DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> NaiveDateTime {
        if is_leap_second(self.min_value.time()) || is_leap_second(self.max_value.time()) {
            invalid_argument!("naive_datetimes bounds cannot use a leap second");
        }
        naive_datetime(
            gs::datetimes()
                .min_value(hegel_datetime(self.min_value))
                .max_value(hegel_datetime(self.max_value))
                .precision(self.precision)
                .do_draw(tc),
        )
    }
}

impl PrintableGenerator<NaiveDateTime> for NaiveDateTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> NaiveDateTime {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::NaiveDateTime`] values.
///
/// Defaults span chrono's full `DateTime<Utc>` window —
/// [`DateTime::<Utc>::MIN_UTC`] through [`DateTime::<Utc>::MAX_UTC`] — at
/// nanosecond precision; shrinks toward 2000-01-01T00:00:00. A bound in
/// chrono's leap-second representation is a usage error. See
/// [`NaiveDateTimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
/// use hegel::extras::chrono as chrono_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = NaiveDateTime::new(
///         NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
///         NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
///     );
///     let max = NaiveDateTime::new(
///         NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
///         NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap(),
///     );
///     let dt = tc.draw(chrono_gs::naive_datetimes().min_value(min).max_value(max));
///     assert!(dt >= min && dt <= max);
/// }
/// ```
pub fn naive_datetimes() -> NaiveDateTimeGenerator {
    NaiveDateTimeGenerator {
        min_value: DateTime::<Utc>::MIN_UTC.naive_utc(),
        max_value: DateTime::<Utc>::MAX_UTC.naive_utc(),
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`chrono::NaiveWeek`] values. Created by [`naive_weeks()`].
pub struct NaiveWeekGenerator<S = <Weekday as DefaultGenerator>::Generator> {
    date_gen: NaiveDateGenerator,
    start_gen: S,
    label: u64,
}

impl<S> NaiveWeekGenerator<S> {
    /// Set the minimum source date (inclusive).
    pub fn min_date(mut self, min: NaiveDate) -> Self {
        self.date_gen = self.date_gen.min_value(min);
        self
    }

    /// Set the maximum source date (inclusive).
    pub fn max_date(mut self, max: NaiveDate) -> Self {
        self.date_gen = self.date_gen.max_value(max);
        self
    }

    /// Replace the start-of-week generator.
    pub fn weekday_starts<S2>(self, start_gen: S2) -> NaiveWeekGenerator<S2>
    where
        S2: Generator<Weekday>,
    {
        NaiveWeekGenerator {
            date_gen: self.date_gen,
            label: combine_labels(&[*NAIVE_WEEK_LABEL, start_gen.label()]),
            start_gen,
        }
    }
}

impl<S: Generator<Weekday>> Generator<NaiveWeek> for NaiveWeekGenerator<S> {
    fn label(&self) -> u64 {
        self.label
    }

    fn do_draw(&self, tc: &TestCase) -> NaiveWeek {
        let (date, start) =
            tc.draw_silent(crate::generators::tuples2(&self.date_gen, &self.start_gen));
        date.week(start)
    }
}

impl<S: Generator<Weekday>> PrintableGenerator<NaiveWeek> for NaiveWeekGenerator<S> {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> NaiveWeek {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::NaiveWeek`] values.
///
/// Each draw produces a 7-day window around a randomly chosen [`NaiveDate`],
/// with the start-of-week [`Weekday`] also drawn at random (full 7-day range
/// by default). Use [`weekday_starts`](NaiveWeekGenerator::weekday_starts) to substitute a
/// different start-day strategy — e.g. `gs::just(Weekday::Sun)` for Sunday-only.
///
/// # Example
///
/// ```no_run
/// use chrono::Weekday;
/// use hegel::extras::chrono as chrono_gs;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     // Default: random start day per draw
///     let _ = tc.draw(chrono_gs::naive_weeks());
///
///     // All Sunday-start weeks
///     let _ = tc.draw(chrono_gs::naive_weeks().weekday_starts(gs::just(Weekday::Sun)));
/// }
/// ```
pub fn naive_weeks() -> NaiveWeekGenerator {
    // Pull the default date range in by a week on each side: a NaiveWeek
    // whose window pokes past NaiveDate::MIN/MAX panics in chrono's own
    // first_day/last_day/days accessors, so default-generated weeks would
    // otherwise be landmines. An explicit min_date/max_date can still opt
    // back into the extremes.
    let start_gen = Weekday::default_generator();
    NaiveWeekGenerator {
        date_gen: naive_dates()
            .min_value(
                NaiveDate::MIN
                    .checked_add_days(chrono::Days::new(7))
                    .unwrap(),
            )
            .max_value(
                NaiveDate::MAX
                    .checked_sub_days(chrono::Days::new(7))
                    .unwrap(),
            ),
        label: combine_labels(&[*NAIVE_WEEK_LABEL, start_gen.label()]),
        start_gen,
    }
}

/// Convert a [`DateTime`] to a total nanosecond count from the Unix epoch.
fn datetime_to_nanos<Tz: TimeZone>(dt: &DateTime<Tz>) -> i128 {
    i128::from(dt.timestamp()) * NANOS_PER_SECOND + i128::from(dt.timestamp_subsec_nanos())
}

/// Inverse of [`datetime_to_nanos`], producing a `DateTime<Utc>`.
fn nanos_to_utc_datetime(n: i128) -> DateTime<Utc> {
    let secs = n.div_euclid(NANOS_PER_SECOND) as i64;
    let nsecs = n.rem_euclid(NANOS_PER_SECOND) as u32;
    DateTime::<Utc>::from_timestamp(secs, nsecs).unwrap()
}

mod zoned {
    use super::{hegel_datetime, hegel_timestamp, is_leap_second};
    use crate::datetime::{Ambiguous, Precision};
    use crate::generators as gs;
    use crate::generators::TestCase;
    use crate::test_case::invalid_argument;
    use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
    use std::sync::OnceLock;

    #[derive(Clone, Copy)]
    pub enum Bounds {
        Civil(NaiveDateTime, NaiveDateTime),
        Instant(DateTime<Utc>, DateTime<Utc>),
    }

    /// Everything a [`DateTime`](super::DateTimeGenerator) draw asks of
    /// the engine's zoned datetime draw besides the zone.
    #[derive(Clone, Copy)]
    pub struct Config {
        pub bounds: Bounds,
        pub precision: Precision,
        pub allow_imaginary: bool,
        pub ambiguous: Ambiguous,
    }

    impl Config {
        pub fn validate(&self) {
            match self.bounds {
                Bounds::Civil(min, max) => {
                    if is_leap_second(min.time()) || is_leap_second(max.time()) {
                        invalid_argument!("datetimes bounds cannot use a leap second");
                    }
                    if min > max {
                        invalid_argument!("Cannot have max_value < min_value");
                    }
                }
                Bounds::Instant(min, max) => {
                    if min > max {
                        invalid_argument!("Cannot have max_instant < min_instant");
                    }
                }
            }
        }

        pub fn generator(&self, zones: gs::TimeZoneGenerator) -> gs::ZonedDateTimeGenerator {
            let generator = gs::zoned_datetimes()
                .timezones(zones)
                .precision(self.precision)
                .allow_imaginary(self.allow_imaginary)
                .ambiguous(self.ambiguous);
            match self.bounds {
                Bounds::Civil(min, max) => generator
                    .min_value(hegel_datetime(min))
                    .max_value(hegel_datetime(max)),
                Bounds::Instant(min, max) => generator
                    .min_instant(hegel_timestamp(min))
                    .max_instant(hegel_timestamp(max)),
            }
        }
    }

    /// A source of zones; `zoned` caches the engine generator a source
    /// builds from `config`, and is reset whenever `config` changes.
    pub trait Zones<Tz: TimeZone> {
        fn label(&self) -> u64;
        fn draw(
            &self,
            tc: &TestCase,
            config: &Config,
            zoned: &OnceLock<gs::ZonedDateTimeGenerator>,
        ) -> DateTime<Tz>;
    }
}

/// A source of time zones for [`datetimes()`]: any generator of [`Utc`]
/// or [`FixedOffset`] values, such as [`gs::just(Utc)`](gs::just) or
/// [`fixed_offsets()`], and, with the `chrono-tz` feature, `timezones()`
/// for IANA zones. The trait is sealed: a zone source has to be one the
/// engine can classify wall times in.
pub trait TimeZones<Tz: TimeZone>: zoned::Zones<Tz> {}

impl<G: Generator<Utc>> TimeZones<Utc> for G {}

impl<G: Generator<Utc>> zoned::Zones<Utc> for G {
    fn label(&self) -> u64 {
        Generator::label(self)
    }

    fn draw(
        &self,
        tc: &TestCase,
        config: &zoned::Config,
        _: &OnceLock<gs::ZonedDateTimeGenerator>,
    ) -> DateTime<Utc> {
        let Utc = tc.draw_silent(self);
        wall_time_at_offset(tc, config, Utc.fix()).and_utc()
    }
}

impl<G: Generator<FixedOffset>> TimeZones<FixedOffset> for G {}

impl<G: Generator<FixedOffset>> zoned::Zones<FixedOffset> for G {
    fn label(&self) -> u64 {
        Generator::label(self)
    }

    fn draw(
        &self,
        tc: &TestCase,
        config: &zoned::Config,
        _: &OnceLock<gs::ZonedDateTimeGenerator>,
    ) -> DateTime<FixedOffset> {
        let offset = tc.draw_silent(self);
        offset
            .from_local_datetime(&wall_time_at_offset(tc, config, offset))
            .single()
            .unwrap()
    }
}

/// Draw a wall time whose instant at `offset` lies within chrono's range
/// and `config`'s bounds, as the date and time nodes of a civil datetime:
/// an offset is not a zone, so no zone node is recorded.
fn wall_time_at_offset(
    tc: &TestCase,
    config: &zoned::Config,
    offset: FixedOffset,
) -> NaiveDateTime {
    let shift = i128::from(offset.local_minus_utc()) * NANOS_PER_SECOND;
    let earliest = datetime_to_nanos(&DateTime::<Utc>::MIN_UTC);
    let latest = datetime_to_nanos(&DateTime::<Utc>::MAX_UTC);
    let (lo, hi) = match config.bounds {
        zoned::Bounds::Civil(min, max) => (
            datetime_to_nanos(&min.and_utc()).max(earliest + shift),
            datetime_to_nanos(&max.and_utc()).min(latest + shift),
        ),
        zoned::Bounds::Instant(min, max) => (
            (datetime_to_nanos(&min) + shift).max(earliest),
            (datetime_to_nanos(&max) + shift).min(latest),
        ),
    };
    if lo > hi {
        tc.reject();
    }
    naive_datetime(
        gs::datetimes()
            .min_value(hegel_datetime(nanos_to_utc_datetime(lo).naive_utc()))
            .max_value(hegel_datetime(nanos_to_utc_datetime(hi).naive_utc()))
            .precision(config.precision)
            .do_draw(tc),
    )
}

/// Generator for [`chrono::DateTime`] values. Created by [`datetimes()`].
pub struct DateTimeGenerator<G = FixedOffsetGenerator, Tz: TimeZone = FixedOffset> {
    tz_gen: G,
    config: zoned::Config,
    zoned: OnceLock<gs::ZonedDateTimeGenerator>,
    label: u64,
    _phantom: PhantomData<fn() -> Tz>,
}

impl<G, Tz: TimeZone> DateTimeGenerator<G, Tz> {
    /// Set the minimum wall-clock datetime (inclusive), in whatever zone
    /// is drawn. Replaces instant bounds.
    pub fn min_value(mut self, min: NaiveDateTime) -> Self {
        let max = match self.config.bounds {
            zoned::Bounds::Civil(_, max) => max,
            zoned::Bounds::Instant(..) => DateTime::<Utc>::MAX_UTC.naive_utc(),
        };
        self.config.bounds = zoned::Bounds::Civil(min, max);
        self.zoned = OnceLock::new();
        self
    }

    /// Set the maximum wall-clock datetime (inclusive), in whatever zone
    /// is drawn. Replaces instant bounds.
    pub fn max_value(mut self, max: NaiveDateTime) -> Self {
        let min = match self.config.bounds {
            zoned::Bounds::Civil(min, _) => min,
            zoned::Bounds::Instant(..) => DateTime::<Utc>::MIN_UTC.naive_utc(),
        };
        self.config.bounds = zoned::Bounds::Civil(min, max);
        self.zoned = OnceLock::new();
        self
    }

    /// Set the minimum instant (inclusive). Replaces wall-clock bounds.
    pub fn min_instant(mut self, min: DateTime<Utc>) -> Self {
        let max = match self.config.bounds {
            zoned::Bounds::Instant(_, max) => max,
            zoned::Bounds::Civil(..) => DateTime::<Utc>::MAX_UTC,
        };
        self.config.bounds = zoned::Bounds::Instant(min, max);
        self.zoned = OnceLock::new();
        self
    }

    /// Set the maximum instant (inclusive). Replaces wall-clock bounds.
    pub fn max_instant(mut self, max: DateTime<Utc>) -> Self {
        let min = match self.config.bounds {
            zoned::Bounds::Instant(min, _) => min,
            zoned::Bounds::Civil(..) => DateTime::<Utc>::MIN_UTC,
        };
        self.config.bounds = zoned::Bounds::Instant(min, max);
        self.zoned = OnceLock::new();
        self
    }

    /// Draw only wall-clock times that are a whole number of `precision`
    /// units since midnight ([`Precision::Nanoseconds`] by default).
    pub fn precision(mut self, precision: Precision) -> Self {
        self.config.precision = precision;
        self.zoned = OnceLock::new();
        self
    }

    /// Whether a wall time the clocks skipped may be drawn (`false` by
    /// default). Allowed, the value is the instant the skipped wall time
    /// shifts forward to, whose own wall time lies past the gap, so it
    /// can exceed [`max_value`](Self::max_value) by the gap's width. Only
    /// an IANA zone has gaps.
    pub fn allow_imaginary(mut self, allow: bool) -> Self {
        self.config.allow_imaginary = allow;
        self.zoned = OnceLock::new();
        self
    }

    /// Which instant a wall time the clocks repeated means
    /// ([`Ambiguous::Both`] by default). Only an IANA zone repeats wall
    /// times.
    pub fn ambiguous(mut self, ambiguous: Ambiguous) -> Self {
        self.config.ambiguous = ambiguous;
        self.zoned = OnceLock::new();
        self
    }

    /// Use the specified timezone source: see [`TimeZones`] for the
    /// sources accepted.
    ///
    /// The returned generator has output type `DateTime<Tz2>` where `Tz2`
    /// is the timezone type produced by `tz_gen`. Bounds and the other
    /// options are timezone-agnostic and carry across.
    pub fn timezones<G2, Tz2>(self, tz_gen: G2) -> DateTimeGenerator<G2, Tz2>
    where
        G2: TimeZones<Tz2>,
        Tz2: TimeZone,
    {
        DateTimeGenerator {
            label: combine_labels(&[*DATETIME_LABEL, <G2 as zoned::Zones<Tz2>>::label(&tz_gen)]),
            tz_gen,
            config: self.config,
            zoned: OnceLock::new(),
            _phantom: PhantomData,
        }
    }
}

impl<G, Tz> Generator<DateTime<Tz>> for DateTimeGenerator<G, Tz>
where
    G: TimeZones<Tz>,
    Tz: TimeZone + Send + Sync + 'static,
{
    fn label(&self) -> u64 {
        self.label
    }

    fn do_draw(&self, tc: &TestCase) -> DateTime<Tz> {
        self.config.validate();
        zoned::Zones::draw(&self.tz_gen, tc, &self.config, &self.zoned)
    }
}

impl<G, Tz> PrintableGenerator<DateTime<Tz>> for DateTimeGenerator<G, Tz>
where
    G: TimeZones<Tz>,
    Tz: TimeZone + Send + Sync + 'static,
{
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> DateTime<Tz> {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`chrono::DateTime`] values: a zone from the
/// [`timezones`](DateTimeGenerator::timezones) source and a wall-clock time
/// in it, drawn through [`gs::zoned_datetimes()`] for an IANA zone and
/// through [`gs::datetimes()`] for [`Utc`] or a [`FixedOffset`], which has
/// no zone to record.
///
/// By default a [`FixedOffset`] from [`fixed_offsets()`] and any wall-clock
/// time chrono can hold at nanosecond precision, shrinking toward
/// 2000-01-01T00:00:00 at UTC. With the `chrono-tz` feature,
/// `timezones()` draws IANA zones instead, and then the
/// [`allow_imaginary`](DateTimeGenerator::allow_imaginary) and
/// [`ambiguous`](DateTimeGenerator::ambiguous) options say what a wall time
/// the clocks skipped or repeated means. See [`DateTimeGenerator`] for the
/// builder methods.
///
/// A `DateTime` holds its instant, so a wall-clock time within a day of
/// chrono's range whose instant in the drawn zone or offset falls outside
/// it is rejected as if by `assume(false)`; bounds that leave the range's
/// last day alone never meet this. So is a wall time chrono-tz's database
/// skips where the engine's does not, and a wall time whose instant under
/// chrono-tz's rules falls outside instant bounds the engine kept it
/// within: the two databases can differ by a release, the engine's being
/// [`tzdb_version`](crate::datetime::tzdb_version) and chrono-tz's
/// `chrono_tz::IANA_TZDB_VERSION`, and chrono-tz applies no
/// daylight-saving rule from 2100 on.
///
/// ```no_run
/// use chrono::{FixedOffset, Utc};
/// use hegel::extras::chrono as chrono_gs;
/// use hegel::generators as gs;
///
/// // Default: DateTime<FixedOffset> with varying offsets
/// let g = chrono_gs::datetimes();
///
/// // All UTC
/// let g = chrono_gs::datetimes().timezones(gs::just(Utc));
///
/// // All in a single fixed offset
/// let off = FixedOffset::east_opt(9 * 3600).unwrap();
/// let g = chrono_gs::datetimes().timezones(gs::just(off));
///
/// // Constrain offsets to a sub-range
/// let g = chrono_gs::datetimes()
///     .timezones(chrono_gs::fixed_offsets().min_value(off));
/// ```
pub fn datetimes() -> DateTimeGenerator<FixedOffsetGenerator, FixedOffset> {
    DateTimeGenerator {
        label: combine_labels(&[*DATETIME_LABEL, *FIXED_OFFSET_LABEL]),
        tz_gen: fixed_offsets(),
        config: zoned::Config {
            bounds: zoned::Bounds::Civil(
                DateTime::<Utc>::MIN_UTC.naive_utc(),
                DateTime::<Utc>::MAX_UTC.naive_utc(),
            ),
            precision: Precision::Nanoseconds,
            allow_imaginary: false,
            ambiguous: Ambiguous::Both,
        },
        zoned: OnceLock::new(),
        _phantom: PhantomData,
    }
}

/// Generator for [`chrono_tz::Tz`] values. Created by [`timezones()`].
#[cfg(feature = "chrono-tz")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono-tz")))]
pub struct TimeZoneGenerator {
    names: Option<Vec<chrono_tz::Tz>>,
    links: bool,
    legacy: bool,
    zones: OnceLock<gs::TimeZoneGenerator>,
}

#[cfg(feature = "chrono-tz")]
impl TimeZoneGenerator {
    /// Draw only these zones. The [`links`](Self::links) and
    /// [`legacy`](Self::legacy) flags do not apply to an explicit list.
    pub fn names(mut self, names: impl IntoIterator<Item = chrono_tz::Tz>) -> Self {
        self.names = Some(names.into_iter().collect());
        self.zones = OnceLock::new();
        self
    }

    /// Whether the default set includes link names such as
    /// `Asia/Calcutta` (on by default).
    pub fn links(mut self, links: bool) -> Self {
        self.links = links;
        self.zones = OnceLock::new();
        self
    }

    /// Whether the default set includes legacy names such as `EST5EDT`
    /// and `US/Eastern` (off by default).
    pub fn legacy(mut self, legacy: bool) -> Self {
        self.legacy = legacy;
        self.zones = OnceLock::new();
        self
    }

    /// The names of the allowed zones, as chrono-tz spells them: every
    /// name it has that the engine's default set would draw, or the
    /// explicit list. A name the engine's database lacks is drawn as an
    /// opaque zone, resolved by chrono-tz alone.
    fn allowed(&self) -> Vec<&'static str> {
        match &self.names {
            Some(names) => names.iter().map(|tz| tz.name()).collect(),
            None => chrono_tz::TZ_VARIANTS
                .iter()
                .map(|tz| tz.name())
                .filter(|name| {
                    hd::info(name).is_none_or(|info| {
                        (self.links || !info.is_link)
                            && (self.legacy || !info.tags.contains(&hd::Tag::Legacy))
                    })
                })
                .collect(),
        }
    }

    fn zone_generator(&self) -> gs::TimeZoneGenerator {
        gs::timezones().names(self.allowed()).opaque(true)
    }
}

#[cfg(feature = "chrono-tz")]
impl Generator<chrono_tz::Tz> for TimeZoneGenerator {
    fn label(&self) -> u64 {
        *TIME_ZONE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> chrono_tz::Tz {
        self.zones
            .get_or_init(|| self.zone_generator())
            .do_draw(tc)
            .name
            .parse()
            .unwrap()
    }
}

#[cfg(feature = "chrono-tz")]
impl PrintableGenerator<chrono_tz::Tz> for TimeZoneGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> chrono_tz::Tz {
        crate::generators::draw_and_print_value(self, tc, printer)
    }
}

#[cfg(feature = "chrono-tz")]
impl TimeZones<chrono_tz::Tz> for TimeZoneGenerator {}

#[cfg(feature = "chrono-tz")]
impl zoned::Zones<chrono_tz::Tz> for TimeZoneGenerator {
    fn label(&self) -> u64 {
        *TIME_ZONE_LABEL
    }

    fn draw(
        &self,
        tc: &TestCase,
        config: &zoned::Config,
        zoned: &OnceLock<gs::ZonedDateTimeGenerator>,
    ) -> DateTime<chrono_tz::Tz> {
        let z = zoned
            .get_or_init(|| config.generator(self.zone_generator()))
            .do_draw(tc);
        let tz: chrono_tz::Tz = z.zone.name.parse().unwrap();
        let instant = DateTime::<Utc>::from_timestamp(z.timestamp.second, z.timestamp.nanosecond)
            .unwrap_or_else(|| tc.reject());
        let dt = chrono_tz_datetime(&z, tz, instant).unwrap_or_else(|| tc.reject());
        if let zoned::Bounds::Instant(min, max) = config.bounds {
            if dt < min || dt > max {
                tc.reject();
            }
        }
        dt
    }
}

/// The chrono-tz value of an engine draw: its wall time resolved by
/// chrono-tz with the engine's fold, or its instant for a wall time both
/// databases skip. `None` when chrono-tz alone skips the wall time, which
/// a newer or older database on its side can do; no value of chrono's
/// has that wall time then.
#[cfg(feature = "chrono-tz")]
fn chrono_tz_datetime(
    z: &hd::ZonedDateTime,
    tz: chrono_tz::Tz,
    instant: DateTime<Utc>,
) -> Option<DateTime<chrono_tz::Tz>> {
    let resolved = tz.from_local_datetime(&naive_datetime(z.civil));
    let picked = if z.fold {
        resolved.latest()
    } else {
        resolved.earliest()
    };
    match (picked, z.kind) {
        (Some(dt), _) => Some(dt),
        (None, hd::Kind::Gap) => Some(instant.with_timezone(&tz)),
        (None, _) => None,
    }
}

/// Generate [`chrono_tz::Tz`] values, for use on their own or as the
/// [`timezones`](DateTimeGenerator::timezones) source of [`datetimes()`].
///
/// The zones are the names chrono-tz and the engine's bundled IANA
/// database share, link names in and legacy names out by default, drawn
/// and shrunk as [`gs::timezones()`] draws them: down a ladder of names
/// to `UTC`, with half the draws weighted toward zones with rare features.
/// See [`TimeZoneGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use chrono::TimeZone;
/// use chrono_tz::Tz;
/// use hegel::extras::chrono as chrono_gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zone = tc.draw(chrono_gs::timezones().names([Tz::Europe__London, Tz::Asia__Tokyo]));
///     assert!(zone == Tz::Europe__London || zone == Tz::Asia__Tokyo);
///     let dt = tc.draw(chrono_gs::datetimes().timezones(chrono_gs::timezones()));
///     assert_eq!(dt.timezone().from_utc_datetime(&dt.naive_utc()), dt);
/// }
/// ```
#[cfg(feature = "chrono-tz")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono-tz")))]
pub fn timezones() -> TimeZoneGenerator {
    TimeZoneGenerator {
        names: None,
        links: true,
        legacy: false,
        zones: OnceLock::new(),
    }
}

#[cfg(all(test, feature = "chrono-tz"))]
#[path = "../../../tests/embedded/extras/chrono_generators_tests.rs"]
mod tests;
