//! Zoned datetimes as a draw (`hegel-c/docs/datetime.md` §6.2): a time
//! zone name, then a wall-clock time within the bounds in that zone, then
//! the fold when the wall time is ambiguous, each an ordinary choice.
//!
//! The node sequence is zone · targeted? · [target index · half-width] ·
//! date · time · [fold]. The targeted node exists only when a target lies
//! in the window: a transition of the zone, or one of the instants of
//! Appendix C that every zone targets (the Unix epoch, 2000-01-01, the
//! 32-bit boundaries, the leap-second boundaries and jiff's extremes). It
//! says whether the date and time are drawn within a half-width of a
//! second, an hour or a day of the target's wall time (for a transition,
//! the wall time just after it), which is how gap times, both folds and
//! the exact boundary instants are reached; its rate is a per-case
//! generation parameter ([`DatetimeGenerationParameters`], drawn when the
//! first such node is), recorded under the fixed [`TARGETED_PROBABILITY`]
//! so the node's kind is the same in every execution. The date and time
//! nodes are the ones `generate_datetime` draws over the window, the time
//! counted in units of the precision; a targeted draw samples them
//! uniformly within its window, since the integer sampler's preference
//! for the low end would otherwise reach one side of the transition only.
//! The fold node exists only when the wall time is ambiguous and the
//! `both` policy is in force.
//!
//! Instant bounds become a wall-clock window in the drawn zone: from the
//! earliest wall time an instant in the bounds reads to the latest, which
//! across a fold is the transition's wall time rather than a bound's own
//! reading, and under `allow` at a lower bound among the instants a gap's
//! skipped wall times shift to is the skipped wall time that shifts to
//! the bound, clamped to the civil envelope. When that window holds no
//! value at the precision, or spans at most a day and lies within two
//! days of a transition, the draw falls back to drawing the instant as one
//! integer node: an index into the values in the bounds in instant order
//! ([`Values`]), shrinking toward the one nearest 2000-01-01T00:00:00Z and
//! sampled at the targeting rate at either side of a transition or at an
//! Appendix C instant in the bounds. The instant fixes which repeat of a
//! fold the value means, so no fold node follows. A zone with no value in
//! the bounds concludes the test case invalid, unless no allowed zone has
//! one, which is an argument error. The wall-clock window of civil bounds
//! is likewise narrowed to the wall times whose instants lie in the
//! Timestamp range of §4, and a drawn wall time whose instant falls
//! outside the bounds (a fold on the wrong side of a bound) concludes the
//! test case invalid.
//!
//! Under the `avoid` policy the time node's sampler redraws a wall time
//! that lands in a gap, uniformly and up to [`GAP_RESAMPLES`] times, so
//! fresh generation draws from the non-gap times of the day. A value that
//! still lands in a gap (a replayed or shrunk one, or one on a day the
//! clocks skipped entirely) is moved to the first wall time after the gap,
//! or when that lies past the window to the last wall time before it, as
//! a transform of the drawn value; when neither lies in the window the
//! window lies within the gap and the test case is concluded invalid. A
//! zone the engine cannot evaluate (an opaque name the spec accepted)
//! reads its civil time as UTC with an unknown offset, as `Etc/Unknown`
//! does.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

pub use super::special::Precision;
use super::special::{
    DAY_NS, Date, DateTime, LAST_NS_OF_DAY, MAX_YEAR, MIN_YEAR, NS, Time, align_down, align_up,
    civil_from_days, date_origin, datetime_of_wall, days_from_civil, time_from_ns, validate_date,
    validate_time, wall_ns,
};
use super::timezone::{Entry, TimeZoneSpec, draw_timezone};
use super::{LABEL_TIMEZONE, LABEL_ZONED_DATETIME, spanned};
use crate::control::{hegel_internal_assert, hegel_internal_error, hegel_internal_unwrap};
use crate::native::bignum::{BigInt, ToPrimitive};
use crate::native::core::choices::IntegerChoice;
use crate::native::core::state::{
    biased_integer_sample, uniform_u64_inclusive, weighted_boolean_sample_precise,
};
use crate::native::core::{
    DatetimeGenerationParameters, EngineError, NativeTestCase, Status, TARGETED_PROBABILITY,
};
use crate::native::rng::EngineRng;
use crate::native::timezone::{
    Kind, MAX_INSTANT, MIN_INSTANT, Resolution, TimeZone, Timestamp, UNKNOWN_NAME, ZoneTransition,
    transitions, type_at, validate_timestamp,
};
use crate::tzdata::Zone;

/// What to do with a wall time the clocks skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imaginary {
    /// Report it, with kind `gap`, both offsets and the forward-shifted
    /// instant.
    Allow,
    /// Draw another wall time, or move a replayed one past the gap.
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

/// The first wall-clock time of the civil range of §4.
const MIN_CIVIL: DateTime = DateTime {
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

/// The last wall-clock time of the civil range of §4.
const MAX_CIVIL: DateTime = DateTime {
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

/// The whole civil range of §4 as a wall-clock window.
pub const FULL_CIVIL_BOUNDS: ZonedBounds = ZonedBounds::Civil {
    min: MIN_CIVIL,
    max: MAX_CIVIL,
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
    /// Check the bounds: each valid and in order, and civil bounds holding
    /// a value at the precision. Whether instant bounds hold one depends on
    /// the zone's offset, so the draw settles that (see the module docs).
    pub fn validate(&self) -> Result<(), EngineError> {
        match &self.bounds {
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
                let unit = i128::from(self.precision.unit());
                if align_up(wall_ns(min), unit) > align_down(wall_ns(max), unit) {
                    return Err(EngineError::InvalidArgument(format!(
                        "generate_zoned_datetime: no value at precision {:?} lies in {:?}",
                        self.precision, self.bounds
                    )));
                }
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
            }
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
    /// RFC 9557 (§6.5): `2024-03-10T01:30:00-05:00[America/New_York]`,
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

const DAY: i64 = 86_400;
const HOUR_NS: i128 = 3_600 * NS;

/// 2000-01-01T00:00:00Z, the shrink origin of instants (§10, Q3).
pub const EPOCH_2000: i64 = 946_684_800;

/// The first instant of the Timestamp range of §4.
const MIN_TIMESTAMP: Timestamp = Timestamp {
    second: MIN_INSTANT,
    nanosecond: 0,
};

/// The last instant of the Timestamp range of §4.
const MAX_TIMESTAMP: Timestamp = Timestamp {
    second: MAX_INSTANT,
    nanosecond: 999_999_999,
};

/// How far either side of the shrink origin, clamped into the window, the
/// draw looks for the zone's transitions: two centuries, so that a rule's
/// transitions are listed over a bounded span of years however wide the
/// window.
pub(crate) const TARGET_RANGE: i64 = 200 * 365 * DAY;

/// How many times the time node's sampler redraws a wall time that lands
/// in a gap under the `avoid` policy before the value is moved past the
/// gap instead; a day the clocks skipped entirely has no other wall time.
const GAP_RESAMPLES: usize = 100;

/// How the date and time nodes are sampled.
#[derive(Clone, Copy)]
enum Sampling {
    /// The engine's integer sampler, as `generate_datetime` draws.
    Biased,
    /// Uniform over the range, for a targeted window.
    Uniform,
}

/// The days after which a leap second was inserted, 1972-06-30 to
/// 2016-12-31: the instants at the start of the next day are the
/// leap-second boundaries of Appendix C.
const LEAP_SECOND_BOUNDARIES: [(i32, u8); 27] = [
    (1972, 7),
    (1973, 1),
    (1974, 1),
    (1975, 1),
    (1976, 1),
    (1977, 1),
    (1978, 1),
    (1979, 1),
    (1980, 1),
    (1981, 7),
    (1982, 7),
    (1983, 7),
    (1985, 7),
    (1988, 1),
    (1990, 1),
    (1991, 1),
    (1992, 7),
    (1993, 7),
    (1994, 7),
    (1996, 1),
    (1997, 7),
    (1999, 1),
    (2006, 1),
    (2009, 1),
    (2012, 7),
    (2015, 7),
    (2017, 1),
];

/// jiff's `Timestamp::MIN`, -9999-01-02T01:59:59Z, and `Timestamp::MAX`,
/// 9999-12-30T22:00:00Z.
const JIFF_MIN_INSTANT: i64 = -377_705_023_201;
const JIFF_MAX_INSTANT: i64 = 253_402_207_200;

/// The instants of Appendix C every zone targets: the Unix epoch, the
/// shrink origin, the 32-bit boundaries, JavaScript's 10¹² ms, the
/// leap-second boundaries and jiff's extremes.
pub fn interesting_instants() -> impl Iterator<Item = i64> {
    [
        0,
        EPOCH_2000,
        i64::from(i32::MAX),
        i64::from(i32::MIN),
        1 << 32,
        1_000_000_000,
        JIFF_MIN_INSTANT,
        JIFF_MAX_INSTANT,
    ]
    .into_iter()
    .chain(LEAP_SECOND_BOUNDARIES.iter().map(|&(year, month)| {
        days_from_civil(&Date {
            year,
            month,
            day: 1,
        }) * DAY
    }))
}

fn instant_ns(timestamp: &Timestamp) -> i128 {
    i128::from(timestamp.second) * NS + i128::from(timestamp.nanosecond)
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

    /// The whole civil range, whose ends are aligned at every precision.
    fn envelope() -> Window {
        Window {
            lo: wall_ns(&MIN_CIVIL),
            hi: wall_ns(&MAX_CIVIL),
        }
    }
}

/// A target in the window: its instant, for the shrink order, and the
/// wall time the targeted window is centred on, which for a transition is
/// the one the clocks read just after it.
struct Target {
    instant: i64,
    center: i128,
}

/// The targets in `window`, nearest the shrink origin first: the zone's
/// transitions within [`TARGET_RANGE`] of the origin whose wall-clock
/// region overlaps the window, and the [`interesting_instants`] whose
/// wall time lies in it.
fn targets(zone: TimeZone, window: &Window) -> Result<Vec<Target>, EngineError> {
    let (lo_s, hi_s) = (
        window.lo.div_euclid(NS) as i64,
        window.hi.div_euclid(NS) as i64,
    );
    let center = EPOCH_2000.clamp(lo_s, hi_s);
    let from = (lo_s - 2 * DAY).max(center.saturating_sub(TARGET_RANGE));
    let to = (hi_s + 2 * DAY).min(center.saturating_add(TARGET_RANGE)) + 1;
    let mut list: Vec<Target> = Vec::new();
    if let TimeZone::Named(name) = zone {
        list.extend(transitions(name.zone, from, to).filter_map(|t| {
            let before = i128::from(t.instant + i64::from(t.before.utoff)) * NS;
            let after = i128::from(t.instant + i64::from(t.after.utoff)) * NS;
            let overlaps = before.min(after) <= window.hi && before.max(after) >= window.lo;
            overlaps.then_some(Target {
                instant: t.instant,
                center: after,
            })
        }));
    }
    for instant in interesting_instants() {
        let timestamp = Timestamp {
            second: instant,
            nanosecond: 0,
        };
        let center = i128::from(instant + i64::from(zone.offset_at(timestamp)?.seconds)) * NS;
        if (window.lo..=window.hi).contains(&center) {
            list.push(Target { instant, center });
        }
    }
    list.sort_by_key(|t| (t.instant.abs_diff(EPOCH_2000), t.instant));
    list.dedup_by_key(|t| t.instant);
    Ok(list)
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

/// Whether the wall-clock second `wall` is one the clocks of `zone`
/// skipped.
fn in_gap(zone: &Zone, wall: i64) -> bool {
    transitions(zone, wall - 2 * DAY, wall + 2 * DAY).any(|t| {
        let before = t.instant + i64::from(t.before.utoff);
        let after = t.instant + i64::from(t.after.utoff);
        before <= wall && wall < after
    })
}

/// One integer node in `[min, max]`, the same node `draw_integer_i64`
/// records, sampled as `sampling` says; with `reject` set, fresh
/// generation redraws a rejected value uniformly, up to [`GAP_RESAMPLES`]
/// times in all.
fn draw_in(
    ntc: &mut NativeTestCase,
    min: i64,
    max: i64,
    sampling: Sampling,
    reject: Option<&dyn Fn(i64) -> bool>,
) -> Result<i64, EngineError> {
    if matches!(sampling, Sampling::Biased) && reject.is_none() {
        return ntc.draw_integer_i64(min, max);
    }
    let params = ntc.family().generation_parameters().integer;
    let uniformly = move |rng: &mut EngineRng| {
        min.wrapping_add_unsigned(uniform_u64_inclusive(0, max.abs_diff(min), rng))
    };
    let v = ntc.draw_integer_from(IntegerChoice::small(min, max, 0), move |kind, rng| {
        let mut v = match sampling {
            Sampling::Biased => hegel_internal_unwrap!(
                biased_integer_sample(kind, rng, params)?.to_i64(),
                "draw_in: sampled a value outside [{min}, {max}]"
            ),
            Sampling::Uniform => uniformly(rng),
        };
        if let Some(reject) = reject {
            for _ in 1..GAP_RESAMPLES {
                if !reject(v) {
                    break;
                }
                v = uniformly(rng);
            }
        }
        Ok(BigInt::from(v))
    })?;
    Ok(hegel_internal_unwrap!(
        v.to_i64(),
        "draw_in: validated value does not fit i64"
    ))
}

/// The date and time nodes of `generate_datetime` over `window`, the
/// time counted in `unit`s and, with `avoid` set, redrawn out of that
/// zone's gaps.
fn draw_datetime_in(
    ntc: &mut NativeTestCase,
    window: &Window,
    unit: i128,
    sampling: Sampling,
    avoid: Option<&'static Zone>,
) -> Result<DateTime, EngineError> {
    let (min_days, max_days) = (
        window.lo.div_euclid(DAY_NS) as i64,
        window.hi.div_euclid(DAY_NS) as i64,
    );
    let origin = date_origin(min_days, max_days);
    let date = civil_from_days(
        origin + draw_in(ntc, min_days - origin, max_days - origin, sampling, None)?,
    );
    let (min_days, max_days) = (i128::from(min_days), i128::from(max_days));
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
    let count = ((hi - lo) / unit) as i64;
    let start = day * DAY_NS + lo;
    let in_gap_at = avoid.map(|zone| {
        move |units: i64| {
            in_gap(
                zone,
                (start + i128::from(units) * unit).div_euclid(NS) as i64,
            )
        }
    });
    let reject = in_gap_at.as_ref().map(|f| f as &dyn Fn(i64) -> bool);
    let units = draw_in(ntc, 0, count, sampling, reject)?;
    Ok(DateTime {
        date,
        time: time_from_ns((lo + i128::from(units) * unit) as i64),
    })
}

/// The wall-clock range of the instants in `[min, max]` in `zone`, in
/// wall nanoseconds: from the earliest reading to the latest, which at a
/// bound inside a fold is not the bound's own reading but the
/// transition's (the later repeat begins at the transition's wall time
/// and the earlier one ends a nanosecond before it), for a transition
/// within the bounds and within an offset's width of the bound; with
/// `gaps` set, from the skipped wall time that shifts to the lower bound
/// when that lies among the instants a gap's skipped wall times shift to.
fn wall_range(
    zone: TimeZone,
    min: &Timestamp,
    max: &Timestamp,
    gaps: bool,
) -> Result<(i128, i128), EngineError> {
    let mut lo = instant_ns(min) + i128::from(zone.offset_at(*min)?.seconds) * NS;
    let mut hi = instant_ns(max) + i128::from(zone.offset_at(*max)?.seconds) * NS;
    if let TimeZone::Named(name) = zone {
        let within = |from: i64, to: i64| {
            transitions(name.zone, from.max(min.second + 1), to.min(max.second) + 1)
        };
        for t in within(min.second + 1, min.second + 2 * DAY) {
            lo = lo.min(i128::from(t.instant + i64::from(t.after.utoff)) * NS);
        }
        for t in within(max.second - 2 * DAY, max.second) {
            hi = hi.max(i128::from(t.instant + i64::from(t.before.utoff)) * NS - 1);
        }
        if gaps {
            for t in transitions(name.zone, min.second - 2 * DAY, min.second + 1) {
                let width = t.after.utoff - t.before.utoff;
                if width > 0 && t.instant + i64::from(width) > min.second {
                    lo = lo.min(instant_ns(min) + i128::from(t.before.utoff) * NS);
                }
            }
        }
    }
    Ok((lo, hi))
}

/// The wall-clock window of the instant bounds in `zone`, aligned to the
/// precision and clamped to the civil envelope, or `None` when the draw
/// should fall back to drawing the instant: the window holds no value, or
/// spans at most a day and lies within two days of a transition. `gaps`
/// is passed on to [`wall_range`].
fn wall_window(
    zone: TimeZone,
    min: &Timestamp,
    max: &Timestamp,
    unit: i128,
    gaps: bool,
) -> Result<Option<Window>, EngineError> {
    if let TimeZone::Named(name) = zone {
        let near = max.second - min.second <= DAY
            && transitions(name.zone, min.second - 2 * DAY, max.second + 2 * DAY + 1)
                .next()
                .is_some();
        if near {
            return Ok(None);
        }
    }
    let envelope = Window::envelope();
    let (lo, hi) = wall_range(zone, min, max, gaps)?;
    Ok(Window::aligned(
        lo.max(envelope.lo),
        hi.min(envelope.hi),
        unit,
    ))
}

/// `count` consecutive wall times at the precision from `first` (wall
/// nanoseconds), each read with `offset`; an ambiguous one among them
/// before `fold_end` means the later repeat, and one at or after it the
/// earlier.
struct Run {
    first: i128,
    count: i128,
    offset: i128,
    fold_end: i128,
}

/// The values of a draw within instant bounds in one zone, in instant
/// order: one run per stretch of constant offset within the bounds,
/// trimmed of the repeat the ambiguous policy excludes, and under `allow`
/// one more per gap for the skipped wall times that shift to instants in
/// the bounds, which read with the offset before the gap. Every run lies
/// in the civil envelope.
struct Values {
    runs: Vec<Run>,
    total: i128,
    unit: i128,
}

impl Values {
    fn of(
        zone: TimeZone,
        min: &Timestamp,
        max: &Timestamp,
        spec: &ZonedSpec,
    ) -> Result<Values, EngineError> {
        let mut values = Values {
            runs: Vec::new(),
            total: 0,
            unit: i128::from(spec.precision.unit()),
        };
        let (lo, hi) = (instant_ns(min), instant_ns(max));
        let TimeZone::Named(name) = zone else {
            values.push(lo, hi, 0, i128::MIN);
            return Ok(values);
        };
        let zone = name.zone;
        let fold_width = |t: &ZoneTransition| i128::from(t.before.utoff - t.after.utoff) * NS;
        let (earlier, later) = (
            spec.ambiguous == Ambiguous::Earlier,
            spec.ambiguous == Ambiguous::Later,
        );
        let mut start = lo;
        let mut offset = i128::from(type_at(zone, min.second)?.utoff) * NS;
        let mut fold_end = i128::MIN;
        if let Some(t) = transitions(zone, min.second - 2 * DAY, min.second + 1).last() {
            let at = i128::from(t.instant) * NS;
            let width = fold_width(&t);
            if width > 0 {
                fold_end = at + i128::from(t.before.utoff) * NS;
                if earlier {
                    start = start.max(at + width);
                }
            }
            if width < 0 && spec.imaginary == Imaginary::Allow {
                let before = i128::from(t.before.utoff) * NS;
                values.push(lo, (at - width - 1).min(hi), before, i128::MIN);
            }
        }
        for t in transitions(zone, min.second + 1, max.second + 1) {
            let at = i128::from(t.instant) * NS;
            let width = fold_width(&t);
            let end = if width > 0 && later { at - width } else { at } - 1;
            values.push(start, end, offset, fold_end);
            if width < 0 && spec.imaginary == Imaginary::Allow {
                values.push(at, (at - width - 1).min(hi), offset, i128::MIN);
            }
            start = if width > 0 && earlier { at + width } else { at };
            offset = i128::from(t.after.utoff) * NS;
            fold_end = if width > 0 {
                at + i128::from(t.before.utoff) * NS
            } else {
                i128::MIN
            };
        }
        let mut end = hi;
        if later {
            if let Some(t) = transitions(zone, max.second + 1, max.second + 2 * DAY + 1).next() {
                let width = fold_width(&t);
                if width > 0 {
                    end = end.min(i128::from(t.instant) * NS - width - 1);
                }
            }
        }
        values.push(start, end, offset, fold_end);
        Ok(values)
    }

    /// Add the wall times at the precision of the instants `start..=end`
    /// read with `offset`, those within the civil envelope.
    fn push(&mut self, start: i128, end: i128, offset: i128, fold_end: i128) {
        let envelope = Window::envelope();
        let first = align_up((start + offset).max(envelope.lo), self.unit);
        let last = align_down((end + offset).min(envelope.hi), self.unit);
        if first <= last {
            let count = (last - first) / self.unit + 1;
            self.runs.push(Run {
                first,
                count,
                offset,
                fold_end,
            });
            self.total += count;
        }
    }

    /// The index of the first value whose instant is at or after
    /// `instant`, or the last index when there is none.
    fn index_at(&self, instant: i128) -> i128 {
        let mut base = 0;
        for run in &self.runs {
            let last = run.first + (run.count - 1) * self.unit - run.offset;
            if instant <= last {
                let wall = (instant + run.offset).max(run.first);
                return base + (align_up(wall, self.unit) - run.first) / self.unit;
            }
            base += run.count;
        }
        self.total - 1
    }

    /// The value at `index`: its wall time and whether, if ambiguous, it
    /// means the later repeat.
    fn at(&self, mut index: i128) -> Result<(DateTime, bool), EngineError> {
        for run in &self.runs {
            if index < run.count {
                let wall = run.first + index * self.unit;
                return Ok((datetime_of_wall(wall), wall < run.fold_end));
            }
            index -= run.count;
        }
        hegel_internal_error!("draw_from_instant: no value {index} past the last run")
    }

    /// The indexes the sampler targets: the last value before and the
    /// first after each change of reading, and the value at or after each
    /// of the [`interesting_instants`] within `[lo, hi]`.
    fn targets(&self, lo: i128, hi: i128) -> Vec<i128> {
        let mut list = Vec::new();
        let mut base = 0;
        for run in &self.runs {
            if base > 0 {
                list.extend([base - 1, base]);
            }
            base += run.count;
        }
        for instant in interesting_instants() {
            let ns = i128::from(instant) * NS;
            if (lo..=hi).contains(&ns) {
                list.push(self.index_at(ns));
            }
        }
        list.sort_unstable();
        list.dedup();
        list
    }
}

/// The instant fallback: one integer node indexing the [`Values`] in the
/// bounds, shrinking toward the one nearest the origin and sampled by the
/// engine's integer sampler or, at the targeting rate, at one of
/// [`Values::targets`]. A zone with no value concludes the test case
/// invalid, unless no allowed zone has one, which is an argument error.
fn draw_from_instant(
    ntc: &mut NativeTestCase,
    tz: &TimeZoneSpec,
    entry: &Entry,
    zone: TimeZone,
    spec: &ZonedSpec,
    min: &Timestamp,
    max: &Timestamp,
) -> Result<ZonedDateTime, EngineError> {
    let values = Values::of(zone, min, max, spec)?;
    if values.total == 0 {
        for other in tz.constraint()?.ladder() {
            let zone = other.zone.unwrap_or(TimeZone::Unknown);
            if Values::of(zone, min, max, spec)?.total > 0 {
                return Err(invalid(ntc));
            }
        }
        return Err(EngineError::InvalidArgument(format!(
            "generate_zoned_datetime: no value at precision {:?} lies in {:?} in any allowed \
             zone",
            spec.precision, spec.bounds
        )));
    }
    let last = hegel_internal_unwrap!(
        i64::try_from(values.total - 1).ok(),
        "draw_from_instant: {} values in {:?}",
        values.total,
        spec.bounds
    );
    let towards = values.index_at(i128::from(EPOCH_2000) * NS) as i64;
    let targets = values.targets(instant_ns(min), instant_ns(max));
    let targeted = if targets.is_empty() {
        0.0
    } else {
        ntc.datetime_parameters()?.targeted_probability
    };
    let integer = ntc.family().generation_parameters().integer;
    let index =
        ntc.draw_integer_from(IntegerChoice::small(0, last, towards), move |kind, rng| {
            if !targets.is_empty() && weighted_boolean_sample_precise(targeted, rng) {
                let i = uniform_u64_inclusive(0, targets.len() as u64 - 1, rng) as usize;
                return Ok(BigInt::from(targets[i]));
            }
            biased_integer_sample(kind, rng, integer)
        })?;
    let index = hegel_internal_unwrap!(
        index.to_i64(),
        "draw_from_instant: validated index does not fit i64"
    );
    let (civil, fold) = values.at(i128::from(index))?;
    let resolution = zone.resolve_any(&civil, fold)?;
    Ok(zoned(
        entry,
        civil,
        fold && resolution.kind == Kind::Fold,
        &resolution,
    ))
}

/// The wall time the gap time `resolution` found moves to under `avoid`:
/// the first after the gap at the precision, or when that lies past
/// `window` the last before it; `None` when neither lies in the window,
/// which lies within the gap.
fn past_gap(
    zone: TimeZone,
    resolution: &Resolution,
    unit: i128,
    window: &Window,
) -> Result<Option<DateTime>, EngineError> {
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
    let start = i128::from(transition.instant + i64::from(transition.before.utoff)) * NS;
    let end = i128::from(transition.instant + i64::from(transition.after.utoff)) * NS;
    let after = align_up(end, unit);
    if after <= window.hi {
        return Ok(Some(datetime_of_wall(after)));
    }
    let before = align_down(start - 1, unit);
    Ok((before >= window.lo).then(|| datetime_of_wall(before)))
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
    let avoid = match (spec.imaginary, zone) {
        (Imaginary::Avoid, TimeZone::Named(name)) => Some(name.zone),
        _ => None,
    };
    let (window, instants) = match &spec.bounds {
        ZonedBounds::Civil { min, max } => {
            let (lo, hi) = wall_range(zone, &MIN_TIMESTAMP, &MAX_TIMESTAMP, false)?;
            let window = Window::aligned(wall_ns(min).max(lo), wall_ns(max).min(hi), unit);
            let instants = (instant_ns(&MIN_TIMESTAMP), instant_ns(&MAX_TIMESTAMP));
            (window, instants)
        }
        ZonedBounds::Instant { min, max } => {
            match wall_window(zone, min, max, unit, spec.imaginary == Imaginary::Allow)? {
                Some(window) => (Some(window), (instant_ns(min), instant_ns(max))),
                None => return draw_from_instant(ntc, tz, entry, zone, spec, min, max),
            }
        }
    };
    let Some(window) = window else {
        return Err(invalid(ntc));
    };
    let mut civil = draw_wall(ntc, zone, &window, unit, avoid)?;
    let mut resolution = zone.resolve_any(&civil, false)?;
    if resolution.kind == Kind::Gap && spec.imaginary == Imaginary::Avoid {
        civil = match past_gap(zone, &resolution, unit, &window)? {
            Some(moved) => moved,
            None => return Err(invalid(ntc)),
        };
        resolution = zone.resolve_any(&civil, false)?;
        hegel_internal_assert!(
            resolution.kind != Kind::Gap,
            "the wall time next to a gap in {} is in a gap",
            entry.name
        );
    }
    let fold = match (resolution.kind, spec.ambiguous) {
        (Kind::Fold, Ambiguous::Both) => ntc.weighted(0.5, None)?,
        (Kind::Fold, Ambiguous::Later) => true,
        _ => false,
    };
    if fold {
        resolution = zone.resolve_any(&civil, true)?;
    }
    let instant = instant_ns(&resolution.timestamp);
    if instant < instants.0 || instant > instants.1 {
        return Err(invalid(ntc));
    }
    Ok(zoned(entry, civil, fold, &resolution))
}

/// The wall time: the targeted node when a target lies in `window`, then
/// when targeted the target index and half-width nodes, then the date and
/// time nodes over the window, or when targeted uniformly over the part
/// of it within the half-width of the target's wall time, or the whole
/// window when no value at the precision lies within the half-width.
fn draw_wall(
    ntc: &mut NativeTestCase,
    zone: TimeZone,
    window: &Window,
    unit: i128,
    avoid: Option<&'static Zone>,
) -> Result<DateTime, EngineError> {
    let list = targets(zone, window)?;
    if list.is_empty() {
        return draw_datetime_in(ntc, window, unit, Sampling::Biased, avoid);
    }
    let DatetimeGenerationParameters {
        targeted_probability,
    } = ntc.datetime_parameters()?;
    if !ntc.weighted_as(TARGETED_PROBABILITY, targeted_probability)? {
        return draw_datetime_in(ntc, window, unit, Sampling::Biased, avoid);
    }
    let center = list[draw_uniform_index(ntc, list.len())?].center;
    let half_width = [NS + unit, HOUR_NS + unit, DAY_NS][draw_uniform_index(ntc, 3)?];
    let narrowed = Window::aligned(
        window.lo.max(center - half_width),
        window.hi.min(center + half_width),
        unit,
    )
    .unwrap_or(*window);
    draw_datetime_in(ntc, &narrowed, unit, Sampling::Uniform, avoid)
}

/// Draw a [`ZonedDateTime`] in a zone allowed by `tz` within `spec`'s
/// bounds, wrapped in a span; see the module docs. Shrinks down the zone
/// ladder, then toward 2000-01-01T00:00 within the bounds, preferring a
/// unique wall time to a repeated one, whose sequence is a node longer.
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
