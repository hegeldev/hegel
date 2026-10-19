//! Plain date and time values, as the engine draws them.
//!
//! Every type here is a public struct or enum of public fields mirroring
//! the engine's value model one to one, with no arithmetic, parsing or
//! conversions: a test that uses a date library wraps the value it draws
//! with [`map`](crate::generators::Generator::map) and nothing here
//! competes with that library. The generators over these types are
//! [`dates`](crate::generators::dates), [`times`](crate::generators::times),
//! [`datetimes`](crate::generators::datetimes),
//! [`timestamps`](crate::generators::timestamps),
//! [`utc_offsets`](crate::generators::utc_offsets),
//! [`time_zones`](crate::generators::time_zones),
//! [`zoned_datetimes`](crate::generators::zoned_datetimes),
//! [`signed_durations`](crate::generators::signed_durations) and
//! [`spans`](crate::generators::spans); the option enums of their builders
//! live here too.
//!
//! Each `Display` implementation prints the RFC 9557 form the engine
//! prints a counterexample in, so a failing value reads the same from
//! every language binding: `2024-03-10T01:30:00-05:00[America/New_York]`,
//! with `[gap]` or `[fold=0]`/`[fold=1]` appended to a skipped or repeated
//! wall time (`2024-03-10T02:30:00-04:00[America/New_York][gap]`),
//! `-00:00` for an unknown offset and `Z[Etc/Unknown]` for the unknown
//! zone.
//!
//! The functions [`tzdb_version`], [`resolve`], [`offset_at`] and [`info`]
//! query the engine's bundled time zone database directly, outside any
//! test.

use std::fmt;

use crate::ffi;

/// The lowest year of a [`Date`].
pub const MIN_YEAR: i32 = -999_999;
/// The highest year of a [`Date`].
pub const MAX_YEAR: i32 = 999_999;
/// The first second of [`MIN_YEAR`], the lowest `second` of a
/// [`Timestamp`].
pub const MIN_INSTANT: i64 = -31_619_087_596_800;
/// The last second of [`MAX_YEAR`], the highest `second` of a
/// [`Timestamp`].
pub const MAX_INSTANT: i64 = 31_494_784_780_799;
/// The largest magnitude of a known [`UtcOffset`] in seconds, 25:59:59.
pub const MAX_OFFSET: i32 = 93_599;
/// The name of the unknown time zone, whose instants are known and whose
/// offset is not.
pub const UNKNOWN_ZONE: &str = "Etc/Unknown";

const NANOS_PER_SECOND: u32 = 1_000_000_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// A value outside the range of the type it was meant for, or a time zone
/// name the bundled database does not know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Date {
    /// A date, or an error when the fields are out of range.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Date, Error> {
        let date = Date { year, month, day };
        if !(MIN_YEAR..=MAX_YEAR).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
        {
            return Err(error(format!("{date} is not a valid date")));
        }
        Ok(date)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if (0..=9999).contains(&self.year) {
            write!(f, "{:04}", self.year)?;
        } else {
            write!(f, "{:+07}", self.year)?;
        }
        write!(f, "-{:02}-{:02}", self.month, self.day)
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
    /// A time of day, or an error when the fields are out of range.
    pub fn new(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Result<Time, Error> {
        let time = Time {
            hour,
            minute,
            second,
            nanosecond,
        };
        if hour > 23 || minute > 59 || second > 59 || nanosecond >= NANOS_PER_SECOND {
            return Err(error(format!("{time} is not a valid time of day")));
        }
        Ok(time)
    }
}

fn write_fraction(f: &mut fmt::Formatter<'_>, nanosecond: u32) -> fmt::Result {
    if nanosecond != 0 {
        let digits = format!("{nanosecond:09}");
        write!(f, ".{}", digits.trim_end_matches('0'))?;
    }
    Ok(())
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)?;
        write_fraction(f, self.nanosecond)
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
    /// A datetime, or an error when either part is out of range.
    pub fn new(date: Date, time: Time) -> Result<DateTime, Error> {
        let date = Date::new(date.year, date.month, date.day)?;
        let time = Time::new(time.hour, time.minute, time.second, time.nanosecond)?;
        Ok(DateTime { date, time })
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}T{}", self.date, self.time)
    }
}

/// An instant: `second`s since the Unix epoch in
/// `[MIN_INSTANT, MAX_INSTANT]` (the civil range of [`Date`], about
/// ±3.2 × 10¹⁶ seconds) and a `nanosecond` in `[0, 999999999]` counted
/// forward from that second, so -1.5 s is `{-2, 500000000}`. Prints as
/// the UTC wall time with a `Z` suffix: `2000-01-01T00:00:00Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp {
    pub second: i64,
    pub nanosecond: u32,
}

impl Timestamp {
    /// An instant, or an error when the fields are out of range.
    pub fn new(second: i64, nanosecond: u32) -> Result<Timestamp, Error> {
        if !(MIN_INSTANT..=MAX_INSTANT).contains(&second) || nanosecond >= NANOS_PER_SECOND {
            return Err(error(format!(
                "second {second}, nanosecond {nanosecond} is not a valid timestamp (seconds in \
                 [{MIN_INSTANT}, {MAX_INSTANT}], nanoseconds in [0, 999999999])"
            )));
        }
        Ok(Timestamp { second, nanosecond })
    }
}

fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let days = self.second.div_euclid(SECONDS_PER_DAY);
        let second_of_day = self.second.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        let date = Date {
            year: year as i32,
            month,
            day,
        };
        let time = Time {
            hour: (second_of_day / 3600) as u8,
            minute: (second_of_day / 60 % 60) as u8,
            second: (second_of_day % 60) as u8,
            nanosecond: self.nanosecond,
        };
        write!(f, "{date}T{time}Z")
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

impl UtcOffset {
    /// A known offset, or an error when `seconds` is out of range.
    pub fn new(seconds: i32) -> Result<UtcOffset, Error> {
        if seconds.unsigned_abs() > MAX_OFFSET.unsigned_abs() {
            return Err(error(format!(
                "{seconds} s is not a valid UTC offset (within ±{MAX_OFFSET} s, 25:59:59)"
            )));
        }
        Ok(UtcOffset::Known(seconds))
    }
}

impl fmt::Display for UtcOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UtcOffset::Unknown => f.write_str("-00:00"),
            UtcOffset::Known(seconds) => {
                let sign = if *seconds < 0 { '-' } else { '+' };
                let magnitude = seconds.unsigned_abs();
                let (hours, minutes) = (magnitude / 3600, magnitude / 60 % 60);
                write!(f, "{sign}{hours:02}:{minutes:02}")?;
                if magnitude % 60 != 0 {
                    write!(f, ":{:02}", magnitude % 60)?;
                }
                Ok(())
            }
        }
    }
}

/// A time zone of the bundled IANA database, or [`UNKNOWN_ZONE`]: `name`
/// as it was drawn or written, link names such as `Asia/Calcutta`
/// included, and `primary_name` the zone it resolves to (`Asia/Kolkata`),
/// or the name itself for a zone the engine's database lacks and only the
/// caller's knows. Prints as the name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeZone {
    pub name: String,
    pub primary_name: String,
}

impl TimeZone {
    /// The zone `name` names, or an error when the bundled database does
    /// not know it. Names are matched exactly as the database spells them.
    pub fn new(name: &str) -> Result<TimeZone, Error> {
        match info(name) {
            Some(info) => Ok(TimeZone {
                name: name.to_string(),
                primary_name: info.primary_name,
            }),
            None => Err(error(format!("unknown time zone {name:?}"))),
        }
    }
}

impl fmt::Display for TimeZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// How a wall-clock time maps to instants in its zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// Exactly one instant.
    Unique,
    /// None: the clocks skipped it.
    Gap,
    /// Two: the clocks repeated it.
    Fold,
}

/// A wall-clock time in a time zone, resolved to an instant: the
/// [`Kind`] of the wall time, the offsets in force before and after the
/// transition it falls on (both the resolved offset for a unique time),
/// which repeat of a fold it means (`fold`, the later when set), the
/// resolved `offset` (unknown in [`UNKNOWN_ZONE`] and in a zone only the
/// caller's database knows) and the `timestamp`: the unique instant, the
/// one `fold` picks, or for a gap the wall time read with `offset_before`.
/// Prints as `2024-03-10T01:30:00-05:00[America/New_York]`, with `[gap]`
/// (`2024-03-10T02:30:00-04:00[America/New_York][gap]`) or `[fold=0]` /
/// `[fold=1]` appended when relevant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZonedDateTime {
    pub zone: TimeZone,
    pub civil: DateTime,
    pub kind: Kind,
    pub offset_before: i32,
    pub offset_after: i32,
    pub fold: bool,
    pub offset: UtcOffset,
    pub timestamp: Timestamp,
}

impl fmt::Display for ZonedDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.civil)?;
        if self.zone.name == UNKNOWN_ZONE {
            f.write_str("Z")?;
        } else {
            write!(f, "{}", self.offset)?;
        }
        write!(f, "[{}]", self.zone)?;
        match self.kind {
            Kind::Unique => Ok(()),
            Kind::Gap => f.write_str("[gap]"),
            Kind::Fold => write!(f, "[fold={}]", u8::from(self.fold)),
        }
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
    /// A duration, or an error when `nanoseconds` is out of range or of
    /// the opposite sign to `seconds`.
    pub fn new(seconds: i64, nanoseconds: i32) -> Result<SignedDuration, Error> {
        let consistent = seconds == 0 || nanoseconds == 0 || (seconds < 0) == (nanoseconds < 0);
        if nanoseconds.unsigned_abs() >= NANOS_PER_SECOND || !consistent {
            return Err(error(format!(
                "{seconds} s {nanoseconds} ns is not a valid duration (nanoseconds within \
                 ±999999999 and of the sign of seconds)"
            )));
        }
        Ok(SignedDuration {
            seconds,
            nanoseconds,
        })
    }
}

impl fmt::Display for SignedDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.seconds < 0 || self.nanoseconds < 0 {
            f.write_str("-")?;
        }
        f.write_str("PT")?;
        let seconds = self.seconds.unsigned_abs();
        let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
        if hours != 0 {
            write!(f, "{hours}H")?;
        }
        if minutes != 0 {
            write!(f, "{minutes}M")?;
        }
        if seconds != 0 || self.nanoseconds != 0 || (hours == 0 && minutes == 0) {
            write!(f, "{seconds}")?;
            write_fraction(f, self.nanoseconds.unsigned_abs())?;
            f.write_str("S")?;
        }
        Ok(())
    }
}

/// A unit of a [`Span`], largest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    pub const ALL: [SpanUnit; 10] = [
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
/// nonzero unit in jiff's friendly form, `1y 2mo 3w 4d 5h 6m 7s 8ms 9µs
/// 10ns`, with one leading `-` when every unit is negative or zero, a `-`
/// on each negative unit when the signs are mixed, and `0s` for zero.
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

const SPAN_UNIT_SUFFIXES: [&str; 10] = ["y", "mo", "w", "d", "h", "m", "s", "ms", "µs", "ns"];

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let values = SpanUnit::ALL.map(|unit| self.get(unit));
        let uniform_negative = values.iter().any(|v| *v < 0) && values.iter().all(|v| *v <= 0);
        if uniform_negative {
            f.write_str("-")?;
        }
        let mut written = false;
        for (value, suffix) in values.into_iter().zip(SPAN_UNIT_SUFFIXES) {
            if value == 0 {
                continue;
            }
            if written {
                f.write_str(" ")?;
            }
            written = true;
            if uniform_negative {
                write!(f, "{}{suffix}", value.unsigned_abs())?;
            } else {
                write!(f, "{value}{suffix}")?;
            }
        }
        if !written {
            f.write_str("0s")?;
        }
        Ok(())
    }
}

/// The unit the time part of a drawn value is counted in: the drawn
/// value is a whole number of these units and shrinks in them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

/// What a zoned datetime draw does with a wall time the clocks skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Imaginary {
    /// Report it, with kind [`Kind::Gap`], both offsets and the instant
    /// it resolves to with the pre-transition offset.
    Allow,
    /// Draw another wall time; a replayed or shrunk value that lands in a
    /// gap is moved past it.
    Avoid,
}

/// Which instant a zoned datetime draw means by a wall time the clocks
/// repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub const ALL: [Tag; 16] = [
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

/// What the bundled database knows about a time zone name; see [`info`].
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
/// as `"2026e"`. A frontend that resolves zones with its own database can
/// compare the two and warn when they differ: the engine decides what to
/// generate, the other database what the value means.
pub fn tzdb_version() -> String {
    ffi::tzdb_version()
}

/// What the bundled database knows about `name`, matched exactly as the
/// database spells it, or `None` for a name it lacks. [`UNKNOWN_ZONE`] is
/// known, with itself as primary name, [`Tag::Undefined`] and the highest
/// rank.
pub fn info(name: &str) -> Option<ZoneInfo> {
    ffi::timezone_info(name).map(|info| ZoneInfo {
        primary_name: info.primary_name,
        is_link: info.is_link,
        rank: info.rank,
        tags: Tag::ALL
            .into_iter()
            .enumerate()
            .filter(|(bit, _)| info.tags & (1 << bit) != 0)
            .map(|(_, tag)| tag)
            .collect(),
    })
}

pub(crate) fn kind_from_c(kind: ffi::sys::hegel_timezone_kind_t) -> Kind {
    use ffi::sys::hegel_timezone_kind_t as c;
    match kind {
        c::HEGEL_TIMEZONE_KIND_UNIQUE => Kind::Unique,
        c::HEGEL_TIMEZONE_KIND_GAP => Kind::Gap,
        c::HEGEL_TIMEZONE_KIND_FOLD => Kind::Fold,
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

pub(crate) fn timestamp_from_c(t: ffi::sys::hegel_timestamp_t) -> Timestamp {
    Timestamp {
        second: t.second,
        nanosecond: t.nanosecond,
    }
}

pub(crate) fn timestamp_to_c(t: Timestamp) -> ffi::sys::hegel_timestamp_t {
    ffi::sys::hegel_timestamp_t {
        second: t.second,
        nanosecond: t.nanosecond,
    }
}

/// Classify the wall-clock time `civil` in the zone `name` and resolve it
/// to an instant, meaning the earlier instant of a repeated wall time
/// (`fold` false); the later is the wall time read with `offset_after`.
/// An error names a zone the bundled database lacks, a `civil` out of
/// range, or one whose instant in the zone lies outside the range of
/// [`Timestamp`], which a wall time within an offset's width of the
/// civil range's ends can do.
pub fn resolve(name: &str, civil: DateTime) -> Result<ZonedDateTime, Error> {
    let r = ffi::timezone_resolve(name, datetime_to_c(civil)).map_err(error)?;
    let zone = TimeZone::new(name)?;
    Ok(ZonedDateTime {
        zone,
        civil,
        kind: kind_from_c(r.kind),
        offset_before: r.offset_before,
        offset_after: r.offset_after,
        fold: false,
        offset: offset_from_c(r.offset_seconds, r.offset_known),
        timestamp: timestamp_from_c(r.timestamp),
    })
}

/// The offset the zone `name` has in force at `timestamp`. An error names
/// a zone the bundled database lacks or a `timestamp` out of range.
pub fn offset_at(name: &str, timestamp: Timestamp) -> Result<ZoneOffset, Error> {
    let o = ffi::timezone_offset_at(name, timestamp_to_c(timestamp)).map_err(error)?;
    Ok(ZoneOffset {
        offset: offset_from_c(o.seconds, o.known),
        is_dst: o.is_dst,
        abbreviation: o.abbreviation,
    })
}
