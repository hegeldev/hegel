use std::fmt;

use super::source::default_zone_source;
use crate::ffi;

/// The lowest year of a [`Date`].
pub const MIN_YEAR: i32 = -999_999;
/// The highest year of a [`Date`].
pub const MAX_YEAR: i32 = 999_999;
/// The first second of [`MIN_YEAR`], the lowest `second` of a
/// [`Instant`].
pub const MIN_INSTANT: i64 = -31_619_087_596_800;
/// The last second of [`MAX_YEAR`], the highest `second` of a
/// [`Instant`].
pub const MAX_INSTANT: i64 = 31_494_784_780_799;
/// The largest magnitude of a known [`UtcOffset`] in seconds, 25:59:59.
pub const MAX_OFFSET: i32 = 93_599;
/// The name of the unknown time zone, whose instants are known and whose
/// offset is not.
pub const UNKNOWN_ZONE: &str = "Etc/Unknown";

/// A time zone name a zone source does not know, a wall time or instant
/// a query cannot answer for, or zone data a
/// [`ZoneSource`](super::ZoneSource) rejects or fails to load. A loader or
/// oracle reports its own failures as one, built from a message with
/// `From`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl From<String> for Error {
    fn from(message: String) -> Error {
        Error { message }
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Error {
        Error {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

fn error(message: String) -> Error {
    Error { message }
}

/// A proleptic Gregorian calendar date with `year` in
/// `[MIN_YEAR, MAX_YEAR]` (±999,999), `month` in `[1, 12]` and `day` in
/// `[1, days in the month]`. Prints as `2024-03-10`, with a six-digit
/// signed year outside `0000` to `9999`.
///
/// This and the other value types of this module are plain data: nothing
/// checks the fields when a value is built, and a generator given a bound
/// that is not a real date reports it as a usage error when it draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    /// The date with these fields.
    pub const fn new(year: i32, month: u8, day: u8) -> Date {
        Date { year, month, day }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ffi::format_date(date_to_c(*self)))
    }
}

/// A time of day from `00:00:00` to `23:59:59.999999999`: `hour` in
/// `[0, 23]`, `minute` and `second` in `[0, 59]` (no leap seconds) and
/// `nanosecond` in `[0, 999999999]`. Prints as `02:30:00`, with the
/// fraction appended when it is not zero: `02:30:00.5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
}

impl Time {
    /// The time of day with these fields.
    pub const fn new(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Time {
        Time {
            hour,
            minute,
            second,
            nanosecond,
        }
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ffi::format_time(time_to_c(*self)))
    }
}

/// A wall-clock date and time with no zone or offset, in the range of
/// [`Date`] and [`Time`]. Prints as `2024-03-10T02:30:00`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}

impl DateTime {
    /// The datetime with these parts.
    pub const fn new(date: Date, time: Time) -> DateTime {
        DateTime { date, time }
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ffi::format_datetime(datetime_to_c(*self)))
    }
}

/// An instant: `second`s since the Unix epoch in
/// `[MIN_INSTANT, MAX_INSTANT]` (the civil range of [`Date`], about
/// ±3.2 × 10¹⁶ seconds) and a `nanosecond` in `[0, 999999999]` counted
/// forward from that second, so -1.5 s is `{-2, 500000000}`. Prints as
/// the UTC wall time with a `Z` suffix: `2000-01-01T00:00:00Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Instant {
    pub second: i64,
    pub nanosecond: u32,
}

impl Instant {
    /// The instant with these fields.
    pub const fn new(second: i64, nanosecond: u32) -> Instant {
        Instant { second, nanosecond }
    }
}

impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ffi::format_timestamp(timestamp_to_c(*self)))
    }
}

/// A UTC offset: a known number of seconds east of UTC within ±25:59:59
/// ([`MAX_OFFSET`]), or the unknown offset of RFC 3339's `-00:00`, which
/// the offset and zone draws yield only when asked to. Prints as `+05:30`,
/// `-00:44:30` (the seconds appended when they are not zero) or `-00:00`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UtcOffset {
    Known(i32),
    Unknown,
}

pub(crate) fn offset_to_c(offset: UtcOffset) -> ffi::sys::hegel_offset_t {
    match offset {
        UtcOffset::Known(seconds) => ffi::sys::hegel_offset_t {
            seconds,
            known: true,
        },
        UtcOffset::Unknown => ffi::sys::hegel_offset_t {
            seconds: 0,
            known: false,
        },
    }
}

impl fmt::Display for UtcOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ffi::format_offset(offset_to_c(*self)))
    }
}

/// A time zone of a zone source, or [`UNKNOWN_ZONE`]: `name`
/// as it was drawn or written, link names such as `Asia/Calcutta`
/// included, and `primary_name` the zone it resolves to (`Asia/Kolkata`),
/// or the name itself for a zone the source lacks and only the
/// caller's knows. Prints as the name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeZone {
    pub name: String,
    pub primary_name: String,
}

impl TimeZone {
    /// The zone `name` names in the default zone source
    /// ([`default_zone_source`](super::default_zone_source)), or an error
    /// when that source does not know it or fails to load it. Names are
    /// matched exactly as the source spells them. For a zone of another
    /// source, use [`ZoneSource::zone`](super::ZoneSource::zone).
    pub fn new(name: &str) -> Result<TimeZone, Error> {
        default_zone_source().zone(name)
    }
}

impl fmt::Display for TimeZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// How a wall-clock time maps to instants in its zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TransitionKind {
    /// Exactly one instant.
    Unique,
    /// None: the clocks skipped it.
    Gap,
    /// Two: the clocks repeated it.
    Fold,
}

/// A wall-clock time in a time zone, resolved to an instant.
///
/// Prints as `2024-03-10T01:30:00-05:00[America/New_York]`, with `[gap]`
/// (`2024-03-10T02:30:00-04:00[America/New_York][gap]`) or `[fold=0]` /
/// `[fold=1]` appended to a skipped or repeated wall time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZonedDateTime {
    /// The zone the wall time is in.
    pub zone: TimeZone,
    /// The wall-clock time.
    pub civil: DateTime,
    /// Whether the zone's clocks showed `civil` once, never or twice.
    pub kind: TransitionKind,
    /// For a gap or fold, the offset in force before the transition;
    /// `offset` otherwise.
    pub offset_before: UtcOffset,
    /// For a gap or fold, the offset in force after the transition;
    /// `offset` otherwise.
    pub offset_after: UtcOffset,
    /// Which instant of a repeated wall time is meant, the later when
    /// set; `false` for a unique or skipped wall time.
    pub fold: bool,
    /// The offset in force at `instant`, unknown in [`UNKNOWN_ZONE`] and
    /// in an opaque zone. For a gap this is `offset_after`, since the
    /// instant lies after the transition, so there `civil` minus `offset`
    /// is not `instant`.
    pub offset: UtcOffset,
    /// The instant `civil` resolves to: the unique one, the one `fold`
    /// picks, or for a gap the wall time read with `offset_before`, which
    /// shifts it forward by the gap's length.
    pub instant: Instant,
}

fn kind_to_c(kind: TransitionKind) -> ffi::sys::hegel_timezone_kind_t {
    use ffi::sys::hegel_timezone_kind_t as c;
    match kind {
        TransitionKind::Unique => c::HEGEL_TIMEZONE_KIND_UNIQUE,
        TransitionKind::Gap => c::HEGEL_TIMEZONE_KIND_GAP,
        TransitionKind::Fold => c::HEGEL_TIMEZONE_KIND_FOLD,
    }
}

impl fmt::Display for ZonedDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let offset = offset_to_c(self.offset);
        let value = ffi::sys::hegel_zoned_datetime_t {
            civil: datetime_to_c(self.civil),
            offset_seconds: offset.seconds,
            offset_known: offset.known,
            fold: self.fold,
            kind: kind_to_c(self.kind),
            offset_before: offset_to_c(self.offset_before).seconds,
            offset_after: offset_to_c(self.offset_after).seconds,
            timestamp: timestamp_to_c(self.instant),
        };
        f.write_str(&ffi::format_zoned_datetime(&value, &self.zone.name))
    }
}

/// A signed duration: `seconds` plus `nanoseconds` of the same sign, the
/// latter within ±999999999, so -1.5 s is `{-1, -500000000}`. Prints in
/// the ISO 8601 duration form `PT1H30M5.5S`, with a leading `-` when
/// negative and `PT0S` for zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SignedDuration {
    pub seconds: i64,
    pub nanoseconds: i32,
}

impl SignedDuration {
    /// The duration with these fields.
    pub const fn new(seconds: i64, nanoseconds: i32) -> SignedDuration {
        SignedDuration {
            seconds,
            nanoseconds,
        }
    }
}

pub(crate) fn duration_to_c(d: SignedDuration) -> ffi::sys::hegel_duration_t {
    ffi::sys::hegel_duration_t {
        seconds: d.seconds,
        nanoseconds: d.nanoseconds,
    }
}

pub(crate) fn duration_from_c(d: ffi::sys::hegel_duration_t) -> SignedDuration {
    SignedDuration {
        seconds: d.seconds,
        nanoseconds: d.nanoseconds,
    }
}

impl fmt::Display for SignedDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ffi::format_duration(duration_to_c(*self)))
    }
}

/// A unit of a [`Span`], largest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SpanUnit {
    Years,
    Months,
    Weeks,
    Days,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

impl SpanUnit {
    /// Every unit, largest first.
    pub const ALL: &'static [SpanUnit] = &[
        SpanUnit::Years,
        SpanUnit::Months,
        SpanUnit::Weeks,
        SpanUnit::Days,
        SpanUnit::Hours,
        SpanUnit::Minutes,
        SpanUnit::Seconds,
        SpanUnit::Milliseconds,
        SpanUnit::Microseconds,
        SpanUnit::Nanoseconds,
    ];
}

/// A calendar span: a signed `i64` count of each unit from years to
/// nanoseconds, never normalised (90 minutes is not an hour and a half,
/// and a month is not a number of days). Every unit shares one sign
/// unless the span was drawn with [`SpanSign::Mixed`]. Prints each
/// nonzero unit, largest first: `1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns`, with
/// one leading `-` when no unit is positive (`-3d 4h` is -3 days and -4
/// hours), a sign on every unit when the signs are mixed (`-3d +4h`), and
/// `0s` for zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub years: i64,
    pub months: i64,
    pub weeks: i64,
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub milliseconds: i64,
    pub microseconds: i64,
    pub nanoseconds: i64,
}

impl Span {
    /// The count of `unit` in this span.
    pub fn get(&self, unit: SpanUnit) -> i64 {
        match unit {
            SpanUnit::Years => self.years,
            SpanUnit::Months => self.months,
            SpanUnit::Weeks => self.weeks,
            SpanUnit::Days => self.days,
            SpanUnit::Hours => self.hours,
            SpanUnit::Minutes => self.minutes,
            SpanUnit::Seconds => self.seconds,
            SpanUnit::Milliseconds => self.milliseconds,
            SpanUnit::Microseconds => self.microseconds,
            SpanUnit::Nanoseconds => self.nanoseconds,
        }
    }
}

pub(crate) fn span_to_c(s: &Span) -> ffi::sys::hegel_span_t {
    ffi::sys::hegel_span_t {
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

pub(crate) fn span_from_c(s: ffi::sys::hegel_span_t) -> Span {
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

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ffi::format_span(&span_to_c(self)))
    }
}

/// The unit the time part of a drawn value is counted in: the drawn
/// value is a whole number of these units and shrinks in them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Precision {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

/// The values a UTC offset draw restricts itself to: whole hours, quarter
/// hours, minutes or seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Granularity {
    Hours,
    QuarterHours,
    Minutes,
    Seconds,
}

/// A day of the week, for restricting a date draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// A month of the year, for restricting a date draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Month {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

/// Which instant a zoned datetime draw means by a wall time the clocks
/// repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Ambiguous {
    /// Either, decided by a drawn fold that shrinks to the earlier.
    Both,
    /// The earlier instant, with no fold drawn.
    Earlier,
    /// The later instant, with no fold drawn.
    Later,
}

/// How the signs of a span's units are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpanSign {
    /// One sign for every unit, as a jiff `Span` or Temporal `Duration`
    /// has.
    Uniform,
    /// Each unit signed on its own, as a java `Period` is.
    Mixed,
    /// Magnitudes only; every enabled unit's lower bound must be at
    /// least 0.
    NonNegative,
}

/// A feature of a time zone, derived from its offsets and name. "After
/// 1970" means at any instant from 1970-01-01T00:00:00Z on, the rule in
/// force after the last explicit transition included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Tag {
    /// No transition after 1970.
    Fixed,
    /// Every offset after 1970 is a whole number of hours.
    WholeHour,
    /// Some offset after 1970 is a whole number of minutes but not of
    /// hours (`+05:30`, `+05:45`, `-03:30`).
    Fractional,
    /// Some offset in the zone's history is not a whole number of minutes
    /// (local mean time, Monrovia's `-00:44:30` until 1972).
    Seconds,
    /// The current rule has a DST offset exactly one hour from the
    /// standard one.
    Dst,
    /// The current rule has a DST offset of another size (Lord Howe's 30
    /// minutes, Troll's two hours).
    DstOther,
    /// Some calendar year has more than two offset changes.
    Multi,
    /// The lowest offset in force during some year after 1970 differs
    /// from the previous year's (a permanent change).
    BaseChange,
    /// A transition after 1970 whose wall time before or after it is
    /// midnight.
    Midnight,
    /// A transition after 1970 whose wall times before and after it fall
    /// on different dates.
    SpansMidnight,
    /// A transition that skips or repeats a whole day (Pacific/Apia 2011).
    WholeDay,
    /// A transition after 1970 that changes the abbreviation but not the
    /// offset.
    SameOffset,
    /// The zone uses the `-00` undefined-offset abbreviation.
    Undefined,
    /// The name is a link to another zone.
    Link,
    /// The name is not `Area/Location` for a current area (`EST5EDT`,
    /// `US/Eastern`).
    Legacy,
    /// The name is one of the `Etc/GMT+5` family, whose sign is POSIX's:
    /// `Etc/GMT+5` is five hours behind UTC.
    InvertedSign,
}

impl Tag {
    /// Every tag, in the order of the engine's bit set.
    pub const ALL: &'static [Tag] = &[
        Tag::Fixed,
        Tag::WholeHour,
        Tag::Fractional,
        Tag::Seconds,
        Tag::Dst,
        Tag::DstOther,
        Tag::Multi,
        Tag::BaseChange,
        Tag::Midnight,
        Tag::SpansMidnight,
        Tag::WholeDay,
        Tag::SameOffset,
        Tag::Undefined,
        Tag::Link,
        Tag::Legacy,
        Tag::InvertedSign,
    ];
}

/// What a zone source knows about a time zone name; see [`info`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZoneInfo {
    /// The zone the name resolves to: the name itself unless it is a
    /// link.
    pub primary_name: String,
    /// Whether the name is a link to `primary_name`.
    pub is_link: bool,
    /// The name's position on the shrink ladder; `UTC` is 0.
    pub rank: u16,
    /// The zone's features, in [`Tag::ALL`] order.
    pub tags: Vec<Tag>,
}

/// The offset a time zone has in force at an instant; see [`offset_at`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZoneOffset {
    pub offset: UtcOffset,
    /// The database's daylight-saving flag for the offset, informational
    /// only.
    pub is_dst: bool,
    /// The abbreviation in force, such as `EST`, or `-00` when the offset
    /// is unknown.
    pub abbreviation: String,
}

/// The IANA release of the time zone database bundled in the engine, such
/// as `"2026e"`: the database of [`ZoneSource::bundled`](super::ZoneSource::bundled).
/// A source over another database reports its own release with
/// [`ZoneSource::version`](super::ZoneSource::version).
pub fn tzdb_version() -> String {
    ffi::tzdb_version()
}

/// What the default zone source
/// ([`default_zone_source`](super::default_zone_source)) knows about
/// `name`, matched exactly as the source spells it: `None` for a name it
/// lacks, and an error when loading the name's data fails.
/// [`UNKNOWN_ZONE`] is known, with itself as primary name,
/// [`Tag::Undefined`] and the highest rank. The default is read at each
/// call.
pub fn info(name: &str) -> Result<Option<ZoneInfo>, Error> {
    default_zone_source().info(name)
}

pub(crate) fn info_in(
    source: &ffi::ZoneSourceHandle,
    name: &str,
) -> Result<Option<ZoneInfo>, Error> {
    let info = ffi::timezone_info(Some(source), name).map_err(error)?;
    Ok(info.map(|info| ZoneInfo {
        primary_name: info.primary_name,
        is_link: info.is_link,
        rank: info.rank,
        tags: Tag::ALL
            .iter()
            .enumerate()
            .filter(|(bit, _)| info.tags & (1 << bit) != 0)
            .map(|(_, tag)| *tag)
            .collect(),
    }))
}

/// The zone `name` of `source`: the name with the primary name `source`
/// resolves it to.
pub(crate) fn zone_in(source: &ffi::ZoneSourceHandle, name: &str) -> Result<TimeZone, Error> {
    match info_in(source, name)? {
        Some(info) => Ok(TimeZone {
            name: name.to_string(),
            primary_name: info.primary_name,
        }),
        None => Err(error(format!("unknown time zone {name:?}"))),
    }
}

pub(crate) fn kind_from_c(kind: ffi::sys::hegel_timezone_kind_t) -> TransitionKind {
    use ffi::sys::hegel_timezone_kind_t as c;
    match kind {
        c::HEGEL_TIMEZONE_KIND_UNIQUE => TransitionKind::Unique,
        c::HEGEL_TIMEZONE_KIND_GAP => TransitionKind::Gap,
        c::HEGEL_TIMEZONE_KIND_FOLD => TransitionKind::Fold,
    }
}

pub(crate) fn offset_from_c(seconds: i32, known: bool) -> UtcOffset {
    if known {
        UtcOffset::Known(seconds)
    } else {
        UtcOffset::Unknown
    }
}

pub(crate) fn date_from_c(d: ffi::sys::hegel_date_t) -> Date {
    Date {
        year: d.year,
        month: d.month,
        day: d.day,
    }
}

pub(crate) fn date_to_c(d: Date) -> ffi::sys::hegel_date_t {
    ffi::sys::hegel_date_t {
        year: d.year,
        month: d.month,
        day: d.day,
    }
}

pub(crate) fn time_from_c(t: ffi::sys::hegel_time_t) -> Time {
    Time {
        hour: t.hour,
        minute: t.minute,
        second: t.second,
        nanosecond: t.nanosecond,
    }
}

pub(crate) fn time_to_c(t: Time) -> ffi::sys::hegel_time_t {
    ffi::sys::hegel_time_t {
        hour: t.hour,
        minute: t.minute,
        second: t.second,
        nanosecond: t.nanosecond,
    }
}

pub(crate) fn datetime_from_c(dt: ffi::sys::hegel_datetime_t) -> DateTime {
    DateTime {
        date: date_from_c(dt.date),
        time: time_from_c(dt.time),
    }
}

pub(crate) fn datetime_to_c(dt: DateTime) -> ffi::sys::hegel_datetime_t {
    ffi::sys::hegel_datetime_t {
        date: date_to_c(dt.date),
        time: time_to_c(dt.time),
    }
}

pub(crate) fn timestamp_from_c(t: ffi::sys::hegel_timestamp_t) -> Instant {
    Instant {
        second: t.second,
        nanosecond: t.nanosecond,
    }
}

pub(crate) fn timestamp_to_c(t: Instant) -> ffi::sys::hegel_timestamp_t {
    ffi::sys::hegel_timestamp_t {
        second: t.second,
        nanosecond: t.nanosecond,
    }
}

/// Classify the wall-clock time `civil` in the zone `name` of the default
/// zone source ([`default_zone_source`](super::default_zone_source), read
/// at each call) and resolve it to an instant, meaning the earlier instant
/// of a repeated wall time (`fold` false); the later is the wall time read
/// with `offset_after`. An error names a zone the source lacks or fails to
/// load, a `civil` that is not a real wall time, or one whose instant in
/// the zone lies outside the range of [`Instant`], which a wall time
/// within an offset's width of the civil range's ends can do.
pub fn resolve(name: &str, civil: DateTime) -> Result<ZonedDateTime, Error> {
    default_zone_source().resolve(name, civil)
}

pub(crate) fn resolve_in(
    source: &ffi::ZoneSourceHandle,
    name: &str,
    civil: DateTime,
) -> Result<ZonedDateTime, Error> {
    let r = ffi::timezone_resolve(Some(source), name, datetime_to_c(civil)).map_err(error)?;
    Ok(ZonedDateTime {
        zone: zone_in(source, name)?,
        civil,
        kind: kind_from_c(r.kind),
        offset_before: offset_from_c(r.offset_before, r.offset_known),
        offset_after: offset_from_c(r.offset_after, r.offset_known),
        fold: false,
        offset: offset_from_c(r.offset_seconds, r.offset_known),
        instant: timestamp_from_c(r.timestamp),
    })
}

/// The offset the zone `name` of the default zone source
/// ([`default_zone_source`](super::default_zone_source), read at each
/// call) has in force at `instant`. An error names a zone the source lacks
/// or fails to load, or an `instant` out of range.
pub fn offset_at(name: &str, instant: Instant) -> Result<ZoneOffset, Error> {
    default_zone_source().offset_at(name, instant)
}

pub(crate) fn offset_at_in(
    source: &ffi::ZoneSourceHandle,
    name: &str,
    instant: Instant,
) -> Result<ZoneOffset, Error> {
    let o = ffi::timezone_offset_at(Some(source), name, timestamp_to_c(instant)).map_err(error)?;
    Ok(ZoneOffset {
        offset: offset_from_c(o.seconds, o.known),
        is_dst: o.is_dst,
        abbreviation: o.abbreviation,
    })
}
