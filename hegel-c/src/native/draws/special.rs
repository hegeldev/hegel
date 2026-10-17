//! Dates, times and datetimes as draws (`hegel-c/docs/datetime.md` §6.4),
//! with the UUID and IP address draws.
//!
//! A date is one integer node, the day offset from 2000-01-01 clamped into
//! the bounds; a time is one integer node counting units of the precision
//! from the first value in the bounds; a datetime is the date node then
//! the time node, the time's bounds tightened on a boundary date. Each
//! node's sampler mixes the engine's integer sampler with the interesting
//! dates and times of Appendix C that lie in the bounds, at the test
//! case's targeting rate. A date draw with a weekday or month mask snaps
//! the drawn day to the nearest allowed day inside the bounds, toward the
//! shrink origin on a tie, so the masks never reject a test case.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::net::{Ipv4Addr, Ipv6Addr};

use crate::control::{InternalError, hegel_internal_assert, hegel_internal_unwrap};
use crate::native::bignum::{BigInt, ToPrimitive};
use crate::native::core::choices::IntegerChoice;
use crate::native::core::state::{
    biased_integer_sample, uniform_u64_inclusive, weighted_boolean_sample_precise,
};
use crate::native::core::{EngineError, NativeTestCase};
use crate::sys::sync::Lazy;

use super::{LABEL_DATE, LABEL_DATETIME, LABEL_IP_ADDRESS, LABEL_TIME, LABEL_UUID, spanned};

/// A drawn proleptic Gregorian calendar date. `year` ∈
/// [[`MIN_YEAR`], [`MAX_YEAR`]], `month` ∈ [1, 12], `day` ∈
/// [1, days-in-month].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

/// A drawn time of day. `hour` ∈ [0, 23], `minute`/`second` ∈ [0, 59],
/// `nanosecond` ∈ [0, 999_999_999].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
}

/// A drawn naive datetime (a [`Date`] plus a [`Time`], no timezone).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}

/// The unit the time part of a drawn value is counted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl Precision {
    /// The unit in nanoseconds.
    pub const fn unit(self) -> i64 {
        match self {
            Precision::Nanoseconds => 1,
            Precision::Microseconds => 1_000,
            Precision::Milliseconds => 1_000_000,
            Precision::Seconds => 1_000_000_000,
            Precision::Minutes => 60_000_000_000,
            Precision::Hours => 3_600_000_000_000,
        }
    }
}

/// Which weekdays and months a date draw may land on, each a bit set
/// with bit 0 for Monday or January.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateOptions {
    pub weekdays: u8,
    pub months: u16,
}

/// Every weekday, Monday to Sunday.
pub const ALL_WEEKDAYS: u8 = 0x7f;

/// Every month, January to December.
pub const ALL_MONTHS: u16 = 0xfff;

impl Default for DateOptions {
    /// Every weekday and every month.
    fn default() -> DateOptions {
        DateOptions {
            weekdays: ALL_WEEKDAYS,
            months: ALL_MONTHS,
        }
    }
}

/// How far either side of a drawn day the snap looks for an allowed one.
/// Allowed days recur every year (an allowed month has at least 28 days,
/// so every allowed weekday falls in it), at most 366 + 7 days apart, so
/// a window this wide around any day in bounds that hold an allowed day
/// holds one of its own.
const SNAP_RANGE: i64 = 400;

impl DateOptions {
    /// Check that each mask names at least one weekday or month and no
    /// bit beyond them.
    pub fn validate(&self) -> Result<(), EngineError> {
        if self.weekdays == 0 || self.weekdays & !ALL_WEEKDAYS != 0 {
            return Err(EngineError::InvalidArgument(format!(
                "weekdays mask must name at least one weekday in bits 0..=6, got {:#x}",
                self.weekdays
            )));
        }
        if self.months == 0 || self.months & !ALL_MONTHS != 0 {
            return Err(EngineError::InvalidArgument(format!(
                "months mask must name at least one month in bits 0..=11, got {:#x}",
                self.months
            )));
        }
        Ok(())
    }

    fn is_unrestricted(&self) -> bool {
        self.weekdays == ALL_WEEKDAYS && self.months == ALL_MONTHS
    }

    /// Whether the day `days` (from 1970-01-01, a Thursday) is allowed.
    fn allows(&self, days: i64) -> bool {
        let weekday = (days + 3).rem_euclid(7);
        let month = civil_from_days(days).month;
        self.weekdays & (1 << weekday) != 0 && self.months & (1 << (month - 1)) != 0
    }

    /// The nearest allowed day to `days` in `[min_days, max_days]`, which
    /// hold one; on a tie the one toward `origin`.
    fn snap(
        &self,
        days: i64,
        min_days: i64,
        max_days: i64,
        origin: i64,
    ) -> Result<i64, EngineError> {
        let in_bounds = |d: i64| (min_days..=max_days).contains(&d) && self.allows(d);
        Ok(hegel_internal_unwrap!(
            (0..=SNAP_RANGE).find_map(|delta| {
                let candidates = if days < origin {
                    [days + delta, days - delta]
                } else {
                    [days - delta, days + delta]
                };
                candidates.into_iter().find(|&d| in_bounds(d))
            }),
            "no allowed day within {SNAP_RANGE} days of {days} in [{min_days}, {max_days}]"
        ))
    }
}

/// Days in the given month of the Gregorian `year`. Leap years follow the
/// usual rule: divisible by 4 unless divisible by 100 but not 400.
pub(crate) fn days_in_month(year: i32, month: u8) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => unreachable!("days_in_month: month {} out of range 1..=12", month),
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn draw_i64(ntc: &mut NativeTestCase, min: i64, max: i64) -> Result<i64, EngineError> {
    Ok(ntc
        .draw_integer(BigInt::from(min), BigInt::from(max))?
        .to_i64()
        .unwrap())
}

/// Inclusive year envelope accepted by the bounded date draws. Wide enough
/// for every date type the frontends expose (chrono's `NaiveDate` spans
/// years ±262143) while keeping the day arithmetic comfortably inside `i64`.
pub const MIN_YEAR: i32 = -999_999;
pub const MAX_YEAR: i32 = 999_999;

/// Days from 1970-01-01 to `d` in the proleptic Gregorian calendar. Port of
/// Howard Hinnant's `days_from_civil`.
pub(crate) fn days_from_civil(d: &Date) -> i64 {
    let y = i64::from(d.year) - i64::from(d.month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from(d.month) + if d.month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d.day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`]. Port of Hinnant's `civil_from_days`.
pub(crate) fn civil_from_days(z: i64) -> Date {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    Date {
        year: (y + i64::from(month <= 2)) as i32,
        month,
        day,
    }
}

pub(crate) fn validate_date(what: &str, d: &Date) -> Result<(), EngineError> {
    let valid = (MIN_YEAR..=MAX_YEAR).contains(&d.year)
        && (1..=12).contains(&d.month)
        && d.day >= 1
        && i64::from(d.day) <= days_in_month(d.year, d.month);
    if !valid {
        return Err(EngineError::InvalidArgument(format!(
            "{what} is not a valid date (year in [{MIN_YEAR}, {MAX_YEAR}]): \
             {:05}-{:02}-{:02}",
            d.year, d.month, d.day
        )));
    }
    Ok(())
}

pub(crate) fn validate_time(what: &str, t: &Time) -> Result<(), EngineError> {
    let valid = t.hour <= 23 && t.minute <= 59 && t.second <= 59 && t.nanosecond <= 999_999_999;
    if !valid {
        return Err(EngineError::InvalidArgument(format!(
            "{what} is not a valid time: {:02}:{:02}:{:02}.{:09}",
            t.hour, t.minute, t.second, t.nanosecond
        )));
    }
    Ok(())
}

pub(crate) fn time_to_ns(t: &Time) -> i64 {
    ((i64::from(t.hour) * 60 + i64::from(t.minute)) * 60 + i64::from(t.second)) * 1_000_000_000
        + i64::from(t.nanosecond)
}

pub(crate) fn time_from_ns(ns: i64) -> Time {
    Time {
        hour: (ns / 3_600_000_000_000) as u8,
        minute: (ns / 60_000_000_000 % 60) as u8,
        second: (ns / 1_000_000_000 % 60) as u8,
        nanosecond: (ns % 1_000_000_000) as u32,
    }
}

pub(crate) const LAST_NS_OF_DAY: i64 = 86_400_000_000_000 - 1;

pub(crate) const NS: i128 = 1_000_000_000;
pub(crate) const DAY_NS: i128 = 86_400 * NS;

/// Wall-clock nanoseconds since 1970-01-01T00:00 read as UTC.
pub(crate) fn wall_ns(civil: &DateTime) -> i128 {
    i128::from(days_from_civil(&civil.date)) * DAY_NS + i128::from(time_to_ns(&civil.time))
}

pub(crate) fn datetime_of_wall(ns: i128) -> DateTime {
    DateTime {
        date: civil_from_days(ns.div_euclid(DAY_NS) as i64),
        time: time_from_ns(ns.rem_euclid(DAY_NS) as i64),
    }
}

/// The largest multiple of `unit` at most `ns`.
pub(crate) fn align_down(ns: i128, unit: i128) -> i128 {
    ns - ns.rem_euclid(unit)
}

/// The smallest multiple of `unit` at least `ns`.
pub(crate) fn align_up(ns: i128, unit: i128) -> i128 {
    let rem = ns.rem_euclid(unit);
    if rem == 0 { ns } else { ns - rem + unit }
}

/// One integer node in `[min, max]` shrinking toward `towards`, sampled
/// by the engine's integer sampler or, when any group of `targets`
/// (values in the range) is not empty, at the test case's targeting rate
/// from a non-empty group chosen uniformly and then uniformly within it,
/// so that a small group of fixed values is reached as often as a large
/// catalogue.
pub(crate) fn draw_targeted(
    ntc: &mut NativeTestCase,
    min: i128,
    max: i128,
    towards: i128,
    targets: Vec<Vec<i128>>,
) -> Result<i128, EngineError> {
    hegel_internal_assert!(min <= max, "draw_targeted: inverted range [{min}, {max}]");
    let groups: Vec<Vec<i128>> = targets.into_iter().filter(|g| !g.is_empty()).collect();
    let targeted = if groups.is_empty() {
        0.0
    } else {
        ntc.datetime_parameters()?.targeted_probability
    };
    let integer = ntc.family().generation_parameters().integer;
    let v = ntc.draw_integer_from(IntegerChoice::new(min, max, towards), move |kind, rng| {
        if !groups.is_empty() && weighted_boolean_sample_precise(targeted, rng) {
            let group = &groups[uniform_u64_inclusive(0, groups.len() as u64 - 1, rng) as usize];
            let i = uniform_u64_inclusive(0, group.len() as u64 - 1, rng) as usize;
            return Ok(BigInt::from(group[i]));
        }
        biased_integer_sample(kind, rng, integer)
    })?;
    Ok(hegel_internal_unwrap!(
        v.to_i128(),
        "draw_targeted: validated value does not fit i128"
    ))
}

/// The dates of Appendix C every date draw targets when they lie in its
/// bounds: the shrink origin and its year boundaries, the epoch, the
/// 32-bit and 64-bit second and nanosecond boundaries, the end of US
/// local mean time, the Julian-to-Gregorian cutover, the century rules,
/// the library edges, the ISO week 53 days and java.time's UT1 boundary.
const INTERESTING_DATES: [(i32, u8, u8); 36] = [
    (2000, 1, 1),
    (1999, 12, 31),
    (2000, 12, 31),
    (2001, 1, 1),
    (1970, 1, 1),
    (1969, 12, 31),
    (1901, 12, 13),
    (2038, 1, 19),
    (2037, 12, 31),
    (2038, 1, 1),
    (2106, 2, 7),
    (1677, 9, 21),
    (2262, 4, 11),
    (1883, 11, 18),
    (1582, 10, 4),
    (1582, 10, 5),
    (1582, 10, 14),
    (1582, 10, 15),
    (1900, 2, 28),
    (1900, 3, 1),
    (2000, 2, 29),
    (2100, 2, 28),
    (2100, 3, 1),
    (2400, 2, 29),
    (0, 2, 29),
    (1, 1, 1),
    (9999, 12, 31),
    (-9999, 1, 1),
    (275_760, 9, 13),
    (-275_760, 9, 13),
    (2024, 12, 30),
    (2024, 12, 31),
    (2021, 1, 3),
    (2027, 1, 1),
    (1972, 11, 3),
    (2024, 2, 29),
];

/// The first February 29 at or after the day `days` (`forward`), or the
/// last one at or before it.
fn leap_day_near(days: i64, forward: bool) -> i64 {
    let step = if forward { 1 } else { -1 };
    let mut year = civil_from_days(days).year;
    loop {
        if is_leap_year(year) {
            let leap = days_from_civil(&Date {
                year,
                month: 2,
                day: 29,
            });
            let near = if forward { leap >= days } else { leap <= days };
            if near {
                return leap;
            }
        }
        year += step;
    }
}

/// The target days of a date draw over `[min_days, max_days]`: the
/// bounds, the leap days nearest each bound, the year boundaries at each
/// bound and the [`INTERESTING_DATES`] in range, as offsets from `origin`.
fn date_targets(min_days: i64, max_days: i64, origin: i64) -> Vec<i128> {
    let (min, max) = (civil_from_days(min_days), civil_from_days(max_days));
    let year_boundary = |year: i32, month: u8, day: u8| {
        ((MIN_YEAR..=MAX_YEAR).contains(&year)).then(|| days_from_civil(&Date { year, month, day }))
    };
    let mut list: Vec<i64> = [
        Some(min_days),
        Some(max_days),
        Some(leap_day_near(min_days, true)),
        Some(leap_day_near(max_days, false)),
        year_boundary(min.year, 12, 31),
        year_boundary(min.year + 1, 1, 1),
        year_boundary(max.year, 1, 1),
        year_boundary(max.year - 1, 12, 31),
    ]
    .into_iter()
    .flatten()
    .chain(
        INTERESTING_DATES
            .iter()
            .map(|&(year, month, day)| days_from_civil(&Date { year, month, day })),
    )
    .filter(|d| (min_days..=max_days).contains(d))
    .collect();
    list.sort_unstable();
    list.dedup();
    list.into_iter().map(|d| i128::from(d - origin)).collect()
}

/// The times of Appendix C every time draw targets when they lie in its
/// bounds at its precision: midnight and the nanosecond after it, noon,
/// the last second, the hours and half-hours transitions use, Chatham's
/// quarter-hours, St. John's 00:01 and fractional seconds of 1, 3, 6, 7
/// and 9 digits.
const INTERESTING_TIMES: [i64; 20] = [
    0,
    1,
    100_000_000,
    1_000_000,
    1_000,
    100,
    43_200_000_000_000,
    86_399_000_000_000,
    LAST_NS_OF_DAY,
    60_000_000_000,
    3_600_000_000_000,
    5_400_000_000_000,
    7_200_000_000_000,
    9_000_000_000_000,
    10_800_000_000_000,
    12_600_000_000_000,
    9_900_000_000_000,
    13_500_000_000_000,
    86_399_999_000_000,
    86_399_999_999_000,
];

/// The target counts of a time draw over the aligned `[lo, hi]` in
/// `unit`s: the bounds and the [`INTERESTING_TIMES`] in range at the
/// precision.
fn time_targets(lo: i64, hi: i64, unit: i64) -> Vec<i128> {
    let mut list: Vec<i64> = [lo, hi]
        .into_iter()
        .chain(INTERESTING_TIMES)
        .filter(|t| (lo..=hi).contains(t) && t % unit == 0)
        .collect();
    list.sort_unstable();
    list.dedup();
    list.into_iter()
        .map(|t| i128::from((t - lo) / unit))
        .collect()
}

/// The unspanned bounded date draw shared by [`generate_date`] and
/// [`generate_datetime`].
///
/// Mirrors Hypothesis's `DateStrategy`: one integer draw of a day offset
/// within the bounds, centred on 2000-01-01 (clamped into range) so that is
/// the shrink target, sampled with the [`date_targets`] mixed in.
pub(crate) fn draw_date_in(
    ntc: &mut NativeTestCase,
    min_days: i64,
    max_days: i64,
) -> Result<Date, EngineError> {
    let center = date_origin(min_days, max_days);
    let targets = date_targets(min_days, max_days, center);
    let offset = draw_targeted(
        ntc,
        i128::from(min_days - center),
        i128::from(max_days - center),
        0,
        vec![targets],
    )?;
    Ok(civil_from_days(center + offset as i64))
}

/// The day a bounded date draw shrinks toward: 2000-01-01, clamped into
/// `[min_days, max_days]`.
pub(crate) fn date_origin(min_days: i64, max_days: i64) -> i64 {
    days_from_civil(&Date {
        year: 2000,
        month: 1,
        day: 1,
    })
    .clamp(min_days, max_days)
}

/// The unspanned bounded time draw shared by [`generate_time`] and
/// [`generate_datetime`], over `[lo, hi]` in nanoseconds of the day, both
/// multiples of `unit`.
///
/// One integer draw of a count of `unit`s from `lo`, shrinking toward `lo`
/// (the representable time closest to midnight), sampled with the
/// [`time_targets`] mixed in.
fn draw_time_in(
    ntc: &mut NativeTestCase,
    lo: i64,
    hi: i64,
    unit: i64,
) -> Result<Time, EngineError> {
    let targets = time_targets(lo, hi, unit);
    let count = draw_targeted(ntc, 0, i128::from((hi - lo) / unit), 0, vec![targets])?;
    Ok(time_from_ns(lo + count as i64 * unit))
}

/// The values at `precision` in the nanosecond range `[min, max]`, as
/// the first and last, or an error naming `what` when there is none.
fn aligned_range(
    what: &str,
    min: i128,
    max: i128,
    precision: Precision,
) -> Result<(i128, i128), EngineError> {
    let unit = i128::from(precision.unit());
    let (lo, hi) = (align_up(min, unit), align_down(max, unit));
    if lo > hi {
        return Err(EngineError::InvalidArgument(format!(
            "{what}: no value at precision {precision:?} lies in the bounds"
        )));
    }
    Ok((lo, hi))
}

/// Draw a [`Date`] in `[min, max]` on a weekday and in a month `options`
/// allows, wrapped in a span so the shrinker treats it as a unit. Shrinks
/// toward 2000-01-01, or the nearest bound when that is out of range, or
/// the nearest allowed day to either.
pub fn generate_date(
    ntc: &mut NativeTestCase,
    min: Date,
    max: Date,
    options: &DateOptions,
) -> Result<Date, EngineError> {
    validate_date("min_value", &min)?;
    validate_date("max_value", &max)?;
    options.validate()?;
    let (min_days, max_days) = (days_from_civil(&min), days_from_civil(&max));
    if min_days > max_days {
        return Err(EngineError::InvalidArgument(format!(
            "generate_date requires min_value <= max_value, got [{min:?}, {max:?}]"
        )));
    }
    if options.is_unrestricted() {
        return spanned(ntc, LABEL_DATE, |ntc| draw_date_in(ntc, min_days, max_days));
    }
    let probe = max_days.min(min_days + SNAP_RANGE);
    if !(min_days..=probe).any(|d| options.allows(d)) {
        return Err(EngineError::InvalidArgument(format!(
            "generate_date: no date in [{min:?}, {max:?}] falls on an allowed weekday and \
             month ({options:?})"
        )));
    }
    spanned(ntc, LABEL_DATE, |ntc| {
        let drawn = days_from_civil(&draw_date_in(ntc, min_days, max_days)?);
        let origin = date_origin(min_days, max_days);
        Ok(civil_from_days(
            options.snap(drawn, min_days, max_days, origin)?,
        ))
    })
}

/// Draw a [`Time`] in `[min, max]` as a whole number of `precision`
/// units, wrapped in a span. Shrinks toward `min` (the representable time
/// closest to midnight at the precision).
pub fn generate_time(
    ntc: &mut NativeTestCase,
    min: Time,
    max: Time,
    precision: Precision,
) -> Result<Time, EngineError> {
    validate_time("min_value", &min)?;
    validate_time("max_value", &max)?;
    let (min_ns, max_ns) = (time_to_ns(&min), time_to_ns(&max));
    if min_ns > max_ns {
        return Err(EngineError::InvalidArgument(format!(
            "generate_time requires min_value <= max_value, got [{min:?}, {max:?}]"
        )));
    }
    let (lo, hi) = aligned_range(
        "generate_time",
        i128::from(min_ns),
        i128::from(max_ns),
        precision,
    )?;
    spanned(ntc, LABEL_TIME, |ntc| {
        draw_time_in(ntc, lo as i64, hi as i64, precision.unit())
    })
}

/// Draw a [`DateTime`] in `[min, max]` whose time is a whole number of
/// `precision` units, wrapped in a span: a bounded date draw, then a time
/// draw whose bounds tighten to the endpoint times when the drawn date
/// lands on a boundary date.
pub fn generate_datetime(
    ntc: &mut NativeTestCase,
    min: DateTime,
    max: DateTime,
    precision: Precision,
) -> Result<DateTime, EngineError> {
    validate_date("min_value.date", &min.date)?;
    validate_date("max_value.date", &max.date)?;
    validate_time("min_value.time", &min.time)?;
    validate_time("max_value.time", &max.time)?;
    if wall_ns(&min) > wall_ns(&max) {
        return Err(EngineError::InvalidArgument(format!(
            "generate_datetime requires min_value <= max_value, got [{min:?}, {max:?}]"
        )));
    }
    let (lo, hi) = aligned_range("generate_datetime", wall_ns(&min), wall_ns(&max), precision)?;
    spanned(ntc, LABEL_DATETIME, |ntc| {
        draw_datetime_in(ntc, lo, hi, precision.unit())
    })
}

/// The date and time nodes over the wall-clock window `[lo, hi]`, both
/// multiples of `unit`, the time counted in `unit`s.
pub(crate) fn draw_datetime_in(
    ntc: &mut NativeTestCase,
    lo: i128,
    hi: i128,
    unit: i64,
) -> Result<DateTime, EngineError> {
    let (min_days, max_days) = (lo.div_euclid(DAY_NS) as i64, hi.div_euclid(DAY_NS) as i64);
    let date = draw_date_in(ntc, min_days, max_days)?;
    let day = days_from_civil(&date);
    let start = i128::from(day) * DAY_NS;
    let time_lo = if day == min_days {
        (lo - start) as i64
    } else {
        0
    };
    let time_hi = if day == max_days {
        (hi - start) as i64
    } else {
        align_down(i128::from(LAST_NS_OF_DAY), i128::from(unit)) as i64
    };
    let time = draw_time_in(ntc, time_lo, time_hi, unit)?;
    Ok(DateTime { date, time })
}

/// Draw a UUID's 16 big-endian bytes, wrapped in a span. When `version` is
/// set, the RFC 4122 version and variant nibbles are forced accordingly.
///
/// Hypothesis's `uuids()` strategy uses `random.getrandbits(128)` via
/// `use_true_random=True`, so its UUIDs sit outside the choice tree and do
/// not shrink. The native port records two 64-bit integer draws instead —
/// the observable distribution still matches (uniform over 2^128 values when
/// `version` is unset, with the RFC 4122 version + variant nibbles overridden
/// when `version` is set), but native UUIDs nominally shrink toward
/// all-zeros.
///
/// When `version` is unset and the draws land at all-zeros (which the
/// Generate-phase all-simplest pre-trial hits deterministically), the result
/// would be the nil UUID. Hypothesis can't produce nil because `getrandbits`
/// sits outside the choice tree; here we bump the low bit so the
/// "uuids never produce nil" property carries over.
pub fn generate_uuid(
    ntc: &mut NativeTestCase,
    version: Option<u8>,
) -> Result<[u8; 16], EngineError> {
    if let Some(v) = version {
        if v > 15 {
            return Err(EngineError::InvalidArgument(format!(
                "uuid version must be a single hex nibble (0..=15), got {v}"
            )));
        }
    }
    spanned(ntc, LABEL_UUID, |ntc| {
        let hi = ntc
            .draw_integer(BigInt::from(0u64), BigInt::from(u64::MAX))?
            .to_u64()
            .unwrap();
        let lo = ntc
            .draw_integer(BigInt::from(0u64), BigInt::from(u64::MAX))?
            .to_u64()
            .unwrap();
        let mut n: u128 = (u128::from(hi) << 64) | u128::from(lo);

        if let Some(v) = version {
            n &= !(0xc000u128 << 48);
            n |= 0x8000u128 << 48;
            n &= !(0xf000u128 << 64);
            n |= u128::from(v) << 76;
        } else if n == 0 {
            n = 1;
        }

        Ok(n.to_be_bytes())
    })
}

const SPECIAL_IPV4_CIDRS: &[&str] = &[
    "0.0.0.0/8",
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.0.0.0/24",
    "192.0.0.0/29",
    "192.0.0.8/32",
    "192.0.0.9/32",
    "192.0.0.10/32",
    "192.0.0.170/32",
    "192.0.0.171/32",
    "192.0.2.0/24",
    "192.31.196.0/24",
    "192.52.193.0/24",
    "192.88.99.0/24",
    "192.168.0.0/16",
    "192.175.48.0/24",
    "198.18.0.0/15",
    "198.51.100.0/24",
    "203.0.113.0/24",
    "240.0.0.0/4",
    "255.255.255.255/32",
];

const SPECIAL_IPV6_CIDRS: &[&str] = &[
    "::1/128",
    "::/128",
    "::ffff:0:0/96",
    "64:ff9b::/96",
    "64:ff9b:1::/48",
    "100::/64",
    "2001::/23",
    "2001::/32",
    "2001:1::1/128",
    "2001:1::2/128",
    "2001:2::/48",
    "2001:3::/32",
    "2001:4:112::/48",
    "2001:10::/28",
    "2001:20::/28",
    "2001:db8::/32",
    "2002::/16",
    "2620:4f:8000::/48",
    "fc00::/7",
    "fe80::/10",
];

/// A CIDR network as `(base address, size - 1)`. `size - 1` is the inclusive
/// upper bound on the host-part offset, drawn as `draw_integer(0, size_minus_1)`
/// then added to `base`. Storing the offset bound rather than the last address
/// avoids `as i128` reinterpret-casts on values above `i128::MAX` (e.g.
/// `fc00::/7` whose base has the top bit set).
struct V4Network {
    base: u32,
    size_minus_1: u32,
}

struct V6Network {
    base: u128,
    size_minus_1: u128,
}

static SPECIAL_IPV4_NETWORKS: Lazy<Result<Vec<V4Network>, InternalError>> = Lazy::new(|| {
    SPECIAL_IPV4_CIDRS
        .iter()
        .map(|s| parse_v4_cidr(s))
        .collect()
});

static SPECIAL_IPV6_NETWORKS: Lazy<Result<Vec<V6Network>, InternalError>> = Lazy::new(|| {
    SPECIAL_IPV6_CIDRS
        .iter()
        .map(|s| parse_v6_cidr(s))
        .collect()
});

fn parse_v4_cidr(s: &str) -> Result<V4Network, InternalError> {
    let (addr, prefix) = s.split_once('/').unwrap();
    let addr: Ipv4Addr = addr.parse().unwrap();
    let prefix: u32 = prefix.parse().unwrap();
    hegel_internal_assert!(
        (1..=32).contains(&prefix),
        "IPv4 prefix must be in 1..=32, got /{prefix}"
    );
    let mask = u32::MAX << (32 - prefix);
    let base = u32::from(addr) & mask;
    let size_minus_1 = !mask;
    Ok(V4Network { base, size_minus_1 })
}

fn parse_v6_cidr(s: &str) -> Result<V6Network, InternalError> {
    let (addr, prefix) = s.split_once('/').unwrap();
    let addr: Ipv6Addr = addr.parse().unwrap();
    let prefix: u32 = prefix.parse().unwrap();
    hegel_internal_assert!(
        (1..=128).contains(&prefix),
        "IPv6 prefix must be in 1..=128, got /{prefix}"
    );
    let mask = u128::MAX << (128 - prefix);
    let base = u128::from(addr) & mask;
    let size_minus_1 = !mask;
    hegel_internal_assert!(
        size_minus_1 <= i128::MAX as u128,
        "IPv6 special range too wide to fit offset in i128: prefix /{prefix}"
    );
    Ok(V6Network { base, size_minus_1 })
}

/// Draw an IPv4 address, wrapped in a span. Half the draws are uniform over
/// the whole address space and half are biased into the IANA
/// special-purpose ranges, mirroring `st.ip_addresses(v=4)`.
pub fn generate_ipv4(ntc: &mut NativeTestCase) -> Result<Ipv4Addr, EngineError> {
    spanned(ntc, LABEL_IP_ADDRESS, draw_ipv4)
}

/// Draw an IPv6 address, wrapped in a span, with the same special-range
/// biasing as [`generate_ipv4`].
pub fn generate_ipv6(ntc: &mut NativeTestCase) -> Result<Ipv6Addr, EngineError> {
    spanned(ntc, LABEL_IP_ADDRESS, draw_ipv6)
}

fn draw_ipv4(ntc: &mut NativeTestCase) -> Result<Ipv4Addr, EngineError> {
    let addr_int: u32 = if draw_i64(ntc, 0, 1)? == 0 {
        let a = draw_i64(ntc, 0, 255)? as u32;
        let b = draw_i64(ntc, 0, 255)? as u32;
        let c = draw_i64(ntc, 0, 255)? as u32;
        let d = draw_i64(ntc, 0, 255)? as u32;
        (a << 24) | (b << 16) | (c << 8) | d
    } else {
        let nets = SPECIAL_IPV4_NETWORKS.as_ref().map_err(Clone::clone)?;
        let idx = draw_i64(ntc, 0, nets.len() as i64 - 1)? as usize;
        let net = &nets[idx];
        let offset = draw_i64(ntc, 0, i64::from(net.size_minus_1))? as u32;
        net.base + offset
    };
    Ok(Ipv4Addr::from(addr_int))
}

fn draw_ipv6(ntc: &mut NativeTestCase) -> Result<Ipv6Addr, EngineError> {
    let addr_int: u128 = if draw_i64(ntc, 0, 1)? == 0 {
        let hi = ntc
            .draw_integer(BigInt::from(0u64), BigInt::from(u64::MAX))?
            .to_u64()
            .unwrap();
        let lo = ntc
            .draw_integer(BigInt::from(0u64), BigInt::from(u64::MAX))?
            .to_u64()
            .unwrap();
        (u128::from(hi) << 64) | u128::from(lo)
    } else {
        let nets = SPECIAL_IPV6_NETWORKS.as_ref().map_err(Clone::clone)?;
        let idx = draw_i64(ntc, 0, nets.len() as i64 - 1)? as usize;
        let net = &nets[idx];
        let offset = ntc
            .draw_integer(BigInt::from(0u128), BigInt::from(net.size_minus_1))?
            .to_u128()
            .unwrap();
        net.base + offset
    };
    Ok(Ipv6Addr::from(addr_int))
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/draws/special_tests.rs"]
mod tests;
