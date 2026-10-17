//! Zoned datetimes as a draw (`hegel-c/docs/datetime.md` §6.2): a time
//! zone name, then a wall-clock time within the bounds in that zone, then
//! the fold when the wall time is ambiguous, each an ordinary choice.
//!
//! The node sequence is zone · targeted? · (date · time | target index ·
//! half-width · offset) · [fold]. The targeted node exists only when a
//! transition of the zone falls in the window, and says whether the wall
//! time is drawn as an offset from the wall time just after one of them,
//! within a half-width of a second, an hour or a day, which is how gap
//! times, both folds and the exact boundary instants are reached and which
//! shrinks toward the transition itself; its rate is a per-case generation
//! parameter ([`DatetimeGenerationParameters`], drawn when the first such
//! node is), recorded under the fixed [`TARGETED_PROBABILITY`] so the
//! node's kind is the same in every execution. Otherwise the date and time nodes are the ones
//! `generate_datetime` draws, the time counted in units of the precision.
//! The fold node exists only when the wall time is ambiguous and the
//! `both` policy is in force.
//!
//! Instant bounds become a wall-clock window in the drawn zone. When that
//! window inverts, or spans at most a day and lies within two days of a
//! transition, the draw falls back to one integer node: the instant
//! itself, counted in precision units from 2000-01-01T00:00:00Z and
//! converted, with the fold chosen so that the value resolves back to it.
//! A wall time drawn through the window whose instant falls outside the
//! bounds (a fold on the wrong side of a bound) concludes the test case
//! invalid, which can only happen within one transition's width of a
//! bound.
//!
//! A gap time under the `avoid` policy is moved to the first wall time
//! after the gap, as a transform of the drawn value rather than a
//! rejection, so a replayed or shrunk value that lands in a gap is moved
//! the same way; when that moves it past the window's end the test case
//! is concluded invalid. A zone the engine cannot evaluate (an opaque name
//! the spec accepted) reads its civil time as UTC with an unknown offset,
//! as `Etc/Unknown` does.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::special::{
    Date, DateTime, LAST_NS_OF_DAY, MAX_YEAR, MIN_YEAR, Time, civil_from_days, days_from_civil,
    draw_date_in, time_from_ns, time_to_ns, validate_date, validate_time,
};
use super::timezone::{Entry, TimeZoneSpec, draw_timezone};
use super::{LABEL_TIMEZONE, LABEL_ZONED_DATETIME, spanned};
use crate::control::{hegel_internal_assert, hegel_internal_error, hegel_internal_unwrap};
use crate::native::bignum::BigInt;
use crate::native::core::choices::IntegerChoice;
use crate::native::core::state::uniform_u64_inclusive;
use crate::native::core::{
    DatetimeGenerationParameters, EngineError, NativeTestCase, Status, TARGETED_PROBABILITY,
};
use crate::native::timezone::{
    Kind, Resolution, TimeZone, Timestamp, UNKNOWN_NAME, transitions, validate_timestamp,
};

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

/// What to do with a wall time the clocks skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imaginary {
    /// Report it, with kind `gap`, both offsets and the forward-shifted
    /// instant.
    Allow,
    /// Move it to the first wall time after the gap.
    Avoid,
}

/// Which instant a wall time the clocks repeated means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ambiguous {
    /// Either, decided by a fold node.
    Both,
    /// The earlier, with no fold node.
    Earlier,
    /// The later, with no fold node.
    Later,
}

/// The bounds of a zoned datetime draw: on the wall clock of whatever
/// zone is drawn, or on the instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZonedBounds {
    Civil { min: DateTime, max: DateTime },
    Instant { min: Timestamp, max: Timestamp },
}

/// The configuration of a zoned datetime draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZonedSpec {
    pub bounds: ZonedBounds,
    pub precision: Precision,
    pub imaginary: Imaginary,
    pub ambiguous: Ambiguous,
}

/// The whole civil range of §4 as a wall-clock window.
pub const FULL_CIVIL_BOUNDS: ZonedBounds = ZonedBounds::Civil {
    min: DateTime {
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
    },
    max: DateTime {
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
    },
};

impl Default for ZonedSpec {
    /// The whole civil range at nanosecond precision, gap times allowed
    /// and both folds drawn.
    fn default() -> ZonedSpec {
        ZonedSpec {
            bounds: FULL_CIVIL_BOUNDS,
            precision: Precision::Nanoseconds,
            imaginary: Imaginary::Allow,
            ambiguous: Ambiguous::Both,
        }
    }
}

impl ZonedSpec {
    /// Check the bounds: each valid, in order, and holding a value at the
    /// precision.
    pub fn validate(&self) -> Result<(), EngineError> {
        let unit = i128::from(self.precision.unit());
        let (lo, hi) = match &self.bounds {
            ZonedBounds::Civil { min, max } => {
                validate_date("min_value.date", &min.date)?;
                validate_date("max_value.date", &max.date)?;
                validate_time("min_value.time", &min.time)?;
                validate_time("max_value.time", &max.time)?;
                if wall_ns(min) > wall_ns(max) {
                    return Err(EngineError::InvalidArgument(format!(
                        "generate_zoned_datetime requires min_value <= max_value, got \
                         [{min:?}, {max:?}]"
                    )));
                }
                (wall_ns(min), wall_ns(max))
            }
            ZonedBounds::Instant { min, max } => {
                validate_timestamp(min)?;
                validate_timestamp(max)?;
                if min > max {
                    return Err(EngineError::InvalidArgument(format!(
                        "generate_zoned_datetime requires min_value <= max_value, got \
                         [{min:?}, {max:?}]"
                    )));
                }
                (instant_ns(min), instant_ns(max))
            }
        };
        if align_up(lo, unit) > align_down(hi, unit) {
            return Err(EngineError::InvalidArgument(format!(
                "generate_zoned_datetime: no value at precision {:?} lies in {:?}",
                self.precision, self.bounds
            )));
        }
        Ok(())
    }
}

/// A drawn zoned datetime (§4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZonedDateTime {
    /// The zone name as drawn.
    pub zone: String,
    pub civil: DateTime,
    /// The offset in force at `timestamp`; 0 when not `offset_known`.
    pub offset_seconds: i32,
    /// False for `Etc/Unknown` and an opaque zone.
    pub offset_known: bool,
    /// Which instant of a fold the value means; false otherwise.
    pub fold: bool,
    pub kind: Kind,
    /// For a gap or fold, the offsets before and after the transition;
    /// both `offset_seconds` otherwise.
    pub offset_before: i32,
    pub offset_after: i32,
    pub timestamp: Timestamp,
}

impl fmt::Display for ZonedDateTime {
    /// RFC 9557 (§6.5): `2024-03-10T02:30:00-05:00[America/New_York]`,
    /// with `[gap]` or `[fold=N]` appended to a skipped or repeated wall
    /// time, `-00:00` for an unknown offset and `Z[Etc/Unknown]` for the
    /// unknown zone.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (d, t) = (&self.civil.date, &self.civil.time);
        if (0..=9999).contains(&d.year) {
            write!(f, "{:04}", d.year)?;
        } else {
            write!(f, "{:+07}", d.year)?;
        }
        let (month, day, hour, minute, second) = (d.month, d.day, t.hour, t.minute, t.second);
        write!(f, "-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")?;
        if t.nanosecond != 0 {
            let digits = format!("{:09}", t.nanosecond);
            write!(f, ".{}", digits.trim_end_matches('0'))?;
        }
        if self.zone == UNKNOWN_NAME {
            f.write_str("Z")?;
        } else if !self.offset_known {
            f.write_str("-00:00")?;
        } else {
            let sign = if self.offset_seconds < 0 { '-' } else { '+' };
            let magnitude = self.offset_seconds.unsigned_abs();
            let (hours, minutes) = (magnitude / 3600, magnitude / 60 % 60);
            write!(f, "{sign}{hours:02}:{minutes:02}")?;
            if magnitude % 60 != 0 {
                write!(f, ":{:02}", magnitude % 60)?;
            }
        }
        write!(f, "[{}]", self.zone)?;
        match self.kind {
            Kind::Unique => Ok(()),
            Kind::Gap => f.write_str("[gap]"),
            Kind::Fold => write!(f, "[fold={}]", u8::from(self.fold)),
        }
    }
}

const NS: i128 = 1_000_000_000;
const DAY: i64 = 86_400;
const DAY_NS: i128 = 86_400 * NS;
const HOUR_NS: i128 = 3_600 * NS;

/// 2000-01-01T00:00:00Z, the shrink origin of instants (§10, Q3).
pub const EPOCH_2000: i64 = 946_684_800;

/// How far either side of the shrink origin, clamped into the window, the
/// draw looks for the zone's transitions: two centuries, so that a rule's
/// transitions are listed over a bounded span of years however wide the
/// window.
const TARGET_RANGE: i64 = 200 * 365 * DAY;

/// Wall-clock nanoseconds since 1970-01-01T00:00 read as UTC.
fn wall_ns(civil: &DateTime) -> i128 {
    i128::from(days_from_civil(&civil.date)) * DAY_NS + i128::from(time_to_ns(&civil.time))
}

fn datetime_of_wall(ns: i128) -> DateTime {
    DateTime {
        date: civil_from_days(ns.div_euclid(DAY_NS) as i64),
        time: time_from_ns(ns.rem_euclid(DAY_NS) as i64),
    }
}

fn instant_ns(timestamp: &Timestamp) -> i128 {
    i128::from(timestamp.second) * NS + i128::from(timestamp.nanosecond)
}

fn timestamp_of(ns: i128) -> Timestamp {
    Timestamp {
        second: ns.div_euclid(NS) as i64,
        nanosecond: ns.rem_euclid(NS) as u32,
    }
}

fn align_down(ns: i128, unit: i128) -> i128 {
    ns - ns.rem_euclid(unit)
}

fn align_up(ns: i128, unit: i128) -> i128 {
    let rem = ns.rem_euclid(unit);
    if rem == 0 { ns } else { ns - rem + unit }
}

/// A wall-clock window aligned to the precision, in wall nanoseconds.
#[derive(Clone, Copy, Debug)]
struct Window {
    lo: i128,
    hi: i128,
}

impl Window {
    fn aligned(lo: i128, hi: i128, unit: i128) -> Option<Window> {
        let (lo, hi) = (align_up(lo, unit), align_down(hi, unit));
        (lo <= hi).then_some(Window { lo, hi })
    }
}

/// A transition in the window: its instant, for the shrink order, and the
/// wall time the clocks read just after it, which the targeted window is
/// centred on.
struct Target {
    instant: i64,
    center: i128,
}

/// The zone's transitions whose wall-clock region overlaps `window`,
/// within [`TARGET_RANGE`] of the shrink origin.
fn targets(zone: TimeZone, window: &Window) -> impl Iterator<Item = Target> {
    let named = match zone {
        TimeZone::Named(name) => Some(name.zone),
        TimeZone::Unknown => None,
    };
    let (lo_s, hi_s) = (
        window.lo.div_euclid(NS) as i64,
        window.hi.div_euclid(NS) as i64,
    );
    let center = EPOCH_2000.clamp(lo_s, hi_s);
    let from = (lo_s - 2 * DAY).max(center.saturating_sub(TARGET_RANGE));
    let to = (hi_s + 2 * DAY).min(center.saturating_add(TARGET_RANGE)) + 1;
    let (lo, hi) = (window.lo, window.hi);
    named
        .into_iter()
        .flat_map(move |zone| transitions(zone, from, to))
        .filter_map(move |t| {
            let before = i128::from(t.instant + i64::from(t.before.utoff)) * NS;
            let after = i128::from(t.instant + i64::from(t.after.utoff)) * NS;
            let overlaps = before.min(after) <= hi && before.max(after) >= lo;
            overlaps.then_some(Target {
                instant: t.instant,
                center: after,
            })
        })
}

fn invalid(ntc: &mut NativeTestCase) -> EngineError {
    ntc.conclude(Status::Invalid, None);
    EngineError::InvalidTestCase
}

/// One integer node drawn uniformly from `0..count`.
fn draw_uniform_index(ntc: &mut NativeTestCase, count: usize) -> Result<usize, EngineError> {
    let last = count as u64 - 1;
    let v = ntc.draw_integer_from(IntegerChoice::small(0, last as i64, 0), move |_, rng| {
        Ok(BigInt::from(uniform_u64_inclusive(0, last, rng)))
    })?;
    Ok(hegel_internal_unwrap!(
        usize::try_from(v).ok(),
        "draw_uniform_index: validated value does not fit usize"
    ))
}

/// The date and time nodes of `generate_datetime` over `window`, the
/// time counted in `unit`s.
fn draw_datetime_in(
    ntc: &mut NativeTestCase,
    window: &Window,
    unit: i128,
) -> Result<DateTime, EngineError> {
    let (min_days, max_days) = (window.lo.div_euclid(DAY_NS), window.hi.div_euclid(DAY_NS));
    let date = draw_date_in(ntc, min_days as i64, max_days as i64)?;
    let day = i128::from(days_from_civil(&date));
    let lo = if day == min_days {
        window.lo - day * DAY_NS
    } else {
        0
    };
    let hi = if day == max_days {
        window.hi - day * DAY_NS
    } else {
        align_down(i128::from(LAST_NS_OF_DAY), unit)
    };
    let units = ntc.draw_integer_i64(0, ((hi - lo) / unit) as i64)?;
    Ok(DateTime {
        date,
        time: time_from_ns((lo + i128::from(units) * unit) as i64),
    })
}

/// The wall-clock window of the instant bounds in `zone`, or `None` when
/// the draw should fall back to drawing the instant: the window inverts,
/// or spans at most a day and lies within two days of a transition.
fn wall_window(zone: TimeZone, min: &Timestamp, max: &Timestamp, unit: i128) -> Option<Window> {
    let lo = instant_ns(min) + i128::from(zone.offset_at(*min).ok()?.seconds) * NS;
    let hi = instant_ns(max) + i128::from(zone.offset_at(*max).ok()?.seconds) * NS;
    if let TimeZone::Named(name) = zone {
        let near = max.second - min.second <= DAY
            && transitions(name.zone, min.second - 2 * DAY, max.second + 2 * DAY + 1)
                .next()
                .is_some();
        if near {
            return None;
        }
    }
    Window::aligned(lo, hi, unit)
}

/// The instant fallback: one integer node of `unit`s from the shrink
/// origin clamped into `[lo, hi]`, converted to the zone's wall time.
fn draw_from_instant(
    ntc: &mut NativeTestCase,
    entry: &Entry,
    zone: TimeZone,
    lo: i128,
    hi: i128,
    unit: i128,
) -> Result<ZonedDateTime, EngineError> {
    let (lo_u, hi_u) = (lo / unit, hi / unit);
    let center = (i128::from(EPOCH_2000) * NS / unit).clamp(lo_u, hi_u);
    let offset = ntc.draw_integer::<i128>(lo_u - center, hi_u - center)?;
    let timestamp = timestamp_of((center + offset) * unit);
    let offset = zone.offset_at(timestamp)?;
    let civil = datetime_of_wall(instant_ns(&timestamp) + i128::from(offset.seconds) * NS);
    let earlier = zone.resolve(&civil, false)?;
    let fold = earlier.kind == Kind::Fold && earlier.timestamp != timestamp;
    let resolution = if fold {
        zone.resolve(&civil, true)?
    } else {
        earlier
    };
    hegel_internal_assert!(
        resolution.timestamp == timestamp,
        "the wall time of {timestamp:?} in {} resolves to {:?}",
        entry.name,
        resolution.timestamp
    );
    Ok(zoned(entry, civil, fold, &resolution))
}

/// The first wall time after the gap `resolution` found, at the precision.
fn after_gap(zone: TimeZone, resolution: &Resolution, unit: i128) -> Result<DateTime, EngineError> {
    let second = resolution.timestamp.second;
    let zone = match zone {
        TimeZone::Named(name) => name.zone,
        TimeZone::Unknown => hegel_internal_error!("the unknown zone has no gaps"),
    };
    let transition = hegel_internal_unwrap!(
        transitions(zone, second - 2 * DAY, second + 1).last(),
        "no transition of {} precedes the gap at {second}",
        zone.name
    );
    let wall = i128::from(transition.instant + i64::from(transition.after.utoff)) * NS;
    Ok(datetime_of_wall(align_up(wall, unit)))
}

fn zoned(entry: &Entry, civil: DateTime, fold: bool, r: &Resolution) -> ZonedDateTime {
    ZonedDateTime {
        zone: entry.name.clone(),
        civil,
        offset_seconds: r.offset,
        offset_known: r.offset_known,
        fold,
        kind: r.kind,
        offset_before: r.offset_before,
        offset_after: r.offset_after,
        timestamp: r.timestamp,
    }
}

fn draw_zoned(
    ntc: &mut NativeTestCase,
    tz: &TimeZoneSpec,
    spec: &ZonedSpec,
) -> Result<ZonedDateTime, EngineError> {
    let unit = i128::from(spec.precision.unit());
    let entry = spanned(ntc, LABEL_TIMEZONE, |ntc| draw_timezone(ntc, tz))?;
    let zone = entry.zone.unwrap_or(TimeZone::Unknown);
    let (window, instants) = match &spec.bounds {
        ZonedBounds::Civil { min, max } => {
            let window = hegel_internal_unwrap!(
                Window::aligned(wall_ns(min), wall_ns(max), unit),
                "generate_zoned_datetime: validated bounds hold no value"
            );
            (window, None)
        }
        ZonedBounds::Instant { min, max } => {
            let (lo, hi) = (instant_ns(min), instant_ns(max));
            match wall_window(zone, min, max, unit) {
                Some(window) => (window, Some((lo, hi))),
                None => {
                    return draw_from_instant(
                        ntc,
                        entry,
                        zone,
                        align_up(lo, unit),
                        align_down(hi, unit),
                        unit,
                    );
                }
            }
        }
    };
    let mut civil = draw_wall(ntc, zone, &window, unit)?;
    let mut resolution = zone.resolve(&civil, false)?;
    if resolution.kind == Kind::Gap && spec.imaginary == Imaginary::Avoid {
        civil = after_gap(zone, &resolution, unit)?;
        if wall_ns(&civil) > window.hi {
            return Err(invalid(ntc));
        }
        resolution = zone.resolve(&civil, false)?;
        hegel_internal_assert!(
            resolution.kind != Kind::Gap,
            "the first wall time after a gap in {} is in a gap",
            entry.name
        );
    }
    let fold = match (resolution.kind, spec.ambiguous) {
        (Kind::Fold, Ambiguous::Both) => ntc.weighted(0.5, None)?,
        (Kind::Fold, Ambiguous::Later) => true,
        _ => false,
    };
    if fold {
        resolution = zone.resolve(&civil, true)?;
    }
    if let Some((lo, hi)) = instants {
        let instant = instant_ns(&resolution.timestamp);
        if instant < lo || instant > hi {
            return Err(invalid(ntc));
        }
    }
    Ok(zoned(entry, civil, fold, &resolution))
}

/// The wall time: the targeted node when the zone has a transition in
/// `window`, then either the date and time nodes over the window or, when
/// targeted, the target index, half-width and offset nodes, the offset
/// counted in `unit`s from the wall time just after the chosen transition
/// and kept within the window, or within the whole window when no value
/// at the precision lies within the half-width.
fn draw_wall(
    ntc: &mut NativeTestCase,
    zone: TimeZone,
    window: &Window,
    unit: i128,
) -> Result<DateTime, EngineError> {
    let mut candidates = targets(zone, window).peekable();
    if candidates.peek().is_none() {
        return draw_datetime_in(ntc, window, unit);
    }
    let DatetimeGenerationParameters {
        targeted_probability,
    } = ntc.datetime_parameters()?;
    if !ntc.weighted_as(TARGETED_PROBABILITY, targeted_probability)? {
        return draw_datetime_in(ntc, window, unit);
    }
    let mut list: Vec<Target> = candidates.collect();
    list.sort_by_key(|t| (t.instant.abs_diff(EPOCH_2000), t.instant));
    let center = list[draw_uniform_index(ntc, list.len())?].center;
    let half_width = [NS + unit, HOUR_NS + unit, DAY_NS][draw_uniform_index(ntc, 3)?];
    let (lo, hi) = match Window::aligned(
        window.lo.max(center - half_width),
        window.hi.min(center + half_width),
        unit,
    ) {
        Some(narrowed) => (narrowed.lo, narrowed.hi),
        None => (window.lo, window.hi),
    };
    let origin = align_up(center, unit).clamp(lo, hi);
    let offset = ntc.draw_integer::<i128>((lo - origin) / unit, (hi - origin) / unit)?;
    Ok(datetime_of_wall(origin + offset * unit))
}

/// Draw a [`ZonedDateTime`] in a zone allowed by `tz` within `spec`'s
/// bounds, wrapped in a span; see the module docs. Shrinks down the zone
/// ladder, then toward 2000-01-01T00:00 within the bounds.
pub fn generate_zoned_datetime(
    ntc: &mut NativeTestCase,
    tz: &TimeZoneSpec,
    spec: &ZonedSpec,
) -> Result<ZonedDateTime, EngineError> {
    spec.validate()?;
    spanned(ntc, LABEL_ZONED_DATETIME, |ntc| draw_zoned(ntc, tz, spec))
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/draws/zoned_tests.rs"]
mod tests;
