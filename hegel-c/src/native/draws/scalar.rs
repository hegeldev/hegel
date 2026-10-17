//! Timestamps, UTC offsets, durations and calendar spans as draws
//! (`hegel-c/docs/datetime.md` §6.3): each a handful of ordinary integer
//! nodes counting units of the draw's precision or granularity, so that
//! every node value is a value of the draw and a frontend at any precision
//! draws the same node shape.
//!
//! A timestamp is one integer node, the count of precision units from
//! 2000-01-01T00:00:00Z clamped into the bounds, whose sampler mixes the
//! engine's integer sampler with the interesting instants of Appendix C
//! and the transitions of every bundled zone. An offset is one integer
//! node of granularity units shrinking toward 0, sampled with the offsets
//! any bundled zone has ever used mixed in, preceded by a weighted boolean
//! node when the unknown offset is allowed, whose `true` is the unknown
//! offset with no seconds node, so a draw shrinks to a known offset before
//! the offset itself shrinks. A duration is one integer node of precision
//! units shrinking toward 0. A span is one integer node per enabled unit,
//! largest first, under a sign policy: `uniform` draws one sign node when
//! the bounds admit both signs and then magnitudes, `mixed` draws each
//! unit signed and `nonnegative` draws magnitudes only; it shrinks to the
//! zero span.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use super::special::{Date, Precision, align_down, align_up, days_from_civil, draw_targeted};
use super::zoned::{EPOCH_2000, interesting_instants};
use super::{LABEL_DURATION, LABEL_OFFSET, LABEL_SPAN, LABEL_TIMESTAMP, spanned};
use crate::control::{InternalError, hegel_internal_unwrap};
use crate::native::core::{EngineError, NativeTestCase};
use crate::native::timezone::{Timestamp, validate_timestamp};
use crate::sys::sync::Lazy;
use crate::tzdata;

const NS: i128 = 1_000_000_000;

/// The instant catalogue every timestamp draw targets, in nanoseconds:
/// the instants every zone targets, the epoch's neighbours, the i64
/// nanosecond boundaries and the leap-day instants of the century rules.
fn interesting_instant_ns() -> impl Iterator<Item = i128> {
    let midnight = |year: i32, month: u8, day: u8| {
        i128::from(days_from_civil(&Date { year, month, day })) * 86_400 * NS
    };
    interesting_instants().map(|s| i128::from(s) * NS).chain([
        -1,
        1,
        i128::from(i64::MIN),
        i128::from(i64::MAX),
        midnight(1900, 2, 28),
        midnight(2000, 2, 29),
        midnight(2024, 2, 29),
        midnight(2100, 2, 28),
        midnight(2400, 2, 29),
    ])
}

/// Every explicit transition instant of every bundled zone, ascending.
static TRANSITION_CATALOGUE: Lazy<Result<Vec<i64>, InternalError>> = Lazy::new(|| {
    let mut set = BTreeSet::new();
    for name in tzdata::names()? {
        if !name.is_link() {
            set.extend(name.zone.transitions.iter().map(|t| t.instant));
        }
    }
    Ok(set.into_iter().collect())
});

/// Draw a [`Timestamp`] in `[min, max]` as a whole number of `precision`
/// units, wrapped in a span. Shrinks toward 2000-01-01T00:00:00Z, or the
/// nearest bound when that is out of range.
pub fn generate_timestamp(
    ntc: &mut NativeTestCase,
    min: Timestamp,
    max: Timestamp,
    precision: Precision,
) -> Result<Timestamp, EngineError> {
    validate_timestamp(&min)?;
    validate_timestamp(&max)?;
    if min > max {
        return Err(EngineError::InvalidArgument(format!(
            "generate_timestamp requires min_value <= max_value, got [{min:?}, {max:?}]"
        )));
    }
    let unit = i128::from(precision.unit());
    let (lo, hi) = (
        align_up(instant_ns(&min), unit),
        align_down(instant_ns(&max), unit),
    );
    if lo > hi {
        return Err(EngineError::InvalidArgument(format!(
            "generate_timestamp: no value at precision {precision:?} lies in [{min:?}, {max:?}]"
        )));
    }
    let origin = (i128::from(EPOCH_2000) * NS).clamp(lo, hi);
    let catalogue = TRANSITION_CATALOGUE.as_ref().map_err(Clone::clone)?;
    let (lo_s, hi_s) = (lo.div_euclid(NS) as i64, hi.div_euclid(NS) as i64);
    let from = catalogue.partition_point(|&t| t < lo_s);
    let to = catalogue.partition_point(|&t| t <= hi_s);
    let count_of = |t: i128| ((lo..=hi).contains(&t) && t % unit == 0).then(|| (t - origin) / unit);
    let mut fixed: Vec<i128> = [lo, hi]
        .into_iter()
        .chain(interesting_instant_ns())
        .filter_map(count_of)
        .collect();
    fixed.sort_unstable();
    fixed.dedup();
    let transitions: Vec<i128> = catalogue[from..to]
        .iter()
        .filter_map(|&t| count_of(i128::from(t) * NS))
        .collect();
    spanned(ntc, LABEL_TIMESTAMP, |ntc| {
        let count = draw_targeted(
            ntc,
            (lo - origin) / unit,
            (hi - origin) / unit,
            0,
            vec![fixed, transitions],
        )?;
        Ok(timestamp_of(origin + count * unit))
    })
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

/// The values an offset draw restricts itself to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granularity {
    Hours,
    QuarterHours,
    Minutes,
    Seconds,
}

impl Granularity {
    /// The unit in seconds.
    pub const fn unit(self) -> i32 {
        match self {
            Granularity::Hours => 3_600,
            Granularity::QuarterHours => 900,
            Granularity::Minutes => 60,
            Granularity::Seconds => 1,
        }
    }
}

/// A drawn UTC offset: `seconds` east of UT, 0 when not `known`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UtcOffset {
    pub seconds: i32,
    pub known: bool,
}

/// The widest offset the model admits, ±25:59:59 (§4).
pub const MAX_OFFSET: i32 = 25 * 3_600 + 59 * 60 + 59;

/// The probability of the unknown offset when it is allowed.
const UNKNOWN_PROBABILITY: f64 = 0.1;

/// Every offset any bundled zone has ever used, with the limits of
/// Appendix C (±01:00, ±18:00, ±23:59:59, ±25:59:59 and ±00:00:01),
/// ascending.
static OFFSET_CATALOGUE: Lazy<Result<Vec<i32>, InternalError>> = Lazy::new(|| {
    let mut set = BTreeSet::from([1, 3_600, 18 * 3_600, 86_399, MAX_OFFSET]);
    set.extend(set.clone().into_iter().map(|s| -s));
    for name in tzdata::names()? {
        if !name.is_link() {
            let zone = name.zone;
            set.insert(zone.initial.utoff);
            set.extend(zone.transitions.iter().map(|t| t.kind.utoff));
            if let Some(footer) = &zone.footer {
                set.insert(footer.standard.utoff);
                set.extend(footer.dst.map(|dst| dst.kind.utoff));
            }
        }
    }
    Ok(set.into_iter().collect())
});

/// Draw a [`UtcOffset`] in `[min_seconds, max_seconds]` at `granularity`,
/// or the unknown offset when `allow_unknown`, wrapped in a span. Shrinks
/// to known, then toward 0 or the nearest bound.
pub fn generate_offset(
    ntc: &mut NativeTestCase,
    min_seconds: i32,
    max_seconds: i32,
    granularity: Granularity,
    allow_unknown: bool,
) -> Result<UtcOffset, EngineError> {
    for (what, seconds) in [("min_seconds", min_seconds), ("max_seconds", max_seconds)] {
        if seconds.unsigned_abs() > MAX_OFFSET.unsigned_abs() {
            return Err(EngineError::InvalidArgument(format!(
                "{what} must be in [-{MAX_OFFSET}, {MAX_OFFSET}], got {seconds}"
            )));
        }
    }
    if min_seconds > max_seconds {
        return Err(EngineError::InvalidArgument(format!(
            "generate_offset requires min_seconds <= max_seconds, got \
             [{min_seconds}, {max_seconds}]"
        )));
    }
    let unit = granularity.unit();
    let (lo, hi) = (
        -(-min_seconds).div_euclid(unit),
        max_seconds.div_euclid(unit),
    );
    if lo > hi {
        return Err(EngineError::InvalidArgument(format!(
            "generate_offset: no offset at granularity {granularity:?} lies in \
             [{min_seconds}, {max_seconds}]"
        )));
    }
    let catalogue = OFFSET_CATALOGUE.as_ref().map_err(Clone::clone)?;
    let targets: Vec<i128> = catalogue
        .iter()
        .filter(|&&s| (min_seconds..=max_seconds).contains(&s) && s % unit == 0)
        .map(|&s| i128::from(s / unit))
        .collect();
    spanned(ntc, LABEL_OFFSET, |ntc| {
        if allow_unknown && ntc.weighted(UNKNOWN_PROBABILITY, None)? {
            return Ok(UtcOffset {
                seconds: 0,
                known: false,
            });
        }
        let count = draw_targeted(ntc, i128::from(lo), i128::from(hi), 0, vec![targets])?;
        Ok(UtcOffset {
            seconds: count as i32 * unit,
            known: true,
        })
    })
}

/// A drawn duration: `seconds` plus `nanoseconds` of the same sign, the
/// latter within ±999,999,999.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    pub seconds: i64,
    pub nanoseconds: i32,
}

/// The durations of Appendix C every duration draw targets when they lie
/// in its bounds at its precision, in nanoseconds: the units a library
/// cap or a day's arithmetic turns on, either sign.
fn interesting_duration_ns() -> impl Iterator<Item = i128> {
    [
        1,
        NS,
        60 * NS,
        3_600 * NS,
        23 * 3_600 * NS,
        86_400 * NS,
        25 * 3_600 * NS,
        36 * 3_600 * NS,
        7 * 86_400 * NS,
        365 * 86_400 * NS,
        i128::from(i32::MAX) * NS,
        i128::from(i64::MAX),
    ]
    .into_iter()
    .flat_map(|ns| [ns, -ns])
}

fn validate_duration(what: &str, d: &Duration) -> Result<(), EngineError> {
    let consistent = d.seconds == 0 || d.nanoseconds == 0 || (d.seconds < 0) == (d.nanoseconds < 0);
    if d.nanoseconds.unsigned_abs() > 999_999_999 || !consistent {
        return Err(EngineError::InvalidArgument(format!(
            "{what} is not a valid duration (nanoseconds within ±999999999 and of the sign of \
             seconds): {d:?}"
        )));
    }
    Ok(())
}

fn duration_ns(d: &Duration) -> i128 {
    i128::from(d.seconds) * NS + i128::from(d.nanoseconds)
}

fn duration_of(ns: i128) -> Duration {
    Duration {
        seconds: (ns / NS) as i64,
        nanoseconds: (ns % NS) as i32,
    }
}

/// Draw a [`Duration`] in `[min, max]` as a whole number of `precision`
/// units, wrapped in a span. Shrinks toward zero, or the nearest bound
/// when that is out of range.
pub fn generate_duration(
    ntc: &mut NativeTestCase,
    min: Duration,
    max: Duration,
    precision: Precision,
) -> Result<Duration, EngineError> {
    validate_duration("min_value", &min)?;
    validate_duration("max_value", &max)?;
    let (min_ns, max_ns) = (duration_ns(&min), duration_ns(&max));
    if min_ns > max_ns {
        return Err(EngineError::InvalidArgument(format!(
            "generate_duration requires min_value <= max_value, got [{min:?}, {max:?}]"
        )));
    }
    let unit = i128::from(precision.unit());
    let (lo, hi) = (align_up(min_ns, unit), align_down(max_ns, unit));
    if lo > hi {
        return Err(EngineError::InvalidArgument(format!(
            "generate_duration: no value at precision {precision:?} lies in [{min:?}, {max:?}]"
        )));
    }
    let mut targets: Vec<i128> = [lo, hi]
        .into_iter()
        .chain(interesting_duration_ns())
        .filter(|t| (lo..=hi).contains(t) && t % unit == 0)
        .map(|t| t / unit)
        .collect();
    targets.sort_unstable();
    targets.dedup();
    spanned(ntc, LABEL_DURATION, |ntc| {
        let count = draw_targeted(ntc, lo / unit, hi / unit, 0, vec![targets])?;
        Ok(duration_of(count * unit))
    })
}

/// A unit of a calendar span, largest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanUnit {
    Years = 0,
    Months = 1,
    Weeks = 2,
    Days = 3,
    Hours = 4,
    Minutes = 5,
    Seconds = 6,
    Milliseconds = 7,
    Microseconds = 8,
    Nanoseconds = 9,
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

/// How the signs of a span's units are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanSign {
    /// One sign for every unit, drawn as one node when the bounds admit
    /// both signs (jiff `Span`, Temporal `Duration`).
    Uniform,
    /// Each unit signed on its own (java `Period`).
    Mixed,
    /// Magnitudes only.
    NonNegative,
}

/// The configuration of a span draw: the inclusive bounds of each enabled
/// unit, in [`SpanUnit`] order, and the sign policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanSpec {
    pub bounds: [Option<(i64, i64)>; 10],
    pub sign: SpanSign,
}

impl Default for SpanSpec {
    /// No unit enabled, under the uniform sign policy.
    fn default() -> SpanSpec {
        SpanSpec {
            bounds: [None; 10],
            sign: SpanSign::Uniform,
        }
    }
}

/// A drawn calendar span: one `i64` per unit, in [`SpanUnit`] order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub units: [i64; 10],
}

impl SpanSpec {
    /// Enable `unit` with the inclusive bounds `[min, max]`.
    pub fn set_unit_bounds(&mut self, unit: SpanUnit, min: i64, max: i64) {
        self.bounds[unit as usize] = Some((min, max));
    }

    /// The enabled units with their bounds, largest unit first.
    fn enabled(&self) -> impl Iterator<Item = (SpanUnit, i64, i64)> + '_ {
        SpanUnit::ALL
            .into_iter()
            .filter_map(|unit| self.bounds[unit as usize].map(|(min, max)| (unit, min, max)))
    }

    /// Whether every enabled unit admits a non-negative value, and
    /// whether every one admits a non-positive value.
    fn admits(&self) -> (bool, bool) {
        self.enabled()
            .fold((true, true), |(pos, neg), (_, min, max)| {
                (pos && max >= 0, neg && min <= 0)
            })
    }

    /// Check that each enabled unit's bounds are in order and that the
    /// sign policy admits a value: `nonnegative` needs every lower bound
    /// at or above 0, and `uniform` needs every unit to admit one common
    /// sign.
    pub fn validate(&self) -> Result<(), EngineError> {
        for (unit, min, max) in self.enabled() {
            if min > max {
                return Err(EngineError::InvalidArgument(format!(
                    "generate_span requires min <= max for {unit:?}, got [{min}, {max}]"
                )));
            }
            if self.sign == SpanSign::NonNegative && min < 0 {
                return Err(EngineError::InvalidArgument(format!(
                    "generate_span: the nonnegative sign policy requires min >= 0 for \
                     {unit:?}, got {min}"
                )));
            }
        }
        let (pos, neg) = self.admits();
        if self.sign == SpanSign::Uniform && !pos && !neg {
            return Err(EngineError::InvalidArgument(format!(
                "generate_span: the uniform sign policy needs every unit to admit one sign, \
                 but {:?} admit none in common",
                self.bounds
            )));
        }
        Ok(())
    }
}

/// Draw a [`Span`] within `spec`'s bounds, wrapped in a span; see the
/// module docs. Shrinks to the zero span, or the nearest value to it in
/// each unit's bounds.
pub fn generate_span(ntc: &mut NativeTestCase, spec: &SpanSpec) -> Result<Span, EngineError> {
    spec.validate()?;
    if spec.enabled().next().is_none() {
        return Err(EngineError::InvalidArgument(
            "generate_span: no unit is enabled; set bounds on at least one".into(),
        ));
    }
    spanned(ntc, LABEL_SPAN, |ntc| {
        let mut span = Span::default();
        let negative = match (spec.sign, spec.admits()) {
            (SpanSign::Uniform, (true, true)) => ntc.weighted(0.5, None)?,
            (SpanSign::Uniform, (pos, _)) => !pos,
            _ => false,
        };
        for (unit, min, max) in spec.enabled() {
            let (min, max) = (i128::from(min), i128::from(max));
            let value = match spec.sign {
                SpanSign::Uniform if negative => {
                    -draw_targeted(ntc, (-max).max(0), -min, 0, Vec::new())?
                }
                SpanSign::Uniform => draw_targeted(ntc, min.max(0), max, 0, Vec::new())?,
                SpanSign::Mixed | SpanSign::NonNegative => {
                    draw_targeted(ntc, min, max, 0, Vec::new())?
                }
            };
            span.units[unit as usize] = hegel_internal_unwrap!(
                i64::try_from(value).ok(),
                "generate_span: {value} for {unit:?} does not fit i64"
            );
        }
        Ok(span)
    })
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/draws/scalar_tests.rs"]
mod tests;
