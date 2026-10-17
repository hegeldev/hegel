//! The time zone evaluator over the bundled database
//! (`hegel-c/docs/datetime.md` §5 and §7).
//!
//! A zone's history is its explicit transitions followed, from the last
//! one on, by the footer rule extrapolated to every later year. The three
//! questions the generators and the C ABI ask are answered here: the local
//! time type in force at an instant ([`type_at`]), how a wall-clock time
//! maps to instants ([`resolve`]: once, never, or twice), and which
//! transitions fall between two instants ([`transitions`]). The semantics
//! match jiff's, which the tests use as the oracle: the footer applies at
//! and after the last explicit transition, a gap is the wall times between
//! a transition's pre- and post-transition clock readings, and a fold the
//! same the other way round.
//!
//! A [`Timestamp`] a query takes must lie in the civil envelope of
//! [`MIN_YEAR`] to [`MAX_YEAR`] (§4), as a [`DateTime`] must, and one
//! outside it is an argument error. The raw-second functions [`type_at`]
//! and [`transitions`] clamp an instant to that envelope before the footer
//! rule is evaluated, so no input can push the calendar arithmetic out of
//! range; nothing here panics for any input.
//!
//! [`MIN_YEAR`]: crate::native::draws::special::MIN_YEAR
//! [`MAX_YEAR`]: crate::native::draws::special::MAX_YEAR

use alloc::ffi::CString;
use alloc::format;
use alloc::vec::Vec;
use core::ffi::CStr;

use crate::control::{InternalError, hegel_internal_unwrap};
use crate::native::core::EngineError;
use crate::native::draws::special::{
    Date, DateTime, civil_from_days, days_from_civil, days_in_month, validate_date, validate_time,
};
use crate::sys::sync::Lazy;
use crate::tzdata::{self, DstRule, Footer, LocalTimeType, Name, RuleDate, RuleDay, Zone};

/// The CLDR name of the zone with a known instant and an unknown offset
/// (§4): civil time is UTC and the offset reports as unknown.
pub const UNKNOWN_NAME: &str = "Etc/Unknown";

/// The abbreviation of [`UNKNOWN_NAME`], the tz database's marker for an
/// undefined offset.
pub const UNKNOWN_ABBREVIATION: &str = "-00";

const DAY: i64 = 86_400;

/// The first second of year `MIN_YEAR`, -999999.
pub const MIN_INSTANT: i64 = -31_619_087_596_800;

/// The last second of year `MAX_YEAR`, 999999.
pub const MAX_INSTANT: i64 = 31_494_784_780_799;

/// Half-width of the instant window a wall time's candidates are searched
/// in. Wider than any offset (RFC 9636 recommends |utoff| <= 93599), so
/// every instant the wall time could denote lies strictly inside it.
const WINDOW: i64 = 2 * DAY;

/// An instant: `second`s since the Unix epoch plus `nanosecond` within
/// that second, always counting forward (`-1.5 s` is `-2` and
/// `500_000_000`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    pub second: i64,
    pub nanosecond: u32,
}

/// The offset in force at an instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offset {
    /// Seconds east of UT; 0 when not `known`.
    pub seconds: i32,
    /// False only for [`UNKNOWN_NAME`].
    pub known: bool,
    /// The TZif `isdst` flag, informational only (§5.1).
    pub is_dst: bool,
    pub abbreviation: &'static str,
}

/// How a wall-clock time maps to instants in a zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Exactly one instant.
    Unique,
    /// None: the clocks skipped it.
    Gap,
    /// Two: the clocks repeated it.
    Fold,
}

/// A wall-clock time resolved against a zone (§4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub kind: Kind,
    /// For a gap or fold, the offsets before and after the transition;
    /// both the unique offset otherwise.
    pub offset_before: i32,
    pub offset_after: i32,
    /// The offset in force at `timestamp`.
    pub offset: i32,
    /// False only for [`UNKNOWN_NAME`].
    pub offset_known: bool,
    /// The resolved instant: the unique one; for a gap the wall time read
    /// with the pre-transition offset, which lands after the transition;
    /// for a fold the earlier instant, or the later with `fold` set.
    pub timestamp: Timestamp,
}

/// A transition: from `instant` on, `after` is in force in place of
/// `before`. The two may share an offset and differ only in flag or
/// abbreviation, as Europe/Lisbon's 1992 transition does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZoneTransition {
    pub instant: i64,
    pub before: LocalTimeType,
    pub after: LocalTimeType,
}

/// A zone the evaluator can answer for: a name in the database, or the
/// unknown zone.
#[derive(Clone, Copy, Debug)]
pub enum TimeZone {
    Unknown,
    Named(&'static Name),
}

impl TimeZone {
    /// The zone for `name`, exactly as the database spells it or
    /// [`UNKNOWN_NAME`], or `None` for a name the database lacks.
    pub fn lookup(name: &str) -> Result<Option<TimeZone>, InternalError> {
        if name == UNKNOWN_NAME {
            return Ok(Some(TimeZone::Unknown));
        }
        Ok(tzdata::lookup(name)?.map(TimeZone::Named))
    }

    /// The offset in force at `timestamp`.
    pub fn offset_at(&self, timestamp: Timestamp) -> Result<Offset, EngineError> {
        validate_timestamp(&timestamp)?;
        match self {
            TimeZone::Unknown => Ok(Offset {
                seconds: 0,
                known: false,
                is_dst: false,
                abbreviation: UNKNOWN_ABBREVIATION,
            }),
            TimeZone::Named(name) => {
                let kind = type_at(name.zone, timestamp.second)?;
                Ok(Offset {
                    seconds: kind.utoff,
                    known: true,
                    is_dst: kind.is_dst,
                    abbreviation: abbreviation(name.zone, kind)?,
                })
            }
        }
    }

    /// Classify the wall-clock time `civil` and resolve it to an instant;
    /// `fold` picks the later instant of a fold.
    pub fn resolve(&self, civil: &DateTime, fold: bool) -> Result<Resolution, EngineError> {
        validate_date("civil.date", &civil.date)?;
        validate_time("civil.time", &civil.time)?;
        match self {
            TimeZone::Unknown => Ok(Resolution {
                kind: Kind::Unique,
                offset_before: 0,
                offset_after: 0,
                offset: 0,
                offset_known: false,
                timestamp: Timestamp {
                    second: wall_seconds(civil),
                    nanosecond: civil.time.nanosecond,
                },
            }),
            TimeZone::Named(name) => Ok(resolve(name.zone, civil, fold)?),
        }
    }
}

fn validate_timestamp(timestamp: &Timestamp) -> Result<(), EngineError> {
    if !(MIN_INSTANT..=MAX_INSTANT).contains(&timestamp.second) {
        return Err(EngineError::InvalidArgument(format!(
            "timestamp.second must be in [{MIN_INSTANT}, {MAX_INSTANT}], got {}",
            timestamp.second
        )));
    }
    if timestamp.nanosecond > 999_999_999 {
        return Err(EngineError::InvalidArgument(format!(
            "timestamp.nanosecond must be at most 999999999, got {}",
            timestamp.nanosecond
        )));
    }
    Ok(())
}

/// The abbreviation `kind` names in `zone`.
pub fn abbreviation(zone: &Zone, kind: LocalTimeType) -> Result<&str, InternalError> {
    Ok(hegel_internal_unwrap!(
        zone.abbreviations.get(kind.abbreviation),
        "zone {} has no abbreviation {}",
        zone.name,
        kind.abbreviation
    ))
}

/// `civil` as seconds since the epoch read as if it were UTC.
fn wall_seconds(civil: &DateTime) -> i64 {
    let t = &civil.time;
    days_from_civil(&civil.date) * DAY
        + (i64::from(t.hour) * 60 + i64::from(t.minute)) * 60
        + i64::from(t.second)
}

fn year_of(second: i64) -> i64 {
    i64::from(civil_from_days(second.clamp(MIN_INSTANT, MAX_INSTANT).div_euclid(DAY)).year)
}

/// The local time type in force at `second` in `zone`: the initial type
/// before the first transition, the footer rule at and after the last,
/// and the last transition passed in between.
pub fn type_at(zone: &Zone, second: i64) -> Result<LocalTimeType, InternalError> {
    let n = zone.transitions.partition_point(|t| t.instant <= second);
    match (zone.footer, n) {
        (Some(footer), n) if n == zone.transitions.len() => footer_type_at(&footer, second),
        (_, 0) => Ok(zone.initial),
        (_, n) => Ok(zone.transitions[n - 1].kind),
    }
}

fn footer_type_at(footer: &Footer, second: i64) -> Result<LocalTimeType, InternalError> {
    let Some(dst) = footer.dst else {
        return Ok(footer.standard);
    };
    let second = second.clamp(MIN_INSTANT, MAX_INSTANT);
    let year = year_of(second);
    let last = RuleTransitions::new(footer.standard, dst, year - 2, year + 1)
        .take_while(|t| t.instant <= second)
        .last();
    Ok(hegel_internal_unwrap!(last, "no footer transition of {year} precedes {second}").after)
}

/// Days since the epoch of the day `rule` names in `year`.
fn rule_day(day: RuleDay, year: i64) -> i64 {
    let year = year as i32;
    let january_first = days_from_civil(&Date {
        year,
        month: 1,
        day: 1,
    });
    match day {
        RuleDay::Julian(n) => {
            let leap_day = i64::from(days_in_month(year, 2) == 29 && n >= 60);
            january_first + i64::from(n) - 1 + leap_day
        }
        RuleDay::ZeroBased(n) => january_first + i64::from(n),
        RuleDay::MonthWeekDay {
            month,
            week,
            weekday,
        } => {
            let first = days_from_civil(&Date {
                year,
                month,
                day: 1,
            });
            let first_weekday = (first + 4).rem_euclid(7);
            let mut day =
                (i64::from(weekday) - first_weekday).rem_euclid(7) + 7 * i64::from(week - 1);
            if day >= days_in_month(year, month) {
                day -= 7;
            }
            first + day
        }
    }
}

/// The instant of `rule` in `year`, whose time is read with `utoff`.
fn rule_instant(rule: RuleDate, year: i64, utoff: i32) -> i64 {
    rule_day(rule.day, year) * DAY + i64::from(rule.time) - i64::from(utoff)
}

/// The footer rule's transitions over a range of years, in instant order,
/// with transitions at one instant merged and those that change nothing
/// dropped (so all-year DST written as `0/0,J365/25` yields none).
struct RuleTransitions {
    standard: LocalTimeType,
    dst: DstRule,
    year: i64,
    last_year: i64,
    pending: Vec<(i64, LocalTimeType)>,
    current: Option<LocalTimeType>,
}

impl RuleTransitions {
    fn new(standard: LocalTimeType, dst: DstRule, first_year: i64, last_year: i64) -> Self {
        RuleTransitions {
            standard,
            dst,
            year: first_year,
            last_year,
            pending: Vec::new(),
            current: None,
        }
    }

    /// Keep enough years buffered that the earliest pending instant is
    /// final: a rule time within ±167 h can reorder a year's transitions
    /// only against its neighbours', never against the year after next.
    fn refill(&mut self) {
        while self.pending.len() < 4 && self.year <= self.last_year {
            let start = rule_instant(self.dst.start, self.year, self.standard.utoff);
            let end = rule_instant(self.dst.end, self.year, self.dst.kind.utoff);
            for entry in [(start, self.dst.kind), (end, self.standard)] {
                let at = self.pending.partition_point(|p| p.0 <= entry.0);
                self.pending.insert(at, entry);
            }
            self.year += 1;
        }
    }

    fn next_merged(&mut self) -> Option<(i64, LocalTimeType)> {
        self.refill();
        if self.pending.is_empty() {
            return None;
        }
        let (instant, mut kind) = self.pending.remove(0);
        loop {
            self.refill();
            match self.pending.first() {
                Some(&(next, next_kind)) if next == instant => {
                    kind = next_kind;
                    self.pending.remove(0);
                }
                _ => return Some((instant, kind)),
            }
        }
    }
}

impl Iterator for RuleTransitions {
    type Item = ZoneTransition;

    fn next(&mut self) -> Option<ZoneTransition> {
        loop {
            let (instant, after) = self.next_merged()?;
            let before = self.current.unwrap_or(if after == self.dst.kind {
                self.standard
            } else {
                self.dst.kind
            });
            self.current = Some(after);
            if before != after {
                return Some(ZoneTransition {
                    instant,
                    before,
                    after,
                });
            }
        }
    }
}

/// Every transition of `zone` with `from <= instant < to`, in order: the
/// explicit ones, then the footer rule's after the last explicit one. The
/// rule's transitions are confined to the civil envelope.
pub struct Transitions<'a> {
    zone: &'a Zone,
    to: i64,
    explicit: usize,
    rule_from: i64,
    rule_to: i64,
    rule: Option<RuleTransitions>,
}

/// See [`Transitions`].
pub fn transitions(zone: &Zone, from: i64, to: i64) -> Transitions<'_> {
    let explicit = zone.transitions.partition_point(|t| t.instant < from);
    let last = zone.transitions.last().map_or(i64::MIN, |t| t.instant);
    let rule_from = from.max(MIN_INSTANT).max(last.saturating_add(1));
    let rule_to = to.min(MAX_INSTANT + 1);
    let rule = zone.footer.and_then(|footer| {
        let dst = footer.dst?;
        let first_year = year_of(rule_from) - 2;
        let last_year = year_of(rule_to) + 1;
        Some(RuleTransitions::new(
            footer.standard,
            dst,
            first_year,
            last_year,
        ))
    });
    Transitions {
        zone,
        to,
        explicit,
        rule_from,
        rule_to,
        rule,
    }
}

impl Iterator for Transitions<'_> {
    type Item = ZoneTransition;

    fn next(&mut self) -> Option<ZoneTransition> {
        if let Some(t) = self.zone.transitions.get(self.explicit) {
            if t.instant >= self.to {
                return None;
            }
            let before = match self.explicit {
                0 => self.zone.initial,
                i => self.zone.transitions[i - 1].kind,
            };
            self.explicit += 1;
            return Some(ZoneTransition {
                instant: t.instant,
                before,
                after: t.kind,
            });
        }
        let rule = self.rule.as_mut()?;
        loop {
            let t = rule.next()?;
            if t.instant >= self.rule_to {
                self.rule = None;
                return None;
            }
            if t.instant >= self.rule_from {
                return Some(t);
            }
        }
    }
}

/// Classify the wall-clock time `civil` against `zone`; see
/// [`Resolution`]. `civil` must be valid.
pub fn resolve(zone: &Zone, civil: &DateTime, fold: bool) -> Result<Resolution, InternalError> {
    let wall = wall_seconds(civil);
    let (lo, hi) = (wall - WINDOW, wall + WINDOW);
    let mut candidates: Vec<(i64, i32)> = Vec::new();
    let mut gap = None;
    let mut candidate = |start: i64, end: i64, utoff: i32| {
        let instant = wall - i64::from(utoff);
        if start <= instant && instant < end {
            candidates.push((instant, utoff));
        }
    };
    let mut kind = type_at(zone, lo)?;
    let mut start = lo;
    for t in transitions(zone, lo, hi) {
        candidate(start, t.instant, kind.utoff);
        let (before, after) = (i64::from(t.before.utoff), i64::from(t.after.utoff));
        if t.instant + before <= wall && wall < t.instant + after {
            gap = Some((t.before.utoff, t.after.utoff));
        }
        kind = t.after;
        start = t.instant;
    }
    candidate(start, hi, kind.utoff);
    let nanosecond = civil.time.nanosecond;
    let at = |second: i64| Timestamp { second, nanosecond };
    Ok(match candidates.as_slice() {
        [] => {
            let (offset_before, offset_after) =
                hegel_internal_unwrap!(gap, "{} skips {wall} without a transition", zone.name);
            Resolution {
                kind: Kind::Gap,
                offset_before,
                offset_after,
                offset: offset_after,
                offset_known: true,
                timestamp: at(wall - i64::from(offset_before)),
            }
        }
        [(second, utoff)] => Resolution {
            kind: Kind::Unique,
            offset_before: *utoff,
            offset_after: *utoff,
            offset: *utoff,
            offset_known: true,
            timestamp: at(*second),
        },
        [earlier, .., later] => {
            let (second, offset) = if fold { *later } else { *earlier };
            Resolution {
                kind: Kind::Fold,
                offset_before: earlier.1,
                offset_after: later.1,
                offset,
                offset_known: true,
                timestamp: at(second),
            }
        }
    })
}

/// A name's strings as the C ABI hands them out: the primary zone's name
/// and the zone's abbreviations, NUL-terminated and static.
#[derive(Debug)]
pub struct CStrings {
    pub primary_name: CString,
    pub abbreviations: Vec<CString>,
}

fn c_strings() -> Result<&'static [CStrings], InternalError> {
    static TABLE: Lazy<Result<Vec<CStrings>, InternalError>> = Lazy::new(|| {
        let c = |s: &str| -> Result<CString, InternalError> {
            Ok(hegel_internal_unwrap!(
                CString::new(s).ok(),
                "tzdata string {s:?} contains a NUL byte"
            ))
        };
        tzdata::names()?
            .iter()
            .map(|name| {
                Ok(CStrings {
                    primary_name: c(&name.zone.name)?,
                    abbreviations: name
                        .zone
                        .abbreviations
                        .iter()
                        .map(|a| c(a))
                        .collect::<Result<Vec<CString>, InternalError>>()?,
                })
            })
            .collect()
    });
    TABLE.as_ref().map(Vec::as_slice).map_err(Clone::clone)
}

/// The C strings of `name`, which must come from the database.
pub fn c_strings_of(name: &Name) -> Result<&'static CStrings, InternalError> {
    let names = tzdata::names()?;
    let index = hegel_internal_unwrap!(
        names
            .binary_search_by(|entry| entry.name.as_str().cmp(&name.name))
            .ok(),
        "tzdata has no name {}",
        name.name
    );
    Ok(&c_strings()?[index])
}

/// [`UNKNOWN_ABBREVIATION`] as a C string.
pub static UNKNOWN_ABBREVIATION_CSTR: &CStr = c"-00";

/// [`UNKNOWN_NAME`] as a C string.
pub static UNKNOWN_NAME_CSTR: &CStr = c"Etc/Unknown";

#[cfg(test)]
#[path = "../../tests/embedded/native/timezone_tests.rs"]
mod tests;
