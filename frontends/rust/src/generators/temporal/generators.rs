use std::any::Any;
use std::fmt::Display;
use std::sync::OnceLock;

use super::source::{ZoneSource, default_zone_source};
use super::values::{
    Ambiguous, Date, DateTime, Granularity, Instant, MAX_INSTANT, MAX_OFFSET, MAX_YEAR,
    MIN_INSTANT, MIN_YEAR, Month, Precision, SignedDuration, Span, SpanSign, SpanUnit, Time,
    TimeZone, UtcOffset, Weekday, ZonedDateTime, date_from_c, date_to_c, datetime_from_c,
    datetime_to_c, duration_from_c, duration_to_c, kind_from_c, offset_from_c, span_from_c,
    time_from_c, time_to_c, timestamp_from_c, timestamp_to_c,
};
use crate::ffi;
use crate::ffi::sys as hegel_c;
use crate::generators::{Generator, PrintableGenerator, TestCase, label_from_name};
use crate::pretty::PrettyPrinter;
use crate::test_case::invalid_argument;
use std::sync::LazyLock;

static DATE_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.datetime.dates"));
static TIME_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.datetime.times"));
static DATETIME_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.datetime.datetimes"));
static TIMESTAMP_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.datetime.timestamps"));
static UTC_OFFSET_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.datetime.utc_offsets"));
static TIME_ZONE_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.datetime.timezones"));
static ZONED_DATETIME_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.datetime.zoned_datetimes"));
static SIGNED_DURATION_LABEL: LazyLock<u64> =
    LazyLock::new(|| label_from_name("hegel.datetime.signed_durations"));
static SPAN_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.datetime.spans"));

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

const MIN_TIMESTAMP: Instant = Instant {
    second: MIN_INSTANT,
    nanosecond: 0,
};

const MAX_TIMESTAMP: Instant = Instant {
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

fn imaginary_to_c(allow: bool) -> u32 {
    use hegel_c::hegel_imaginary_t as c;
    (if allow {
        c::HEGEL_IMAGINARY_ALLOW
    } else {
        c::HEGEL_IMAGINARY_AVOID
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

/// Draw from `generator` and print the value's `Display` form: for the
/// values of this module, the engine's text of the value, which every
/// frontend prints a counterexample in.
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
        *DATE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Date {
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
/// use hegel::generators::temporal::{Date, Weekday};
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = Date::new(2024, 1, 1);
///     let d = tc.draw(gs::temporal::dates().min_value(min).weekdays([Weekday::Saturday, Weekday::Sunday]));
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
        *TIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Time {
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
/// use hegel::generators::temporal::Precision;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let t = tc.draw(gs::temporal::times().precision(Precision::Seconds));
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
        *DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> DateTime {
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
/// use hegel::generators::temporal::{Date, DateTime, Time};
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let min = DateTime::new(Date::new(2024, 1, 1), Time::new(0, 0, 0, 0));
///     let dt = tc.draw(gs::temporal::datetimes().min_value(min));
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

/// Generator for [`Instant`] values. Created by [`instants()`].
pub struct InstantGenerator {
    min_value: Instant,
    max_value: Instant,
    precision: Precision,
}

impl InstantGenerator {
    /// Set the minimum instant (inclusive).
    pub fn min_value(mut self, min: Instant) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum instant (inclusive).
    pub fn max_value(mut self, max: Instant) -> Self {
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

impl Generator<Instant> for InstantGenerator {
    fn label(&self) -> u64 {
        *TIMESTAMP_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Instant {
        timestamp_from_c(tc.generate_timestamp(
            timestamp_to_c(self.min_value),
            timestamp_to_c(self.max_value),
            precision_to_c(self.precision),
        ))
    }
}

impl PrintableGenerator<Instant> for InstantGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Instant {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`Instant`] values.
///
/// By default any instant of the engine's range, years ±999,999, at
/// nanosecond precision; shrinks toward 2000-01-01T00:00:00Z. One draw in
/// twenty on average targets an interesting instant: the Unix epoch, the
/// 32-bit and 64-bit boundaries, a leap second's or a time zone
/// transition's. See [`InstantGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::generators::temporal::Instant;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let epoch = Instant::new(0, 0);
///     let t = tc.draw(gs::temporal::instants().min_value(epoch));
///     assert!(t >= epoch);
/// }
/// ```
pub fn instants() -> InstantGenerator {
    InstantGenerator {
        min_value: MIN_TIMESTAMP,
        max_value: MAX_TIMESTAMP,
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`UtcOffset`] values. Created by [`offsets()`].
pub struct OffsetGenerator {
    min_value: UtcOffset,
    max_value: UtcOffset,
    granularity: Granularity,
    allow_unknown: bool,
}

impl OffsetGenerator {
    /// Set the minimum offset (inclusive), a known offset within
    /// ±25:59:59.
    pub fn min_value(mut self, min: UtcOffset) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum offset (inclusive), a known offset within
    /// ±25:59:59.
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
    pub fn allow_unknown(mut self, allow: bool) -> Self {
        self.allow_unknown = allow;
        self
    }
}

fn known_bound(what: &str, bound: UtcOffset) -> i32 {
    match bound {
        UtcOffset::Known(seconds) if seconds.unsigned_abs() <= MAX_OFFSET.unsigned_abs() => seconds,
        UtcOffset::Known(seconds) => invalid_argument!(
            "{what} must be within ±25:59:59 ({MAX_OFFSET} seconds), got {seconds} seconds"
        ),
        UtcOffset::Unknown => invalid_argument!("{what} must be a known offset"),
    }
}

impl Generator<UtcOffset> for OffsetGenerator {
    fn label(&self) -> u64 {
        *UTC_OFFSET_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> UtcOffset {
        let min = known_bound("min_value", self.min_value);
        let max = known_bound("max_value", self.max_value);
        if min > max {
            invalid_argument!("Cannot have max_value < min_value");
        }
        let o = tc.generate_offset(
            min,
            max,
            granularity_to_c(self.granularity),
            self.allow_unknown,
        );
        offset_from_c(o.seconds, o.known)
    }
}

impl PrintableGenerator<UtcOffset> for OffsetGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> UtcOffset {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`UtcOffset`] values.
///
/// By default any known offset within ±25:59:59 at second granularity;
/// shrinks toward `+00:00`. One draw in twenty on average targets an
/// offset some time zone has used, so `+05:45` and `-00:44:30` appear.
/// See [`OffsetGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::generators::temporal::{Granularity, UtcOffset};
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let o = tc.draw(gs::temporal::offsets().granularity(Granularity::Minutes));
///     let UtcOffset::Known(seconds) = o else { unreachable!() };
///     assert_eq!(seconds % 60, 0);
/// }
/// ```
pub fn offsets() -> OffsetGenerator {
    OffsetGenerator {
        min_value: UtcOffset::Known(-MAX_OFFSET),
        max_value: UtcOffset::Known(MAX_OFFSET),
        granularity: Granularity::Seconds,
        allow_unknown: false,
    }
}

/// Generator for [`TimeZone`] values. Created by [`zones()`].
pub struct ZoneGenerator {
    source: ZoneSource,
    names: Option<Vec<String>>,
    allow_links: bool,
    allow_legacy: bool,
    allow_unknown: bool,
    allow_opaque: bool,
    spec: OnceLock<ffi::TimeZoneSpec>,
}

impl ZoneGenerator {
    /// Draw against `source` rather than the default zone source the
    /// generator read when [`zones()`] built it: its names, and wall times
    /// classified by its data when the generator is the zone generator of
    /// [`zoned_datetimes`].
    pub fn source(mut self, source: ZoneSource) -> Self {
        self.source = source;
        self.spec = OnceLock::new();
        self
    }

    /// Draw only these names, each exactly as the zone source spells it
    /// or `Etc/Unknown`. The [`allow_links`](Self::allow_links) and
    /// [`allow_legacy`](Self::allow_legacy) flags do not apply to an
    /// explicit list.
    pub fn names(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.names = Some(names.into_iter().map(Into::into).collect());
        self.spec = OnceLock::new();
        self
    }

    /// Whether the default set includes link names such as
    /// `Asia/Calcutta` (on by default).
    pub fn allow_links(mut self, allow: bool) -> Self {
        self.allow_links = allow;
        self.spec = OnceLock::new();
        self
    }

    /// Whether the default set includes legacy names such as `EST5EDT`
    /// and `US/Eastern` (off by default).
    pub fn allow_legacy(mut self, allow: bool) -> Self {
        self.allow_legacy = allow;
        self.spec = OnceLock::new();
        self
    }

    /// Whether `Etc/Unknown`, the zone with a known instant and an unknown
    /// offset, is drawn (off by default).
    pub fn allow_unknown(mut self, allow: bool) -> Self {
        self.allow_unknown = allow;
        self.spec = OnceLock::new();
        self
    }

    /// Whether a name in an explicit [`names`](Self::names) list that the
    /// zone source lacks is accepted as an opaque zone (off by
    /// default): one only the caller's database knows, drawn like any
    /// other name but evaluated like `Etc/Unknown`, with its civil time
    /// read as UTC and an unknown offset.
    pub fn allow_opaque(mut self, allow: bool) -> Self {
        self.allow_opaque = allow;
        self.spec = OnceLock::new();
        self
    }

    fn spec(&self) -> &ffi::TimeZoneSpec {
        self.spec.get_or_init(|| {
            let mut spec = ffi::TimeZoneSpec::new();
            configured(spec.set_source(self.source.handle()));
            if let Some(names) = &self.names {
                configured(spec.set_names(names));
            }
            configured(spec.set_links(self.allow_links));
            configured(spec.set_legacy(self.allow_legacy));
            configured(spec.set_unknown(self.allow_unknown));
            configured(spec.set_opaque(self.allow_opaque));
            configured(spec.validate());
            spec
        })
    }

    /// Draw the name of a zone, without asking the source for its primary
    /// name.
    pub(crate) fn draw_name(&self, tc: &TestCase) -> String {
        tc.generate_timezone(self.spec())
    }
}

/// The zone `name` of `source`, or of no source when it is an opaque
/// zone.
fn time_zone_named(source: &ZoneSource, name: String) -> TimeZone {
    let primary_name = source
        .info(&name)
        .ok()
        .flatten()
        .map_or_else(|| name.clone(), |info| info.primary_name);
    TimeZone { name, primary_name }
}

impl Generator<TimeZone> for ZoneGenerator {
    fn label(&self) -> u64 {
        *TIME_ZONE_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> TimeZone {
        time_zone_named(&self.source, self.draw_name(tc))
    }
}

impl PrintableGenerator<TimeZone> for ZoneGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> TimeZone {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`TimeZone`] values.
///
/// By default any name of the default zone source, link names in and
/// legacy names out; shrinks down a ladder of names to `UTC` when it is
/// allowed, otherwise to the lowest-ranked allowed name. Half the draws
/// are uniform over the allowed names and half weighted by the zones'
/// features, so a zone with a rare feature (a fractional offset, a
/// skipped day, negative DST) is drawn at a rate a short run reaches.
///
/// The default zone source is the engine's bundled IANA database unless
/// [`set_default_zone_source`](super::set_default_zone_source) replaced
/// it. It is read here, when the generator is built: a generator keeps the
/// source it read when the default changes later.
/// [`source`](ZoneGenerator::source) draws against another. See
/// [`ZoneGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zone = tc.draw(gs::temporal::zones().names(["Europe/London", "Asia/Calcutta"]));
///     assert!(zone.primary_name == "Europe/London" || zone.primary_name == "Asia/Kolkata");
/// }
/// ```
pub fn zones() -> ZoneGenerator {
    ZoneGenerator {
        source: default_zone_source(),
        names: None,
        allow_links: true,
        allow_legacy: false,
        allow_unknown: false,
        allow_opaque: false,
        spec: OnceLock::new(),
    }
}

/// What a zoned draw given both kinds of bounds reports.
pub(crate) const MIXED_BOUNDS: &str = "Cannot combine the wall-clock bounds min_value and \
     max_value with the instant bounds min_instant and max_instant";

/// The bounds of a zoned draw: on the wall clock, or on the instant.
pub(crate) enum ZonedBounds {
    Civil(DateTime, DateTime),
    Instant(Instant, Instant),
}

trait AnyZones: Generator<TimeZone> + Any + Send + Sync {}

impl<G> AnyZones for G where G: Generator<TimeZone> + Any + Send + Sync {}

/// Generator for [`ZonedDateTime`] values. Created by
/// [`zoned_datetimes()`].
pub struct ZonedDateTimeGenerator {
    zones: Option<Box<dyn AnyZones>>,
    source: Option<ZoneSource>,
    default_source: ZoneSource,
    min_value: Option<DateTime>,
    max_value: Option<DateTime>,
    min_instant: Option<Instant>,
    max_instant: Option<Instant>,
    civil_range: (DateTime, DateTime),
    instant_range: (Instant, Instant),
    precision: Precision,
    allow_imaginary: bool,
    ambiguous: Ambiguous,
    spec: OnceLock<ffi::ZonedSpec>,
    all_zones: OnceLock<ZoneGenerator>,
}

impl ZonedDateTimeGenerator {
    /// Draw the zone from `zones` rather than from every name of the zone
    /// source.
    ///
    /// Given a [`ZoneGenerator`] (what [`zones()`] returns, with whatever
    /// builder methods were applied), the zone and the wall time come
    /// from one engine draw against that generator's
    /// [`source`](ZoneGenerator::source), so the zone shrinks down the
    /// engine's ladder of names and transitions are targeted across the
    /// allowed zones. Giving this generator a [`source`](Self::source) as
    /// well is then a usage error: pass it to [`zones()`].
    ///
    /// Any other generator of [`TimeZone`], such as `gs::sampled_from(...)`,
    /// `gs::just(...)` or a [`ZoneGenerator`] that was boxed, mapped or
    /// filtered, draws the zone first and then a wall time in that zone,
    /// with the same bounds, precision, gap and fold handling and
    /// transition targeting, the zone shrinking as that generator
    /// shrinks. The zone's name is looked up in this generator's
    /// [`source`](Self::source), or in the default zone source it read
    /// when [`zoned_datetimes()`] built it, so a zone drawn from another
    /// source needs that source passed here too. A name the source does
    /// not know is a usage error, never an opaque zone; for those, give
    /// [`zones()`] the names with
    /// [`allow_opaque`](ZoneGenerator::allow_opaque). The value's `zone`
    /// is the drawn [`TimeZone`] itself.
    pub fn zones<G>(mut self, zones: G) -> Self
    where
        G: Generator<TimeZone> + Send + Sync + 'static,
    {
        self.zones = Some(Box::new(zones));
        self
    }

    /// Resolve zone names against `source` rather than the default zone
    /// source the generator read when [`zoned_datetimes()`] built it.
    /// With no [`zones`](Self::zones) generator the zones drawn are every
    /// name of `source`, as `zones().source(source)` draws them; with one
    /// that is not a [`ZoneGenerator`], `source` is where the names it
    /// draws are looked up. A [`ZoneGenerator`] carries its own source, so
    /// combining one with this is a usage error.
    pub fn source(mut self, source: ZoneSource) -> Self {
        self.source = Some(source);
        self.all_zones = OnceLock::new();
        self
    }

    /// Set the minimum wall-clock time (inclusive), in whatever zone is
    /// drawn. Wall-clock bounds cannot be combined with instant bounds.
    pub fn min_value(mut self, min: DateTime) -> Self {
        self.min_value = Some(min);
        self.spec = OnceLock::new();
        self
    }

    /// Set the maximum wall-clock time (inclusive), in whatever zone is
    /// drawn. Wall-clock bounds cannot be combined with instant bounds.
    pub fn max_value(mut self, max: DateTime) -> Self {
        self.max_value = Some(max);
        self.spec = OnceLock::new();
        self
    }

    /// Set the minimum instant (inclusive). Instant bounds cannot be
    /// combined with wall-clock bounds.
    pub fn min_instant(mut self, min: Instant) -> Self {
        self.min_instant = Some(min);
        self.spec = OnceLock::new();
        self
    }

    /// Set the maximum instant (inclusive). Instant bounds cannot be
    /// combined with wall-clock bounds.
    pub fn max_instant(mut self, max: Instant) -> Self {
        self.max_instant = Some(max);
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

    /// Whether a wall time the clocks skipped may be drawn (`true` by
    /// default). Allowed, it is reported with kind
    /// [`TransitionKind::Gap`](super::TransitionKind::Gap), both offsets and the
    /// instant it resolves to with the pre-transition offset; disallowed,
    /// another wall time is drawn, and a replayed or shrunk value that
    /// lands in a gap is moved past it.
    pub fn allow_imaginary(mut self, allow: bool) -> Self {
        self.allow_imaginary = allow;
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
            configured(match self.bounds() {
                ZonedBounds::Civil(min, max) => {
                    spec.set_civil_bounds(datetime_to_c(min), datetime_to_c(max))
                }
                ZonedBounds::Instant(min, max) => {
                    spec.set_instant_bounds(timestamp_to_c(min), timestamp_to_c(max))
                }
            });
            configured(spec.set_imaginary(imaginary_to_c(self.allow_imaginary)));
            configured(spec.set_ambiguous(ambiguous_to_c(self.ambiguous)));
            configured(spec.validate());
            spec
        })
    }

    /// The bounds of the draw, a side no bound was given for taking the
    /// end of the generator's range, or a usage error when the bounds
    /// given do not fit together.
    pub(crate) fn bounds(&self) -> ZonedBounds {
        let civil = self.min_value.is_some() || self.max_value.is_some();
        let instant = self.min_instant.is_some() || self.max_instant.is_some();
        if civil && instant {
            invalid_argument!("{MIXED_BOUNDS}");
        }
        if !instant {
            return ZonedBounds::Civil(
                self.min_value.unwrap_or(self.civil_range.0),
                self.max_value.unwrap_or(self.civil_range.1),
            );
        }
        let min = self.min_instant.unwrap_or(self.instant_range.0);
        let max = self.max_instant.unwrap_or(self.instant_range.1);
        if min > max {
            invalid_argument!("Cannot have max_instant < min_instant");
        }
        ZonedBounds::Instant(min, max)
    }

    /// The source zone names are resolved against when the zones do not
    /// come from a [`ZoneGenerator`], which has its own.
    fn zone_source(&self) -> &ZoneSource {
        self.source.as_ref().unwrap_or(&self.default_source)
    }

    /// Draw the zone and the wall time in one engine draw.
    fn draw_with(&self, tc: &TestCase, zones: &ZoneGenerator) -> ZonedDateTime {
        let (z, name) = tc.generate_zoned_datetime(zones.spec(), self.spec());
        zoned_from_c(z, time_zone_named(&zones.source, name))
    }

    /// Draw a wall time in `zone`, a zone of [`zone_source`](Self::zone_source)
    /// chosen some other way than by the engine. `Err` carries the
    /// engine's reason when the source does not know the zone or fails to
    /// load it.
    pub(crate) fn draw_in_zone(
        &self,
        tc: &TestCase,
        zone: TimeZone,
    ) -> Result<ZonedDateTime, String> {
        let source = self.zone_source().handle();
        let z = tc.generate_zoned_datetime_for_zone(source, &zone.name, self.spec())?;
        Ok(zoned_from_c(z, zone))
    }

    fn unresolved_zone(&self, name: &str, message: String) -> ! {
        if !matches!(self.zone_source().info(name), Ok(None)) {
            invalid_argument!("{message}");
        }
        let which = match self.source {
            Some(_) => "the zone source given to source()",
            None => "the default zone source",
        };
        invalid_argument!(
            "zoned_datetimes(): the zones generator drew the time zone {name:?}, which {which} \
             does not know. Pass the zone source that has it to source()."
        )
    }
}

fn zoned_from_c(z: hegel_c::hegel_zoned_datetime_t, zone: TimeZone) -> ZonedDateTime {
    ZonedDateTime {
        zone,
        civil: datetime_from_c(z.civil),
        kind: kind_from_c(z.kind),
        offset_before: offset_from_c(z.offset_before, z.offset_known),
        offset_after: offset_from_c(z.offset_after, z.offset_known),
        fold: z.fold,
        offset: offset_from_c(z.offset_seconds, z.offset_known),
        instant: timestamp_from_c(z.timestamp),
    }
}

impl Generator<ZonedDateTime> for ZonedDateTimeGenerator {
    fn label(&self) -> u64 {
        *ZONED_DATETIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> ZonedDateTime {
        let Some(zones) = &self.zones else {
            let all_zones = self
                .all_zones
                .get_or_init(|| zones().source(self.zone_source().clone()));
            return self.draw_with(tc, all_zones);
        };
        let any_zones: &dyn Any = &**zones;
        if let Some(engine) = any_zones.downcast_ref::<ZoneGenerator>() {
            if self.source.is_some() {
                invalid_argument!(
                    "zoned_datetimes(): source() cannot be combined with a zones() generator, \
                     which has a zone source of its own. Pass the source to zones().source() \
                     instead."
                );
            }
            return self.draw_with(tc, engine);
        }
        let zone = tc.draw_silent(&**zones);
        let name = zone.name.clone();
        self.draw_in_zone(tc, zone)
            .unwrap_or_else(|message| self.unresolved_zone(&name, message))
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
/// By default any zone [`zones()`] draws and any wall-clock time of
/// the engine's range at nanosecond precision, with skipped wall times
/// reported as [`TransitionKind::Gap`](super::TransitionKind::Gap) and both repeats
/// of a fold drawn; shrinks down the zone ladder, then toward
/// 2000-01-01T00:00:00, preferring a unique wall time to a repeated one
/// (so a shrink whose lower bound lies in a fold ends just after the
/// fold). One draw in twenty on average targets a transition
/// of the zone, which is how gaps, folds and the exact boundary instants
/// are reached. Build a native value from `civil`, the zone name and
/// `fold` (or, for a gap, from `instant`), never from the offsets, so
/// that a database on your side that differs from the zone source
/// changes what a value means but never makes it invalid; draw against
/// your library's database with [`source`](ZonedDateTimeGenerator::source)
/// to avoid the difference.
///
/// Zones are drawn from, and wall times classified against, the default
/// zone source, the engine's bundled IANA database unless
/// [`set_default_zone_source`](super::set_default_zone_source) replaced
/// it. The default is read here, when the generator is built, and the
/// generator keeps it when the default changes later.
/// [`source`](ZonedDateTimeGenerator::source) names another, and
/// [`zones`](ZonedDateTimeGenerator::zones) takes any generator of
/// [`TimeZone`] to choose the zones; see it for how each kind of generator
/// is drawn and which source its names are looked up in. See
/// [`ZonedDateTimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::generators::temporal::TransitionKind;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let z = tc.draw(gs::temporal::zoned_datetimes().allow_imaginary(false));
///     assert_ne!(z.kind, TransitionKind::Gap);
/// }
/// ```
pub fn zoned_datetimes() -> ZonedDateTimeGenerator {
    zoned_datetimes_within((MIN_DATETIME, MAX_DATETIME), (MIN_TIMESTAMP, MAX_TIMESTAMP))
}

/// [`zoned_datetimes()`] over a narrower range, for a generator that
/// builds another library's value from the draw: the bounds a draw has on
/// a side no bound was given for.
pub(crate) fn zoned_datetimes_within(
    civil_range: (DateTime, DateTime),
    instant_range: (Instant, Instant),
) -> ZonedDateTimeGenerator {
    ZonedDateTimeGenerator {
        zones: None,
        source: None,
        default_source: default_zone_source(),
        min_value: None,
        max_value: None,
        min_instant: None,
        max_instant: None,
        civil_range,
        instant_range,
        precision: Precision::Nanoseconds,
        allow_imaginary: true,
        ambiguous: Ambiguous::Both,
        spec: OnceLock::new(),
        all_zones: OnceLock::new(),
    }
}

/// Generator for [`SignedDuration`] values. Created by
/// [`durations()`].
pub struct DurationGenerator {
    min_value: SignedDuration,
    max_value: SignedDuration,
    precision: Precision,
}

impl DurationGenerator {
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

impl Generator<SignedDuration> for DurationGenerator {
    fn label(&self) -> u64 {
        *SIGNED_DURATION_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> SignedDuration {
        duration_from_c(tc.generate_duration(
            duration_to_c(self.min_value),
            duration_to_c(self.max_value),
            precision_to_c(self.precision),
        ))
    }
}

impl PrintableGenerator<SignedDuration> for DurationGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> SignedDuration {
        draw_and_print_display(self, tc, printer)
    }
}

/// Generate [`SignedDuration`] values.
///
/// By default any duration of ±`i64` seconds at nanosecond precision;
/// shrinks toward zero. One draw in twenty on average targets a bound or
/// a duration such as one unit, 24 hours or the `i32` seconds limit. See
/// [`DurationGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use hegel::generators::temporal::SignedDuration;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let zero = SignedDuration::new(0, 0);
///     let d = tc.draw(gs::temporal::durations().min_value(zero));
///     assert!(d >= zero);
/// }
/// ```
pub fn durations() -> DurationGenerator {
    DurationGenerator {
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

const SPAN_UNITS: usize = SpanUnit::ALL.len();

/// Generator for [`Span`] values. Created by [`spans()`].
pub struct SpanGenerator {
    bounds: Option<[Option<(i64, i64)>; SPAN_UNITS]>,
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
        let bounds = self.bounds.get_or_insert([None; SPAN_UNITS]);
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
            let bounds = self.bounds.unwrap_or([Some((min, i64::MAX)); SPAN_UNITS]);
            for (unit, bound) in SpanUnit::ALL.iter().zip(bounds) {
                if let Some((min, max)) = bound {
                    configured(spec.set_unit_bounds(span_unit_to_c(*unit), min, max));
                }
            }
            configured(spec.validate());
            spec
        })
    }
}

impl Generator<Span> for SpanGenerator {
    fn label(&self) -> u64 {
        *SPAN_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Span {
        span_from_c(tc.generate_span(self.spec()))
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
/// use hegel::generators::temporal::SpanUnit;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let s = tc.draw(gs::temporal::spans().unit_bounds(SpanUnit::Days, 0, 30));
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
